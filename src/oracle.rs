//! Source adapters for the published full-history oracle construction. The
//! library supplies every gate; source adapters supply state, rows and successors.
use crate::{
    Error, Result, Status,
    expansion::{Binding, Env, Library, Operation},
    object::{self, Node, Object},
    schedule::{Coordinate, Kind, Location, Occurrence, Phase},
    source::{Event, Invocation, Slot, Trace},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn postponed(feature: &str) -> Error {
    Error::new(
        Status::GateUnsupported,
        format!("§11: {feature} is postponed (bennett-sched-ark)"),
    )
}
fn allocation(name: &str, width: u64) -> Result<Binding> {
    let identity = format!("oracle-state/{name}");
    let n = usize::try_from(width)
        .map_err(|_| Error::new(Status::NoMem, "oracle allocation exceeds host size"))?;
    let mut wires = Vec::new();
    wires
        .try_reserve_exact(n)
        .map_err(|_| Error::new(Status::NoMem, "oracle allocation exceeds host memory"))?;
    wires.extend((0..n).map(|i| format!("{identity}[{i}]")));
    Ok(Binding::Allocation {
        identity,
        wires,
        class: "any".into(),
        state: "clean".into(),
    })
}
fn read(name: &str, env: &Env) -> Result<Vec<String>> {
    match env.get(name) {
        Some(Binding::Wires(w) | Binding::Allocation { wires: w, .. }) => Ok(w.clone()),
        Some(Binding::Value { .. }) => Err(postponed("classical pseudo-location bit adapters")),
        _ => Err(Error::arg(format!("§4.2: unbound oracle operand {name}"))),
    }
}
fn invoke(name: &str, args: Vec<Value>) -> Value {
    json!({"invoke":name,"args":args})
}
fn slice(from: u64, width: u64) -> Value {
    json!({"range":"frame","from":from,"to":from+width})
}
fn widths(node: &Node, env: &Env, locations: &BTreeMap<String, u64>) -> Result<Vec<u64>> {
    if let Some(lhs) = &node.lhs {
        let names = if lhs.starts_with('(') {
            object::list(lhs)?
        } else {
            vec![lhs.clone()]
        };
        names
            .iter()
            .map(|n| {
                locations.get(n).copied().ok_or_else(|| {
                    Error::arg("§3.6: assignment must write a declared oracle location")
                })
            })
            .collect()
    } else {
        if node.op.starts_with("lift.")
            && ![
                "lift.retval",
                "lift.faultif",
                "lift.boundscheck",
                "lift.free",
                "lift.store",
                "lift.oracle_call",
                "lift.oracle_uncall",
                "lift.perm_call",
                "lift.ret",
            ]
            .iter()
            .any(|op| node.op == *op || node.op.starts_with(&format!("{op}.")))
        {
            return Err(Error::arg("lifted assignment requires a declared result"));
        }
        let _ = env;
        Ok(vec![])
    }
}
fn row(
    node: &Node,
    index: u64,
    n: u64,
    p: u64,
    f: u64,
    t: u64,
    slots: &[u64],
    env: &Env,
    locations: &BTreeMap<String, u64>,
    labels: &BTreeMap<String, u64>,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut body = Vec::new();
    let operands = &node.operands;
    let written = widths(node, env, locations)?;
    let destination = node.lhs.as_deref();
    let mut successor = index + 1;
    let flag = slice(p + slots.iter().sum::<u64>(), 1);
    let succ = slice(p + slots.iter().sum::<u64>() + 1, p);
    let slot = slice(p, slots.first().copied().unwrap_or(0));
    let mut flagged = false;
    let mut dynamic = false;
    let arity = |count: usize| {
        if operands.len() == count {
            Ok(())
        } else {
            Err(Error::arg(format!("§4.2: {} operand count", node.op)))
        }
    };
    let width = |name: &str| read(name, env).map(|w| w.len() as u64);
    let label = |name: &str| {
        name.strip_prefix('^')
            .and_then(|s| labels.get(s))
            .copied()
            .ok_or_else(|| Error::arg("unknown oracle label"))
    };
    let dest = || destination.ok_or_else(|| Error::arg("missing assignment destination"));
    let written_width = || {
        written
            .first()
            .copied()
            .ok_or_else(|| Error::arg("missing assignment width"))
    };
    if let Some(suffix) = node.op.strip_prefix("lift.const.i") {
        arity(1)?;
        let w = written_width()?;
        if crate::schedule::uint(&json!(suffix))? != w {
            return Err(Error::arg("lift.const suffix/destination width mismatch"));
        }
        let constant = operands[0]
            .parse::<num_bigint::BigInt>()
            .map_err(|_| Error::arg("invalid integer literal"))?;
        let modulus = num_bigint::BigInt::from(1u8)
            << usize::try_from(w)
                .map_err(|_| Error::new(Status::NoMem, "literal width exceeds host size"))?;
        if constant.to_string() != operands[0]
            || constant < -(&modulus / 2u8)
            || constant >= modulus
        {
            return Err(Error::arg(
                "§3.3: noncanonical or out-of-range lifted constant",
            ));
        }
        let constant = ((constant % &modulus) + &modulus) % &modulus;
        body.push(invoke(
            "lift.const",
            vec![
                json!(w),
                serde_json::from_str(&constant.to_string())?,
                slot,
                json!(dest()?),
            ],
        ));
    } else if [
        "lift.add",
        "lift.sub",
        "lift.mul",
        "lift.and",
        "lift.or",
        "lift.xor",
        "lift.shl",
        "lift.lshr",
        "lift.ashr",
    ]
    .contains(&node.op.as_str())
        || node.op.starts_with("lift.icmp.")
    {
        arity(2)?;
        let w = width(&operands[0])?;
        if width(&operands[1])? != w
            || written_width()?
                != if node.op.starts_with("lift.icmp.") {
                    1
                } else {
                    w
                }
        {
            return Err(Error::arg(
                "lifted arithmetic operand/destination width mismatch",
            ));
        }
        body.push(invoke(
            &node.op,
            vec![
                json!(w),
                json!(operands[0]),
                json!(operands[1]),
                slice(p, written_width()?),
                json!(dest()?),
            ],
        ));
    } else if ["lift.udiv", "lift.urem", "lift.sdiv", "lift.srem"].contains(&node.op.as_str()) {
        arity(2)?;
        let w = width(&operands[0])?;
        if width(&operands[1])? != w || written_width()? != w {
            return Err(Error::arg("division width mismatch"));
        }
        body.push(invoke(
            &node.op,
            vec![
                json!(w),
                json!(f),
                json!(operands[0]),
                json!(operands[1]),
                slice(p, w),
                json!(dest()?),
                flag.clone(),
                json!("faulted"),
                json!("code"),
            ],
        ));
        flagged = true;
    } else if node.op == "lift.mov" {
        arity(1)?;
        let w = width(&operands[0])?;
        if written_width()? != w {
            return Err(Error::arg("lift.mov width mismatch"));
        }
        body.push(invoke(
            "lift.mov",
            vec![json!(w), json!(operands[0]), slice(p, w), json!(dest()?)],
        ));
    } else if ["lift.zext.i", "lift.sext.i", "lift.trunc.i"]
        .iter()
        .any(|s| node.op.starts_with(s))
    {
        arity(1)?;
        let (op, suffix) = node.op.rsplit_once(".i").unwrap();
        let m = crate::schedule::uint(&json!(suffix))?;
        let w = width(&operands[0])?;
        if m != written_width()? || (op == "lift.trunc" && m >= w) || (op != "lift.trunc" && m <= w)
        {
            return Err(Error::arg("cast width mismatch"));
        }
        body.push(invoke(
            op,
            vec![
                json!(w),
                json!(m),
                json!(operands[0]),
                slice(p, m),
                json!(dest()?),
            ],
        ));
    } else if node.op == "lift.select" {
        arity(3)?;
        let w = width(&operands[1])?;
        if width(&operands[0])? != 1 || width(&operands[2])? != w || written_width()? != w {
            return Err(Error::arg("lift.select width mismatch"));
        }
        body.push(invoke(
            "lift.select",
            vec![
                json!(w),
                json!(operands[1]),
                json!(operands[2]),
                json!(operands[0]),
                slice(p, w),
                json!(dest()?),
            ],
        ));
    } else if node.op == "lift.parmov" {
        arity(1)?;
        let sources = object::list(&operands[0])?;
        let destinations = object::list(dest()?)?;
        if sources.len() != written.len() || destinations.len() != sources.len() {
            return Err(Error::arg("parallel move list mismatch"));
        }
        let mut offset = p;
        for ((src, w), slot_width) in sources.iter().zip(&written).zip(slots) {
            if width(src)? != *w {
                return Err(Error::arg("parallel move width mismatch"));
            }
            for bit in 0..*w {
                body.push(
                    json!({"gate":"cx","wires":[format!("{src}[{bit}]"),slice(offset+bit,1)]}),
                );
            }
            offset += slot_width;
        }
        offset = p;
        for ((dst, w), slot_width) in destinations.iter().zip(&written).zip(slots) {
            for bit in 0..*w {
                body.push(
                    json!({"gate":"swap","wires":[slice(offset+bit,1),format!("{dst}[{bit}]")]}),
                );
            }
            offset += slot_width;
        }
    } else if node.op == "lift.label" {
        arity(1)?;
        let w = written_width()?;
        if w < p {
            return Err(Error::arg(
                "label location cannot represent the index space",
            ));
        }
        body.push(invoke(
            "lift.label",
            vec![
                json!(w),
                json!(label(&operands[0])?),
                slice(p, w),
                json!(dest()?),
            ],
        ));
    } else if node.op == "lift.retval" {
        arity(1)?;
        if width(&operands[0])? != t {
            return Err(Error::arg("retval width does not match target"));
        }
        body.push(invoke(
            "lift.retval",
            vec![json!(t), json!(operands[0]), json!("result")],
        ));
        successor = n;
    } else if node.op == "qjump" {
        arity(1)?;
        successor = label(&operands[0])?;
        body.push(invoke("qjump", vec![json!(successor)]));
    } else if node.op == "qbranch" {
        arity(3)?;
        if width(&operands[0])? != 1 {
            return Err(Error::arg("qbranch condition must be one bit"));
        }
        body.push(invoke(
            "qbranch",
            vec![
                json!(p),
                json!(label(&operands[1])?),
                json!(label(&operands[2])?),
                json!(operands[0]),
                succ.clone(),
            ],
        ));
        dynamic = true;
    } else if node.op == "qjumpr" {
        arity(1)?;
        if width(&operands[0])? != p {
            return Err(Error::arg("qjumpr width must equal canonical P"));
        }
        body.push(invoke(
            "qjumpr",
            vec![
                json!(p),
                json!(f),
                json!(0),
                json!(n),
                json!(operands[0]),
                succ.clone(),
                flag.clone(),
                json!("faulted"),
                json!("code"),
            ],
        ));
        flagged = true;
        dynamic = true;
    } else if node.op == "fault" {
        arity(1)?;
        let c = crate::schedule::uint(&json!(operands[0]))?;
        if u64::from(64 - c.leading_zeros()) > f {
            return Err(Error::arg(
                "§3.3: fault literal does not fit the fault register",
            ));
        }
        body.push(invoke(
            "fault",
            vec![json!(f), json!(c), json!("faulted"), json!("code")],
        ));
        successor = n;
    } else if node.op == "lift.faultif" {
        arity(2)?;
        if width(&operands[0])? != 1 {
            return Err(Error::arg("faultif condition width"));
        }
        let cause = crate::schedule::uint(&json!(operands[1]))?;
        if u64::from(64 - cause.leading_zeros()) > f {
            return Err(Error::arg(
                "§3.3: faultif literal does not fit the fault register",
            ));
        }
        body.push(invoke(
            "lift.faultif",
            vec![
                json!(f),
                json!(cause),
                json!(operands[0]),
                flag.clone(),
                json!("faulted"),
                json!("code"),
            ],
        ));
        flagged = true;
    } else {
        return Err(postponed(&format!("source row adapter for {}", node.op)));
    }
    let value = if dynamic {
        vec![
            json!({"for":"i","from":0,"to":p,"body":[{"gate":"cx","wires":[{"range":"frame","from":["+",p+slots.iter().sum::<u64>()+1,"i"],"to":["+",p+slots.iter().sum::<u64>()+2,"i"]},"next[\"i\"]"]}]}),
        ]
    } else {
        vec![invoke(
            "value.literal",
            vec![json!(p), json!(successor), json!("next")],
        )]
    };
    let next = if flagged {
        vec![
            json!({"gate":"x","wires":[flag.clone()]}),
            json!({"under":flag.clone(),"body":value}),
            json!({"gate":"x","wires":[flag.clone()]}),
            json!({"under":flag,"body":[invoke("value.literal",vec![json!(p),json!(n),json!("next")])]}),
        ]
    } else {
        value
    };
    Ok((body, next))
}

pub fn build(
    object: &Object,
    name: &str,
    inherited: u32,
    params: &Value,
    budget: [u64; 4],
) -> Result<Trace> {
    let f = &object.functions[name];
    if object.metadata.contains_key("layout") {
        return Err(postponed(
            "layout state and fixed-binding lproc oracle construction",
        ));
    }
    if !f.groups[3].is_empty() {
        return Err(Error::arg(
            "memoryless oracle cannot declare interface memory",
        ));
    }
    if budget[1..].iter().any(|n| *n != 0) {
        return Err(Error::new(
            Status::Budget,
            "memoryless oracle requires zero memory/stack/depth limits",
        ));
    }
    let library = Library::published()?;
    let mut env = Env::new();
    let mut slots = Vec::new();
    for (group, role) in [(0, "in"), (1, "tgt"), (2, "fault")] {
        for (index, param) in f.groups[group].iter().enumerate() {
            let width = object::wire_width(&param.ty)?;
            let id = format!("{role}:{index}");
            let wires = (0..width).map(|i| format!("{id}[{i}]")).collect::<Vec<_>>();
            env.insert(
                format!("source.{}", param.name),
                Binding::Wires(wires.clone()),
            );
            slots.push(Slot {
                id,
                role: role.into(),
                wires,
            });
        }
    }
    for (p, value) in f.groups[4].iter().zip(params.as_array().unwrap()) {
        env.insert(
            format!("source.{}", p.name),
            Binding::Value {
                isa_type: json!(p.ty),
                value: value.clone(),
            },
        );
    }
    let n = f.body.len() as u64;
    if n == 0 || f.body.iter().any(|r| !r.bodies.is_empty()) {
        return Err(Error::arg("oracle rows must be nonempty and unstructured"));
    }
    let p = (64 - n.leading_zeros()) as u64;
    let fault = object::wire_width(&f.groups[2][0].ty)?;
    let target = object::wire_width(&f.groups[1][0].ty)?;
    let mut locations = BTreeMap::new();
    for reg in &f.regs {
        if !reg.ty.starts_with("reg[")
            || locations
                .insert(format!("source.{}", reg.name), object::wire_width(&reg.ty)?)
                .is_some()
            || env.contains_key(&format!("source.{}", reg.name))
        {
            return Err(Error::arg("invalid/duplicate oracle location"));
        }
    }
    fn rename(value: &str, names: &std::collections::BTreeSet<String>) -> Result<String> {
        if names.contains(value) {
            Ok(format!("source.{value}"))
        } else if value.starts_with('(') {
            Ok(format!(
                "({})",
                object::list(value)?
                    .iter()
                    .map(|n| rename(n, names))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ")
            ))
        } else {
            Ok(value.into())
        }
    }
    let names = f
        .groups
        .iter()
        .flatten()
        .chain(&f.regs)
        .map(|p| p.name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut nodes = f.body.clone();
    for node in &mut nodes {
        node.lhs = node.lhs.as_ref().map(|s| rename(s, &names)).transpose()?;
        node.operands = node
            .operands
            .iter()
            .map(|s| rename(s, &names))
            .collect::<Result<_>>()?;
    }
    let mut slot_widths = Vec::<u64>::new();
    for node in &nodes {
        for (i, width) in widths(node, &env, &locations)?.into_iter().enumerate() {
            if i == slot_widths.len() {
                slot_widths.push(width);
            } else {
                slot_widths[i] = slot_widths[i].max(width);
            }
        }
    }
    let q = p
        .checked_mul(2)
        .and_then(|v| v.checked_add(1))
        .and_then(|v| v.checked_add(slot_widths.iter().sum()))
        .ok_or_else(|| Error::new(Status::NoMem, "oracle frame exceeds host width"))?;
    let state_schema = library.recipes["oracle.construct"]["parameters"][13].clone();
    let frames_schema = library.recipes["oracle.construct"]["parameters"][14].clone();
    let mut state = Vec::new();
    let frame_count = usize::try_from(budget[0])
        .map_err(|_| Error::new(Status::NoMem, "history count exceeds host size"))?;
    state
        .try_reserve_exact(frame_count)
        .map_err(|_| Error::new(Status::NoMem, "state descriptors exceed host memory"))?;
    let mut add_state = |name: &str, width: u64, env: &mut Env| -> Result<()> {
        let storage = allocation(name, width)?;
        env.insert(name.into(), storage.clone());
        state.push(Env::from([
            ("width".into(), Binding::Integer(width.into())),
            ("initial".into(), Binding::Integer(0u8.into())),
            ("I".into(), Binding::Image(String::new())),
            ("storage".into(), storage),
        ]));
        Ok(())
    };
    for (name, width) in [
        ("pc", p),
        ("next", p),
        ("faulted", 1),
        ("code", fault),
        ("result", target),
    ] {
        if env.contains_key(name) {
            return Err(Error::arg(
                "source name conflicts with oracle adapter state; qualified binding required",
            ));
        }
        add_state(name, width, &mut env)?;
    }
    for reg in &f.regs {
        add_state(
            &format!("source.{}", reg.name),
            locations[&format!("source.{}", reg.name)],
            &mut env,
        )?;
    }
    let mut frames = Vec::new();
    let frame_count = usize::try_from(budget[0])
        .map_err(|_| Error::new(Status::NoMem, "history count exceeds host size"))?;
    frames
        .try_reserve_exact(frame_count)
        .map_err(|_| Error::new(Status::NoMem, "history descriptors exceed host memory"))?;
    for index in 0..budget[0] {
        let name = format!("history-{index}");
        add_state(&name, q, &mut env)?;
        frames.push(Env::from([
            ("index".into(), Binding::Integer(index.into())),
            ("frame".into(), env[&name].clone()),
        ]));
    }
    env.insert(
        "State".into(),
        Binding::Records {
            schema: state_schema,
            records: state,
        },
    );
    env.insert(
        "Frames".into(),
        Binding::Records {
            schema: frames_schema,
            records: frames,
        },
    );
    let mut labels = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        labels
            .entry(node.point.split(':').next().unwrap().to_owned())
            .or_insert(i as u64);
    }
    for (i, node) in nodes.iter().enumerate() {
        let block = node.point.split(':').next().unwrap();
        if (i + 1 == f.body.len() || f.body[i + 1].point.split(':').next().unwrap() != block)
            && !["qjump", "qbranch", "qjumpr", "lift.retval", "fault"].contains(&node.op.as_str())
        {
            return Err(Error::arg("§3.6: oracle block needs a terminator"));
        }
    }
    let mut rows = Vec::new();
    let mut next = Vec::new();
    let readable=f.groups[0].iter().chain(f.groups[4].iter().filter(|p|p.ty!="token")).chain(&f.regs).map(|p|format!("source.{}",p.name)).collect::<std::collections::BTreeSet<_>>();
    let source_env=env.iter().filter(|(name,_)|readable.contains(*name)).map(|(name,binding)|(name.clone(),binding.clone())).collect::<Env>();
    for (i, node) in nodes.iter().enumerate() {
        let (body, successor) = row(
            node,
            i as u64,
            n,
            p,
            fault,
            target,
            &slot_widths,
            &source_env,
            &locations,
            &labels,
        )?;
        rows.push(json!({"parameters":[{"name":"frame","type":"wire","width":q}],"body":body}));
        next.push(json!({"parameters":[{"name":"frame","type":"wire","width":q},{"name":"next","type":"wire","width":p}],"body":successor}));
    }
    let args = json!([
        p,
        n,
        q,
        budget[0],
        fault,
        target,
        "pc",
        "next",
        "faulted",
        "code",
        "result",
        format!("source.{}", f.groups[1][0].name),
        format!("source.{}", f.groups[2][0].name),
        "State",
        "Frames",
        rows,
        next
    ]);
    let inherited = (0..inherited)
        .map(|i| format!("control:{i}"))
        .collect::<Vec<_>>();
    let leaves = library.body(
        &[json!({"invoke":"oracle.construct","args":args})],
        &env,
        &inherited,
    )?;
    let mut counts = BTreeMap::<(u64, String), u64>::new();
    for leaf in &leaves {
        if let Some(frame) = leaf
            .origin
            .iter()
            .rev()
            .find(|o| o.constructor == "visit" && o.name == "Frames" && !o.reverse)
        {
            let point = leaf
                .origin
                .iter()
                .rev()
                .find(|o| o.constructor == "apply" && (o.name == "Rows" || o.name == "Next"))
                .map(|o| f.body[o.index].point.clone())
                .unwrap_or_else(|| "$step".into());
            *counts.entry((frame.index as u64 + 1, point)).or_default() += 1;
        }
    }
    let mut ordinals = BTreeMap::<(Option<u64>, bool, String), u64>::new();
    let mut events = Vec::new();
    for mut leaf in leaves {
        let state = leaf
            .origin
            .iter()
            .rev()
            .find(|o| o.constructor == "visit" && o.name == "State");
        let frame = leaf
            .origin
            .iter()
            .rev()
            .find(|o| o.constructor == "visit" && o.name == "Frames");
        let (point, step, phase) = if let Some(state) = state {
            (
                if state.reverse {
                    "$release"
                } else {
                    "$construct"
                }
                .to_owned(),
                None,
                None,
            )
        } else if let Some(frame) = frame {
            let point = leaf
                .origin
                .iter()
                .rev()
                .find(|o| o.constructor == "apply" && (o.name == "Rows" || o.name == "Next"))
                .map(|o| f.body[o.index].point.clone())
                .unwrap_or_else(|| "$step".into());
            (
                point,
                Some(frame.index as u64 + 1),
                Some(if frame.reverse {
                    Phase::Rev
                } else {
                    Phase::Fwd
                }),
            )
        } else {
            ("$commit".into(), None, None)
        };
        let counter = ordinals
            .entry((step, phase == Some(Phase::Rev), point.clone()))
            .or_default();
        let ordinal = if phase == Some(Phase::Rev) {
            counts[&(step.unwrap(), point.clone())] - 1 - *counter
        } else {
            *counter
        };
        *counter += 1;
        leaf.ordinal = ordinal;
        let kind = match &leaf.op {
            Operation::Gate(_) => Kind::Gate,
            Operation::Alloc { .. } => Kind::Alloc,
            Operation::Release { .. } => Kind::Release,
            Operation::Classical { .. } => {
                return Err(Error::arg("classical event in oracle construction"));
            }
        };
        events.push(Event {
            owner: 0,
            leaf,
            location: Location {
                root: Coordinate { step, phase },
                path: vec![Occurrence {
                    symbol: object.symbols[name].interface_hash.clone(),
                    point,
                    iteration: vec![],
                    instance: vec![],
                    call_index: None,
                    entered_oracle: None,
                }],
                generated: ordinal,
                kind,
            },
        });
    }
    let end = events.len();
    Ok(Trace {
        events,
        invocations: vec![Invocation {
            name: name.into(),
            hash: object.closure_hash(name)?.into(),
            controls: inherited.clone(),
            slots: slots.clone(),
            params: params.clone(),
            budget,
            start: 0,
            end,
            parent: None,
            adjoint: false,
            path: vec![],
        }],
        slots,
        inherited,
        borrow_proofs: vec![],
        cleanups: vec![],
    })
}
