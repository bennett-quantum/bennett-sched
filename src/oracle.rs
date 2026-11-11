//! Source adapters for the published full-history oracle construction. The
//! library supplies every gate; source adapters supply state, rows and successors.
use crate::{
    Error, Result, Status,
    expansion::{Binding, Env, Library, Operation},
    object::{self, Node, Object},
    schedule::{Coordinate, Kind, Phase},
    source::{Event, Invocation, Slot, Trace},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
pub(crate) fn read(name: &str, env: &Env) -> Result<Vec<String>> {
    match env.get(name) {
        Some(Binding::Wires(w) | Binding::Allocation { wires: w, .. }) => Ok(w.clone()),
        Some(Binding::Value { isa_type, .. }) if isa_type == "token" => {
            Err(Error::arg("§4.7: a token is not a bit-readable location"))
        }
        Some(Binding::Value { .. }) => Err(Error::new(
            Status::GateUnsupported,
            "§4.7/K25: classical pseudo-location physical wire initialization requires a published contract",
        )),
        _ => Err(Error::arg(format!("§4.2: unbound oracle operand {name}"))),
    }
}
fn invoke(name: &str, args: Vec<Value>) -> Value {
    json!({"invoke":name,"args":args})
}
pub(crate) fn slice(from: u64, width: u64) -> Value {
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
pub(crate) struct Source<'a> {
    pub object: &'a Object,
    pub name: &'a str,
    pub layout: Option<&'a crate::layout::Layout>,
    pub plan: &'a crate::oracle_state::Plan,
    pub function: &'a object::Function,
    pub instance: Option<usize>,
    pub original: &'a Node,
    pub budget: [u64; 4],
    pub first: u64,
}
struct RowInput<'a> {
    node: &'a Node,
    env: &'a Env,
    written: &'a [u64],
    labels: &'a BTreeMap<String, u64>,
}
impl RowInput<'_> {
    fn arity(&self, count: usize) -> Result<()> {
        if self.node.operands.len() == count {
            Ok(())
        } else {
            Err(Error::arg(format!("§4.2: {} operand count", self.node.op)))
        }
    }
    fn width(&self, name: &str) -> Result<u64> {
        read(name, self.env).map(|w| w.len() as u64)
    }
    fn dest(&self) -> Result<&str> {
        self.node
            .lhs
            .as_deref()
            .ok_or_else(|| Error::arg("missing assignment destination"))
    }
    fn written_width(&self) -> Result<u64> {
        self.written
            .first()
            .copied()
            .ok_or_else(|| Error::arg("missing assignment width"))
    }
    fn label(&self, name: &str) -> Result<u64> {
        name.strip_prefix('^')
            .and_then(|s| self.labels.get(s))
            .copied()
            .ok_or_else(|| Error::arg("unknown oracle label"))
    }
}
fn value_row(
    input: &RowInput<'_>,
    p: u64,
    f: u64,
    slots: &[u64],
) -> Result<Option<(Vec<Value>, bool)>> {
    let node = input.node;
    let operands = &node.operands;
    let written = input.written;
    let mut body = Vec::new();
    let mut flagged = false;
    let flag = slice(p + slots.iter().sum::<u64>(), 1);
    if let Some(suffix) = node.op.strip_prefix("lift.const.i") {
        input.arity(1)?;
        let w = input.written_width()?;
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
                slice(p, w),
                json!(input.dest()?),
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
        input.arity(2)?;
        let w = input.width(&operands[0])?;
        if input.width(&operands[1])? != w
            || input.written_width()?
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
                slice(p, input.written_width()?),
                json!(input.dest()?),
            ],
        ));
    } else if ["lift.udiv", "lift.urem", "lift.sdiv", "lift.srem"].contains(&node.op.as_str()) {
        input.arity(2)?;
        let w = input.width(&operands[0])?;
        if input.width(&operands[1])? != w || input.written_width()? != w {
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
                json!(input.dest()?),
                flag.clone(),
                json!("faulted"),
                json!("code"),
            ],
        ));
        flagged = true;
    } else if node.op == "lift.mov" {
        input.arity(1)?;
        let w = input.width(&operands[0])?;
        if input.written_width()? != w {
            return Err(Error::arg("lift.mov width mismatch"));
        }
        body.push(invoke(
            "lift.mov",
            vec![
                json!(w),
                json!(operands[0]),
                slice(p, w),
                json!(input.dest()?),
            ],
        ));
    } else if ["lift.zext.i", "lift.sext.i", "lift.trunc.i"]
        .iter()
        .any(|s| node.op.starts_with(s))
    {
        input.arity(1)?;
        let (op, suffix) = node.op.rsplit_once(".i").unwrap();
        let m = crate::schedule::uint(&json!(suffix))?;
        let w = input.width(&operands[0])?;
        if m != input.written_width()?
            || (op == "lift.trunc" && m >= w)
            || (op != "lift.trunc" && m <= w)
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
                json!(input.dest()?),
            ],
        ));
    } else if node.op == "lift.select" {
        input.arity(3)?;
        let w = input.width(&operands[1])?;
        if input.width(&operands[0])? != 1
            || input.width(&operands[2])? != w
            || input.written_width()? != w
        {
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
                json!(input.dest()?),
            ],
        ));
    } else if node.op == "lift.parmov" {
        input.arity(1)?;
        let sources = object::list(&operands[0])?;
        let destinations = object::list(input.dest()?)?;
        if sources.len() != written.len() || destinations.len() != sources.len() {
            return Err(Error::arg("parallel move list mismatch"));
        }
        let mut offset = p;
        for ((src, w), slot_width) in sources.iter().zip(written).zip(slots) {
            if input.width(src)? != *w {
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
        for ((dst, w), slot_width) in destinations.iter().zip(written).zip(slots) {
            for bit in 0..*w {
                body.push(
                    json!({"gate":"swap","wires":[slice(offset+bit,1),format!("{dst}[{bit}]")]}),
                );
            }
            offset += slot_width;
        }
    } else if node.op == "lift.label" {
        input.arity(1)?;
        let w = input.written_width()?;
        if w < p {
            return Err(Error::arg(
                "label location cannot represent the index space",
            ));
        }
        body.push(invoke(
            "lift.label",
            vec![
                json!(w),
                json!(input.label(&operands[0])?),
                slice(p, w),
                json!(input.dest()?),
            ],
        ));
    } else {
        return Ok(None);
    }
    Ok(Some((body, flagged)))
}
fn memory_row(
    input: &RowInput<'_>,
    source: &Source<'_>,
    p: u64,
    f: u64,
    slots: &[u64],
) -> Result<Option<(Vec<Value>, bool)>> {
    let node = input.node;
    let operands = &node.operands;
    let mut body = Vec::new();
    let mut flagged = false;
    let flag = slice(p + slots.iter().sum::<u64>(), 1);
    let env = input.env;
    if node.op == "lift.addr" {
        input.arity(1)?;
        let w = input.written_width()?;
        if w != source.object.ptr_width {
            return Err(Error::arg("lift.addr destination must have pointer width"));
        }
        let name = operands[0]
            .strip_prefix('@')
            .ok_or_else(|| Error::arg("lift.addr requires @name"))?;
        let layout = source
            .layout
            .ok_or_else(|| Error::arg("lift.addr requires layout"))?;
        let base = layout
            .globals
            .iter()
            .find(|g| g.name == name)
            .map(|g| &g.base)
            .or_else(|| {
                layout.interface_regions[source.name]
                    .iter()
                    .find(|m| m.name == name)
                    .map(|m| &m.base)
            })
            .ok_or_else(|| {
                Error::arg("lift.addr must name a global or this oracle's mapped region")
            })?;
        body.push(invoke(
            "lift.addr",
            vec![
                json!(w),
                serde_json::from_str(&base.to_string())?,
                slice(p, w),
                json!(input.dest()?),
            ],
        ));
    } else if node.op == "lift.boundscheck" {
        input.arity(2)?;
        let layout = source
            .layout
            .ok_or_else(|| Error::arg("lift.boundscheck requires layout"))?;
        let ptr = source.object.ptr_width;
        if input.width(&operands[0])? != ptr {
            return Err(Error::arg("boundscheck address requires pointer width"));
        }
        let mut literal = None;
        let (length, argument) = if env.contains_key(&operands[1]) {
            (input.width(&operands[1])?, json!(operands[1]))
        } else {
            let value = crate::oracle_memory::literal_length(&operands[1], ptr)?;
            let length = value.bits().max(1);
            let value: Value = serde_json::from_str(&value.to_string())?;
            body.push(
                json!({"alloc":"bounds.length","width":length,"class":"any","state":"clean"}),
            );
            let initialization = invoke(
                "value.literal",
                vec![json!(length), value, json!("bounds.length")],
            );
            body.push(initialization.clone());
            literal = Some(initialization);
            (length, json!("bounds.length"))
        };
        let extended = ptr
            .max(length)
            .checked_add(1)
            .ok_or_else(|| Error::new(Status::NoMem, "bounds comparison exceeds host width"))?;
        let count = 1u64
            .checked_shl(
                u32::try_from(layout.a_tab)
                    .map_err(|_| Error::new(Status::NoMem, "table exponent exceeds host size"))?,
            )
            .ok_or_else(|| Error::new(Status::NoMem, "table exceeds host size"))?;
        body.push(invoke(
            "lift.boundscheck",
            vec![
                json!(ptr),
                json!(length),
                json!(extended),
                json!(count),
                json!(layout.a_tab + 1),
                json!(f),
                json!(operands[0]),
                argument,
                json!("table.base"),
                json!("table.size"),
                json!("table.live"),
                json!("table.pinned"),
                json!("atop"),
                flag.clone(),
                json!("faulted"),
                json!("code"),
            ],
        ));
        if let Some(initialization) = literal {
            body.push(json!({"inverse":true,"body":[initialization]}));
            body.push(json!({"release":"bounds.length"}));
        }
        flagged = true;
    } else if let Some(suffix) = node.op.strip_prefix("lift.load.i") {
        input.arity(1)?;
        let w = crate::schedule::uint(&json!(suffix))?;
        if w == 0
            || w > 128
            || w % 8 != 0
            || input.written_width()? != w
            || input.width(&operands[0])? != source.object.ptr_width
        {
            return Err(Error::arg(
                "lift.load requires a pointer and matching whole-byte result",
            ));
        }
        let layout = source
            .layout
            .ok_or_else(|| Error::arg("lift.load requires layout"))?;
        let ptr = source.object.ptr_width;
        let bytes = w / 8;
        let length = 64 - bytes.leading_zeros() as u64;
        let extended = ptr.max(length).checked_add(1).ok_or_else(|| {
            Error::new(Status::NoMem, "memory comparison width exceeds host size")
        })?;
        let count = 1u64
            .checked_shl(
                u32::try_from(layout.a_tab)
                    .map_err(|_| Error::new(Status::NoMem, "table exponent exceeds host size"))?,
            )
            .ok_or_else(|| Error::new(Status::NoMem, "table exceeds host size"))?;
        body.push(invoke(
            "lift.load",
            vec![
                json!(ptr),
                json!(length),
                json!(extended),
                json!(count),
                json!(layout.a_tab + 1),
                json!(f),
                json!(w),
                json!(bytes),
                json!(operands[0]),
                json!("table.base"),
                json!("table.size"),
                json!("table.live"),
                json!("table.pinned"),
                json!("atop"),
                flag.clone(),
                json!("faulted"),
                json!("code"),
                json!(input.dest()?),
                slice(p, w),
                json!("ByteLayout"),
            ],
        ));
        flagged = true;
    } else {
        return Ok(None);
    }
    Ok(Some((body, flagged)))
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
    source: &Source<'_>,
    library: &mut Library,
    bindings: &mut Env,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut body = Vec::new();
    let operands = &node.operands;
    let written = widths(node, env, locations)?;
    let mut successor = index + 1;
    let flag = slice(p + slots.iter().sum::<u64>(), 1);
    let succ = slice(p + slots.iter().sum::<u64>() + 1, p);
    let mut flagged = false;
    let mut dynamic = false;
    let input = RowInput {
        node,
        env,
        written: &written,
        labels,
    };
    if let Some((operations, uses_flag)) = value_row(&input, p, f, slots)? {
        body = operations;
        flagged = uses_flag;
    } else if let Some((operations, uses_flag)) = memory_row(&input, source, p, f, slots)? {
        body = operations;
        flagged = uses_flag;
    } else if ["lift.alloc", "lift.free"].contains(&node.op.as_str())
        || node.op.starts_with("lift.store.i")
    {
        body.extend(crate::oracle_memory::row(node, source, p, f, slots, env)?);
        flagged = true;
    } else if node.op == "lift.call" || node.op == "lift.ret" {
        let (operation, target, is_dynamic) =
            crate::oracle_activation::row(node, source, p, f, slots, env)?;
        body.push(operation);
        successor = target;
        dynamic = is_dynamic;
        flagged = true;
    } else if node.op == "lift.retval" {
        if source.function.kind != "oracle" {
            return Err(Error::arg("lift.retval inside lproc"));
        }
        input.arity(1)?;
        if input.width(&operands[0])? != t {
            return Err(Error::arg("retval width does not match target"));
        }
        body.push(invoke(
            "lift.retval",
            vec![json!(t), json!(operands[0]), json!("result")],
        ));
        successor = n;
    } else if node.op == "qjump" {
        input.arity(1)?;
        successor = input.label(&operands[0])?;
        body.push(invoke("qjump", vec![json!(successor)]));
    } else if node.op == "qbranch" {
        input.arity(3)?;
        if input.width(&operands[0])? != 1 {
            return Err(Error::arg("qbranch condition must be one bit"));
        }
        body.push(invoke(
            "qbranch",
            vec![
                json!(p),
                json!(input.label(&operands[1])?),
                json!(input.label(&operands[2])?),
                json!(operands[0]),
                succ.clone(),
            ],
        ));
        dynamic = true;
    } else if node.op == "qjumpr" {
        input.arity(1)?;
        if input.width(&operands[0])? != p {
            return Err(Error::arg("qjumpr width must equal canonical P"));
        }
        body.push(invoke(
            "qjumpr",
            vec![
                json!(p),
                json!(f),
                json!(source.first),
                json!(source.function.body.len()),
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
        input.arity(1)?;
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
        input.arity(2)?;
        if input.width(&operands[0])? != 1 {
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
    } else if ["lift.oracle_call", "lift.oracle_uncall", "lift.perm_call"]
        .contains(&node.op.as_str())
    {
        body.push(crate::oracle_calls::row(
            node, index, env, locations, source, library, bindings,
        )?);
    } else {
        return Err(Error::arg(format!(
            "§4.2: unknown or misplaced oracle row {}",
            node.op
        )));
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

fn rename(value: &str, names: &std::collections::BTreeSet<String>, prefix: &str) -> Result<String> {
    if names.contains(value) {
        Ok(format!("{prefix}{value}"))
    } else if value.starts_with('(') {
        Ok(format!(
            "({})",
            object::list(value)?
                .iter()
                .map(|n| rename(n, names, prefix))
                .collect::<Result<Vec<_>>>()?
                .join(", ")
        ))
    } else {
        Ok(value.into())
    }
}

fn qualified_nodes(
    body: &[Node],
    names: &std::collections::BTreeSet<String>,
    prefix: &str,
) -> Result<Vec<Node>> {
    body.iter()
        .map(|original| {
            let mut node = original.clone();
            node.lhs = node
                .lhs
                .as_ref()
                .map(|s| rename(s, names, prefix))
                .transpose()?;
            node.operands = node
                .operands
                .iter()
                .map(|s| rename(s, names, prefix))
                .collect::<Result<_>>()?;
            Ok(node)
        })
        .collect()
}

fn bind_interfaces(
    f: &object::Function,
    params: &Value,
    token_ids: &[u64],
) -> Result<(Env, Vec<Slot>)> {
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
    for (index, param) in f.groups[3].iter().enumerate() {
        let (address, cell) = object::region_shape(&param.ty)?;
        let width = 1u64
            .checked_shl(
                u32::try_from(address)
                    .map_err(|_| Error::new(Status::NoMem, "region exponent exceeds host size"))?,
            )
            .and_then(|n| n.checked_mul(cell))
            .ok_or_else(|| Error::new(Status::NoMem, "region exceeds host size"))?;
        if param.role.as_deref() != Some("in") {
            return Err(Error::arg(
                "§5.1: oracle interface regions must have in role",
            ));
        }
        let id = format!("mem:{index}");
        let mut wires = Vec::new();
        wires
            .try_reserve_exact(
                usize::try_from(width)
                    .map_err(|_| Error::new(Status::NoMem, "region exceeds host size"))?,
            )
            .map_err(|_| Error::new(Status::NoMem, "region exceeds host memory"))?;
        wires.extend((0..width).map(|i| format!("{id}[{i}]")));
        env.insert(
            format!("source.{}", param.name),
            Binding::Region {
                address,
                cell,
                role: "in".into(),
                origin: format!("interface:{}", param.name),
                wires: wires.clone(),
            },
        );
        slots.push(Slot {
            id,
            role: "mem_in".into(),
            wires,
        });
    }
    let mut token_index = 1;
    for (p, value) in f.groups[4].iter().zip(params.as_array().unwrap()) {
        env.insert(
            format!("source.{}", p.name),
            Binding::Value {
                isa_type: json!(p.ty),
                value: value.clone(),
                token_identity: if p.ty == "token" {
                    let id = token_ids[token_index];
                    token_index += 1;
                    Some(id)
                } else {
                    None
                },
            },
        );
    }
    Ok((env, slots))
}

fn compile_instances(
    source: &Source<'_>,
    params: &Value,
    library: &mut Library,
    env: &mut Env,
    rows: &mut Vec<Value>,
    next: &mut Vec<Value>,
    row_points: &mut Vec<(String, String, Vec<u64>)>,
    cleanup_rows: &mut BTreeMap<usize, usize>,
) -> Result<()> {
    let object = source.object;
    let name = source.name;
    let f = source.function;
    let plan = source.plan;
    let layout = source.layout;
    let budget = source.budget;
    let (n, p, q) = (plan.index.halt, plan.index.pc_width, plan.frame_width);
    let fault = object::wire_width(&f.groups[2][0].ty)?;
    let target = object::wire_width(&f.groups[1][0].ty)?;
    let slot_widths = &plan.slot_widths;
    for (instance_index, instance) in plan.index.instances.iter().enumerate() {
        let function = &object.functions[&instance.name];
        let prefix = format!("instance/{instance_index}/source.");
        let fixed = plan.index.resolve_bindings(object, instance, params)?;
        for (i, (parameter, value)) in function.groups[2].iter().zip(fixed.tokens).enumerate() {
            let root = &f.groups[4][instance.key[i] as usize];
            let id =
                crate::token_aliases::binding_identity(&env[&format!("source.{}", root.name)])?;
            env.insert(
                format!("{prefix}{}", parameter.name),
                Binding::Value {
                    isa_type: json!("token"),
                    value,
                    token_identity: Some(id),
                },
            );
        }
        for (parameter, value) in function.groups[3].iter().zip(fixed.classical) {
            let isa_type = if parameter.ty == "ptr" {
                json!({"ptr":object.ptr_width})
            } else {
                json!(parameter.ty)
            };
            env.insert(
                format!("{prefix}{}", parameter.name),
                Binding::Value {
                    isa_type,
                    value,
                    token_identity: None,
                },
            );
        }
        for (parameter, position) in function.groups[4].iter().zip(fixed.regions) {
            let root = f.groups[3]
                .get(position as usize)
                .ok_or_else(|| Error::arg("invalid fixed region position"))?;
            let binding = env
                .get(&format!("source.{}", root.name))
                .ok_or_else(|| Error::arg("missing root mapped region"))?
                .clone();
            env.insert(format!("{prefix}{}", parameter.name), binding);
        }
        let locations = instance
            .locations
            .iter()
            .map(|l| (format!("{prefix}{}", l.name), l.width))
            .collect::<BTreeMap<_, _>>();
        let names = function
            .groups
            .iter()
            .flatten()
            .chain(&function.regs)
            .map(|p| p.name.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let readable = function.groups[..2]
            .iter()
            .flatten()
            .chain(&function.regs)
            .map(|p| format!("{prefix}{}", p.name))
            .chain(
                function.groups[3]
                    .iter()
                    .map(|p| format!("{prefix}{}", p.name)),
            )
            .chain(
                function.groups[2]
                    .iter()
                    .chain(&function.groups[4])
                    .map(|p| format!("{prefix}{}", p.name)),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let source_env = env
            .iter()
            .filter(|(name, _)| readable.contains(*name))
            .map(|(n, b)| (n.clone(), b.clone()))
            .collect::<Env>();
        if rows.len() as u64 != instance.first {
            return Err(Error::arg(
                "lproc rows differ from the canonical index space",
            ));
        }
        let nodes = qualified_nodes(&function.body, &names, &prefix)?;
        let token_origins = function.groups[2]
            .iter()
            .enumerate()
            .map(|(_, p)| {
                let name = format!("{prefix}{}", p.name);
                Ok((
                    name.clone(),
                    crate::token_aliases::binding_identity(&source_env[&name])?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let matches = crate::oracle_pairs::validate(
            &nodes,
            &source_env,
            instance.first,
            &instance.labels,
            &token_origins,
        )?;
        cleanup_rows.extend(matches.into_iter().map(|(uncall, call)| {
            (
                instance.first as usize + uncall,
                instance.first as usize + call,
            )
        }));
        for (original, node) in function.body.iter().zip(&nodes) {
            let (body, successor) = row(
                node,
                rows.len() as u64,
                n,
                p,
                fault,
                target,
                slot_widths,
                &source_env,
                &locations,
                &instance.labels,
                &Source {
                    object,
                    name,
                    layout,
                    plan: &plan,
                    function,
                    instance: Some(instance_index),
                    original,
                    budget,
                    first: instance.first,
                },
                library,
                env,
            )?;
            rows.push(json!({"parameters":[{"name":"frame","type":"wire","width":q}],"body":body}));
            next.push(json!({"parameters":[{"name":"frame","type":"wire","width":q},{"name":"next","type":"wire","width":p}],"body":successor}));
            row_points.push((
                object.symbols[&instance.name].interface_hash.clone(),
                original.point.clone(),
                instance.key.clone(),
            ));
        }
    }
    Ok(())
}

fn project_expansion(
    source: &Source<'_>,
    params: &Value,
    token_ids: &[u64],
    slots: Vec<Slot>,
    inherited: Vec<String>,
    expanded: crate::source_registry::Expanded,
    row_points: &[(String, String, Vec<u64>)],
    cleanup_rows: &BTreeMap<usize, usize>,
) -> Result<Trace> {
    let (object, name, budget) = (source.object, source.name, source.budget);
    let leaves = expanded.leaves;
    let mut counts = BTreeMap::<(u64, usize), u64>::new();
    for leaf in &leaves {
        if leaf.origin.iter().any(|o| o.source.is_some()) {
            continue;
        }
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
                .map(|o| o.index)
                .unwrap_or(usize::MAX);
            *counts.entry((frame.index as u64 + 1, point)).or_default() += 1;
        }
    }
    let mut ordinals = BTreeMap::<(Option<u64>, bool, usize), u64>::new();
    let mut events = Vec::new();
    let mut projection = crate::oracle_calls::Projection::default();
    for call in &expanded.source_calls {
        let (parent, row) = crate::oracle_calls::constructor_location(
            &call.origin,
            &row_points,
            &object.symbols[name].interface_hash,
        );
        projection.register(call, &parent)?;
        if let Some(compute) = cleanup_rows.get(&row) {
            projection.lifted_cleanup(call, &parent, &row_points[*compute].1);
        }
    }
    for mut leaf in leaves {
        // Source callees carry their own State/Frames tags. Only constructors
        // outside the linked source boundary address the containing oracle.
        let boundary = leaf
            .origin
            .iter()
            .rposition(|o| o.source.is_some())
            .map_or(0, |i| i + 1);
        let provenance = &leaf.origin[boundary..];
        let (mut location, space) = crate::oracle_calls::constructor_location(
            provenance,
            &row_points,
            &object.symbols[name].interface_hash,
        );
        let Coordinate { step, phase } = location.root;
        let nested = leaf.origin.iter().any(|o| o.source.is_some());
        let counter = ordinals
            .entry((step, phase == Some(Phase::Rev), space))
            .or_default();
        let ordinal = if nested {
            leaf.ordinal
        } else if phase == Some(Phase::Rev) {
            counts[&(step.unwrap(), space)] - 1 - *counter
        } else {
            *counter
        };
        if !nested {
            *counter += 1;
        }
        leaf.ordinal = ordinal;
        let kind = match &leaf.op {
            Operation::Gate(_) => Kind::Gate,
            Operation::Alloc { .. } => Kind::Alloc,
            Operation::Release { .. } => Kind::Release,
            Operation::Classical { .. } => {
                return Err(Error::arg("classical event in oracle construction"));
            }
        };
        location.generated = ordinal;
        location.kind = kind;
        let mut event = Event {
            owner: 0,
            leaf,
            location,
        };
        if let Some((location, owner)) =
            projection.event(&event.leaf, &event.location, events.len())?
        {
            event.location = location;
            event.owner = owner;
        }
        events.push(event);
    }
    let end = events.len();
    let mut invocations = vec![Invocation {
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
        raw_inverse: false,
        token_ids: token_ids.to_vec(),
        path: vec![],
        containing: Coordinate::default(),
    }];
    invocations.extend(projection.invocations);
    let mut trace = Trace {
        object: std::sync::Arc::new(object.clone()),
        events,
        invocations,
        slots,
        inherited,
        borrow_proofs: projection.borrow_proofs,
        cleanups: projection.cleanups,
        qinplaces: projection.qinplaces,
        replaced_invocations: Default::default(),
    };
    // Source-call proofs were built in callee-local coordinates. Reconstruct
    // them after attaching all invocation paths and raw inverse lifetimes.
    trace.borrow_proofs = crate::borrow::prove(object, &trace)?;
    Ok(trace)
}

const HOST_STATE_BYTES: u64 = 64 * 1024 * 1024;
fn check_workspace(
    f: &object::Function,
    plan: &crate::oracle_state::Plan,
    inherited: u32,
    budget: [u64; 4],
) -> Result<()> {
    let mut wires = plan
        .base
        .iter()
        .try_fold(u64::from(inherited), |a, r| a.checked_add(r.width))
        .and_then(|a| {
            plan.frame_width
                .checked_mul(budget[0])
                .and_then(|b| a.checked_add(b))
        });
    for param in f.groups[..3].iter().flatten() {
        let width = object::wire_width(&param.ty)?;
        wires = wires.and_then(|n| n.checked_add(width));
    }
    for param in &f.groups[3] {
        let (address, cell) = object::region_shape(&param.ty)?;
        let width = u32::try_from(address)
            .ok()
            .and_then(|a| 1u64.checked_shl(a))
            .and_then(|n| n.checked_mul(cell));
        wires = wires.and_then(|n| width.and_then(|w| n.checked_add(w)));
    }
    let estimate = wires
        .and_then(|w| w.checked_mul(128))
        .and_then(|w| budget[0].checked_mul(1024).and_then(|b| w.checked_add(b)));
    if estimate.is_none_or(|n| n > HOST_STATE_BYTES) {
        return Err(Error::new(
            Status::NoMem,
            "oracle construction exceeds 64 MiB host state workspace",
        ));
    }
    Ok(())
}

fn bind_byte_layout(
    object: &Object,
    f: &object::Function,
    plan: &crate::oracle_state::Plan,
    library: &Library,
    env: &mut Env,
) -> Result<()> {
    let table = read("alloctab", &env)?;
    let ptr = usize::try_from(object.ptr_width)
        .map_err(|_| Error::new(Status::NoMem, "pointer exceeds host size"))?;
    let entry = ptr
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .ok_or_else(|| Error::new(Status::NoMem, "table entry exceeds host size"))?;
    let mut base = Vec::new();
    let mut size = Vec::new();
    let mut live = Vec::new();
    let mut pinned = Vec::new();
    for cell in table.chunks_exact(entry) {
        base.extend_from_slice(&cell[..ptr]);
        size.extend_from_slice(&cell[ptr..2 * ptr]);
        live.push(cell[2 * ptr].clone());
        pinned.push(cell[2 * ptr + 1].clone());
    }
    for (name, wires) in [
        ("table.base", base),
        ("table.size", size),
        ("table.live", live),
        ("table.pinned", pinned),
    ] {
        env.insert(name.into(), Binding::Wires(wires));
    }
    let mut records = Vec::new();
    for span in &plan.bytes {
        let (wires, offset) = match span.storage {
            crate::oracle_state::ByteStorage::Ordinary { offset } => (read("mem", &env)?, offset),
            crate::oracle_state::ByteStorage::Interface { position, offset } => {
                let Some(Binding::Region { wires, .. }) =
                    env.get(&format!("source.{}", f.groups[3][position].name))
                else {
                    return Err(Error::arg("missing mapped region binding"));
                };
                (wires.clone(), offset)
            }
        };
        let first = usize::try_from(offset * 8)
            .map_err(|_| Error::new(Status::NoMem, "byte slice exceeds host size"))?;
        let last = usize::try_from((offset + span.extent) * 8)
            .map_err(|_| Error::new(Status::NoMem, "byte slice exceeds host size"))?;
        let cells = wires
            .get(first..last)
            .ok_or_else(|| Error::arg("byte layout does not match source storage"))?
            .to_vec();
        records.push(Env::from([
            ("base".into(), Binding::Integer(span.base.into())),
            ("extent".into(), Binding::Integer(span.extent.into())),
            ("cells".into(), Binding::Wires(cells)),
        ]));
    }
    let schema = library.recipes["lift.load"]["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Layout")
        .unwrap()
        .clone();
    env.insert("ByteLayout".into(), Binding::Records { schema, records });
    Ok(())
}

pub fn build(
    object: &Object,
    name: &str,
    inherited: u32,
    params: &Value,
    budget: [u64; 4],
    token_ids: &[u64],
) -> Result<Trace> {
    let f = &object.functions[name];
    let plan = crate::oracle_state::Plan::form(object, name, budget)?;
    let layout = crate::layout::Layout::from_object(object)?;
    if layout.is_none() && !f.groups[3].is_empty() {
        return Err(Error::arg(
            "memoryless oracle cannot declare interface memory",
        ));
    }
    if layout.is_none() && budget[1..].iter().any(|n| *n != 0) {
        return Err(Error::new(
            Status::Budget,
            "memoryless oracle requires zero memory/stack/depth limits",
        ));
    }
    // Account for all interface and private wire strings before allocating any.
    // The symbolic plan is available independently of this host workspace.
    check_workspace(f, &plan, inherited, budget)?;
    let mut library = Library::published()?;
    let (mut env, slots) = bind_interfaces(f, params, token_ids)?;
    let n = plan.index.halt;
    if n == 0 || f.body.iter().any(|r| !r.bodies.is_empty()) {
        return Err(Error::arg("oracle rows must be nonempty and unstructured"));
    }
    let p = plan.index.pc_width;
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

    let names = f
        .groups
        .iter()
        .flatten()
        .chain(&f.regs)
        .map(|p| p.name.clone())
        .collect::<std::collections::BTreeSet<_>>();

    let nodes = qualified_nodes(&f.body, &names, "source.")?;
    let slot_widths = &plan.slot_widths;
    let q = plan.frame_width;
    let state_schema = library.recipes["oracle.construct"]["parameters"][13].clone();
    let frames_schema = library.recipes["oracle.construct"]["parameters"][14].clone();
    let mut state = Vec::new();
    let mut frames = Vec::new();
    let frame_count = usize::try_from(budget[0])
        .map_err(|_| Error::new(Status::NoMem, "history count exceeds host size"))?;
    state
        .try_reserve_exact(plan.base.len() + frame_count)
        .map_err(|_| Error::new(Status::NoMem, "state descriptors exceed host memory"))?;
    frames
        .try_reserve_exact(frame_count)
        .map_err(|_| Error::new(Status::NoMem, "history descriptors exceed host memory"))?;
    for record in plan.records() {
        let storage = allocation(&record.name, record.width)?;
        env.insert(record.name.clone(), storage.clone());
        let image = if let Some(image) = record.image {
            image
                .materialize(HOST_STATE_BYTES as usize)?
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        } else {
            String::new()
        };
        state.push(Env::from([
            ("width".into(), Binding::Integer(record.width.into())),
            ("initial".into(), Binding::Integer(record.initial.into())),
            ("I".into(), Binding::Image(image)),
            ("storage".into(), storage.clone()),
        ]));
        if let Some(index) = record.name.strip_prefix("history-") {
            let index = index.parse::<u64>().expect("source-derived frame index");
            frames.push(Env::from([
                ("index".into(), Binding::Integer(index.into())),
                ("frame".into(), storage),
            ]));
        }
    }
    if layout.is_some() {
        bind_byte_layout(object, f, &plan, &library, &mut env)?;
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
    let readable = f.groups[0]
        .iter()
        .chain(&f.groups[4])
        .chain(&f.groups[3])
        .chain(&f.regs)
        .map(|p| format!("source.{}", p.name))
        .collect::<std::collections::BTreeSet<_>>();
    let source_env = env
        .iter()
        .filter(|(name, _)| readable.contains(*name))
        .map(|(name, binding)| (name.clone(), binding.clone()))
        .collect::<Env>();
    let token_origins = f.groups[4]
        .iter()
        .enumerate()
        .filter(|(_, p)| p.ty == "token")
        .map(|(_, p)| {
            let name = format!("source.{}", p.name);
            Ok((
                name.clone(),
                crate::token_aliases::binding_identity(&source_env[&name])?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut cleanup_rows =
        crate::oracle_pairs::validate(&nodes, &source_env, 0, &labels, &token_origins)?;
    for (i, node) in nodes.iter().enumerate() {
        let (body, successor) = row(
            node,
            i as u64,
            n,
            p,
            fault,
            target,
            slot_widths,
            &source_env,
            &locations,
            &labels,
            &Source {
                object,
                name,
                layout: layout.as_ref(),
                plan: &plan,
                function: f,
                instance: None,
                original: &f.body[i],
                budget,
                first: 0,
            },
            &mut library,
            &mut env,
        )?;
        rows.push(json!({"parameters":[{"name":"frame","type":"wire","width":q}],"body":body}));
        next.push(json!({"parameters":[{"name":"frame","type":"wire","width":q},{"name":"next","type":"wire","width":p}],"body":successor}));
    }
    let mut row_points = f
        .body
        .iter()
        .map(|node| {
            (
                object.symbols[name].interface_hash.clone(),
                node.point.clone(),
                Vec::<u64>::new(),
            )
        })
        .collect::<Vec<_>>();
    let source = Source {
        object,
        name,
        layout: layout.as_ref(),
        plan: &plan,
        function: f,
        instance: None,
        original: &f.body[0],
        budget,
        first: 0,
    };
    compile_instances(
        &source,
        params,
        &mut library,
        &mut env,
        &mut rows,
        &mut next,
        &mut row_points,
        &mut cleanup_rows,
    )?;

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
    let expanded = library.body_with_sources(
        &[json!({"invoke":"oracle.construct","args":args})],
        &env,
        &inherited,
    )?;
    project_expansion(
        &source,
        params,
        token_ids,
        slots,
        inherited,
        expanded,
        &row_points,
        &cleanup_rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_row_keeps_frame_slice_and_rejects_a_mismatched_destination_width() {
        let node = Node {
            point: "entry:0".into(),
            line: "r = lift.mov x".into(),
            lhs: Some("r".into()),
            op: "lift.mov".into(),
            operands: vec!["x".into()],
            bodies: vec![],
        };
        let env = Env::from([(
            "x".into(),
            Binding::Wires(vec!["x[0]".into(), "x[1]".into()]),
        )]);
        let labels = BTreeMap::new();
        let input = RowInput {
            node: &node,
            env: &env,
            written: &[2],
            labels: &labels,
        };
        assert_eq!(
            value_row(&input, 2, 4, &[2]).unwrap(),
            Some((
                vec![json!({
                    "invoke":"lift.mov", "args":[2,"x",{"range":"frame","from":2,"to":4},"r"]
                })],
                false
            ))
        );
        let invalid = RowInput {
            written: &[1],
            ..input
        };
        let error = value_row(&invalid, 2, 4, &[2]).unwrap_err();
        assert_eq!(error.status, Status::Arg);
        assert!(error.reason.contains("lift.mov width mismatch"));
    }
}
