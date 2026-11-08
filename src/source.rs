//! Source-bound action traces; source coordinates precede target scheme expansion.
use crate::{
    Error, Result, Status, canonical,
    expansion::{self, Binding, Env, Leaf, Library, Operation},
    object::{self, Function, Node, Object},
    schedule::{Coordinate, Kind, Location, Occurrence},
    target::{GATES, Gate},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
mod call_graph;
mod oracle_call;
mod qinplace;
#[derive(Clone, Debug)]
pub struct Slot {
    pub id: String,
    pub role: String,
    pub wires: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Event {
    pub location: Location,
    pub leaf: Leaf,
    pub owner: usize,
}
#[derive(Clone, Debug)]
pub struct Invocation {
    pub name: String,
    pub hash: String,
    pub controls: Vec<String>,
    pub slots: Vec<Slot>,
    pub params: Value,
    pub budget: [u64; 4],
    pub start: usize,
    pub end: usize,
    pub parent: Option<usize>,
    pub adjoint: bool,
    pub raw_inverse: bool,
    pub token_ids: Vec<u64>,
    pub path: Vec<Occurrence>,
    /// Coordinate in the trace's root oracle, outside this invocation's path.
    pub containing: Coordinate,
}
impl Invocation {
    pub fn context_key(&self) -> Value {
        json!(
            self.path
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    let mut value = point.json();
                    let coordinate = if index == 0 {
                        self.containing
                    } else {
                        self.path[index - 1].entered_oracle.unwrap_or_default()
                    };
                    if let Some(step) = coordinate.step {
                        value["step"] = json!(step);
                        value["phase"] =
                            json!(match coordinate.phase.expect("oracle step phase") {
                                crate::schedule::Phase::Fwd => "fwd",
                                crate::schedule::Phase::Rev => "rev",
                            });
                    }
                    value
                })
                .collect::<Vec<_>>()
        )
    }
}
#[derive(Clone, Debug)]
pub struct Trace {
    pub(crate) object: std::sync::Arc<Object>,
    pub events: Vec<Event>,
    pub invocations: Vec<Invocation>,
    pub slots: Vec<Slot>,
    pub inherited: Vec<String>,
    pub borrow_proofs: Vec<crate::borrow::Proof>,
    pub cleanups: Vec<crate::cleanup::Pair>,
    pub qinplaces: Vec<crate::qinplace::Proof>,
    pub(crate) replaced_invocations: BTreeSet<usize>,
}
fn deferred(feature: &str) -> Error {
    Error::new(
        Status::GateUnsupported,
        format!("§11: source binding for {feature} is postponed"),
    )
}
fn union(a: &mut Vec<String>, b: &[String]) {
    for w in b {
        if !a.contains(w) {
            a.push(w.clone());
        }
    }
}
fn live_types(env: &Env) -> BTreeMap<String, Value> {
    env.iter()
        .map(|(name, b)| {
            let ty = match b {
                Binding::Wires(w) => json!(["wire", w.len()]),
                Binding::Allocation {
                    wires,
                    class,
                    state,
                    ..
                } => json!(["allocation", wires.len(), class, state]),
                Binding::Region {
                    address,
                    cell,
                    role,
                    origin,
                    ..
                } => json!(["region", address, cell, role, origin]),
                Binding::Value { isa_type, .. } => json!(["value", isa_type]),
                _ => json!("internal"),
            };
            (name.clone(), ty)
        })
        .collect()
}
fn region_width(address: u64, cell: u64) -> Result<usize> {
    1usize
        .checked_shl(u32::try_from(address).unwrap_or(u32::MAX))
        .and_then(|c| c.checked_mul(usize::try_from(cell).ok()?))
        .ok_or_else(|| {
            Error::new(
                Status::NoMem,
                "region width exceeds host materialisation size",
            )
        })
}
fn region_wires(identity: &str, width: usize) -> Result<Vec<String>> {
    let mut wires = Vec::new();
    wires
        .try_reserve_exact(width)
        .map_err(|_| Error::new(Status::NoMem, "region wire list exceeds host memory"))?;
    wires.extend((0..width).map(|i| format!("{identity}[{i}]")));
    Ok(wires)
}
fn wires(operand: &str, env: &Env) -> Result<Vec<String>> {
    if let Some((name, subscript)) = operand.split_once('[') {
        let i = subscript
            .strip_suffix(']')
            .ok_or_else(|| Error::arg("invalid indexed wire"))?
            .parse::<usize>()
            .map_err(|_| Error::arg("invalid wire index"))?;
        let w = wires(name, env)?;
        return w
            .get(i)
            .cloned()
            .map(|w| vec![w])
            .ok_or_else(|| Error::arg("wire index outside bundle"));
    }
    match env.get(operand) {
        Some(Binding::Wires(w) | Binding::Allocation { wires: w, .. }) => Ok(w.clone()),
        _ => Err(Error::arg(format!("unbound source wire {operand}"))),
    }
}
fn location(
    function: &Function,
    node: &Node,
    iteration: &[u64],
    chain: &[Occurrence],
    ordinal: u64,
    kind: Kind,
) -> Location {
    let mut path = chain.to_vec();
    path.push(Occurrence {
        symbol: function
            .signature
            .get("interface_hash")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        point: node.point.clone(),
        iteration: iteration.to_vec(),
        instance: vec![],
        call_index: None,
        entered_oracle: None,
    });
    Location {
        root: Coordinate::default(),
        path,
        generated: ordinal,
        kind,
    }
}
impl Trace {
    pub fn build(
        object: &Object,
        name: &str,
        inherited: u32,
        params: &Value,
        budget: [u64; 4],
    ) -> Result<Self> {
        Self::build_oriented(object, name, inherited, params, budget, false)
    }
    pub(crate) fn build_oriented(
        object: &Object,
        name: &str,
        inherited: u32,
        params: &Value,
        budget: [u64; 4],
        adjoint: bool,
    ) -> Result<Self> {
        object.validate_params(name, params)?;
        let f = object
            .functions
            .get(name)
            .ok_or_else(|| Error::arg("unknown source region"))?;
        let ids = crate::token_aliases::resolve(f, params, budget, None)?;
        Self::build_with_token_ids(object, name, inherited, params, budget, adjoint, &ids)
    }
    pub fn build_with_token_aliases(
        object: &Object,
        name: &str,
        inherited: u32,
        params: &Value,
        budget: [u64; 4],
        aliases: &[u64],
    ) -> Result<Self> {
        object.validate_params(name, params)?;
        let f = object
            .functions
            .get(name)
            .ok_or_else(|| Error::arg("unknown source region"))?;
        let ids = crate::token_aliases::resolve(f, params, budget, Some(aliases))?;
        Self::build_with_token_ids(object, name, inherited, params, budget, false, &ids)
    }
    pub(crate) fn build_with_token_ids(
        object: &Object,
        name: &str,
        inherited: u32,
        params: &Value,
        budget: [u64; 4],
        adjoint: bool,
        token_ids: &[u64],
    ) -> Result<Self> {
        object.validate_params(name, params)?;
        object.validate_budget(budget)?;
        let f = object
            .functions
            .get(name)
            .ok_or_else(|| Error::arg("unknown source region"))?;
        if !["oracle", "action", "perm_action"].contains(&f.kind.as_str()) {
            return Err(Error::arg("§11: root must be an oracle or action"));
        }
        crate::token_aliases::validate_ids(f, params, budget, token_ids)?;
        for tuple in crate::token_aliases::tuples(f, params, budget)? {
            object.validate_budget(tuple)?;
        }
        call_graph::validate(object, name)?;
        if f.kind == "oracle" {
            return crate::oracle::build(object, name, inherited, params, budget, token_ids);
        }
        let mut env = Env::new();
        let mut slots = Vec::new();
        for (group, role) in [(0, "mut"), (1, "in")] {
            for (i, p) in f.groups[group].iter().enumerate() {
                let width = object::wire_width(&p.ty)?;
                let id = format!("{role}:{i}");
                let w = (0..width)
                    .map(|bit| format!("{id}[{bit}]"))
                    .collect::<Vec<_>>();
                env.insert(p.name.clone(), Binding::Wires(w.clone()));
                slots.push(Slot {
                    id,
                    role: role.into(),
                    wires: w,
                });
            }
        }
        for (i, p) in f.groups[2].iter().enumerate() {
            let (address, cell) = object::region_shape(&p.ty)?;
            let role = p
                .role
                .as_deref()
                .ok_or_else(|| Error::arg("region interface requires a role"))?;
            let width = region_width(address, cell)?;
            let id = format!("mem:{i}");
            let w = region_wires(&id, width)?;
            env.insert(
                p.name.clone(),
                Binding::Region {
                    address,
                    cell,
                    role: role.into(),
                    origin: format!("interface:{}", p.name),
                    wires: w.clone(),
                },
            );
            slots.push(Slot {
                id,
                role: format!("mem_{role}"),
                wires: w,
            });
        }
        let mut token_index = 0;
        for (p, v) in f.groups[3]
            .iter()
            .zip(params.as_array().expect("validated params"))
        {
            env.insert(
                p.name.clone(),
                Binding::Value {
                    isa_type: json!(p.ty),
                    value: v.clone(),
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
        let inherited = (0..inherited)
            .map(|i| format!("control:{i}"))
            .collect::<Vec<_>>();
        let mut trace = Self {
            object: std::sync::Arc::new(object.clone()),
            events: vec![],
            invocations: vec![],
            slots: slots.clone(),
            inherited: inherited.clone(),
            borrow_proofs: vec![],
            cleanups: vec![],
            qinplaces: vec![],
            replaced_invocations: BTreeSet::new(),
        };
        let lib = Library::published()?;
        let mut builder = Builder {
            object,
            library: &lib,
            trace: &mut trace,
            active: BTreeSet::new(),
            adjoint_depth: 0,
            replacements: BTreeMap::new(),
        };
        builder.invoke(
            f,
            env,
            inherited,
            slots,
            params.clone(),
            budget,
            &[],
            adjoint,
            None,
        )?;
        if f.kind=="perm_action"&&trace.events.iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if !["x","cx","ccx","mcx","swap"].contains(&g.op.as_str()))){return Err(Error::arg("§5.3: permutation actions require permutation gates transitively"));}
        trace.borrow_proofs = crate::borrow::prove(object, &trace)?;
        for owner in 0..trace.invocations.len() {
            if object.functions[&trace.invocations[owner].name].kind == "oracle" {
                // The implicit full-history construction and its inverse are
                // supplied by the bound library, as for standalone roots.
                continue;
            }
            let events = crate::source_ir::events(object, &trace, owner)?;
            let (matching, permutation) = verify_clean_lifetimes(&events)?;
            trace.cleanups.extend(crate::cleanup::partition(
                owner,
                &events,
                &matching,
                &permutation,
            ));
        }
        Ok(trace)
    }
}
struct Builder<'a> {
    object: &'a Object,
    library: &'a Library,
    trace: &'a mut Trace,
    active: BTreeSet<String>,
    adjoint_depth: usize,
    replacements: BTreeMap<InstructionKey, Vec<Leaf>>,
}
#[derive(Clone, Copy)]
struct InstructionContext<'a> {
    f: &'a Function,
    node: &'a Node,
    chain: &'a [Occurrence],
    iteration: &'a [u64],
    owner: usize,
}
type InstructionKey = (usize, String, Vec<u64>);
fn instruction_key(event: &Event) -> InstructionKey {
    let occurrence = event.location.path.last().expect("source instruction path");
    (
        event.owner,
        occurrence.point.clone(),
        occurrence.iteration.clone(),
    )
}
fn rebase_leaf(leaf: &mut Leaf, prefix: &str) -> Kind {
    fn rebase(wire: &mut String, prefix: &str) {
        if wire.starts_with("scratch/") || wire.starts_with("declaration/") {
            *wire = format!("{prefix}/{wire}");
        }
    }
    match &mut leaf.op {
        Operation::Gate(gate) => {
            for wire in gate.wires.iter_mut().chain(&mut gate.controls) {
                rebase(wire, prefix);
            }
            Kind::Gate
        }
        Operation::Alloc {
            identity, wires, ..
        }
        | Operation::Release {
            identity, wires, ..
        } => {
            rebase(identity, prefix);
            for wire in wires {
                rebase(wire, prefix);
            }
            if matches!(leaf.op, Operation::Alloc { .. }) {
                Kind::Alloc
            } else {
                Kind::Release
            }
        }
        Operation::Classical { .. } => Kind::Classical,
    }
}
struct LocalRegion {
    identity: String,
    wires: Vec<String>,
    image: Vec<Gate>,
}
fn support_point(point: &str) -> Node {
    Node {
        point: point.into(),
        line: String::new(),
        lhs: None,
        op: "qregion".into(),
        operands: vec![],
        bodies: vec![],
    }
}
struct MemoryBinding {
    kind: String,
    args: Value,
    consumed: Option<String>,
}
fn bind_memory(
    node: &Node,
    env: &mut Env,
    readonly: &BTreeSet<String>,
    identity: &str,
) -> Result<MemoryBinding> {
    let op = node.op.as_str();
    let operands = &node.operands;
    let expected = if op == "qload" { 2 } else { 3 };
    if operands.len() != expected {
        return Err(Error::arg("§4.6: coherent memory operand count mismatch"));
    }
    let Some(Binding::Region {
        address,
        cell,
        role,
        origin,
        wires: cells,
    }) = env.get(&operands[0]).cloned()
    else {
        return Err(Error::arg("coherent memory requires a region operand"));
    };
    let address_wires = if address == 0 {
        if operands[1] != "()" {
            return Err(Error::arg("§4.6/K79: zero-address region requires ()"));
        }
        vec![]
    } else {
        if operands[1] == "()" {
            return Err(Error::arg(
                "§4.6/K79: nonzero address width requires a wire bundle",
            ));
        }
        let wires = wires(&operands[1], env)?;
        if wires.len() as u64 != address {
            return Err(Error::arg("§4.6: address bundle width mismatch"));
        }
        wires
    };
    let cells_count = cells.len() as u64 / cell;
    let a = if address == 0 {
        json!([])
    } else {
        json!(operands[1])
    };
    let source_key = canonical::hash(&canonical::to_vec(&json!([origin, cells, address_wires]))?);
    let mut consumed = None;
    let (kind, value) = if op == "qload" {
        let name = node
            .lhs
            .as_ref()
            .filter(|n| object::name(n))
            .ok_or_else(|| Error::arg("qload requires one result name"))?;
        if env.contains_key(name) {
            return Err(Error::arg("qload result shadows a live name"));
        }
        let id = format!("source-load/{identity}/{source_key}");
        let w = region_wires(
            &id,
            usize::try_from(cell)
                .map_err(|_| Error::new(Status::NoMem, "qload result exceeds host size"))?,
        )?;
        env.insert(
            name.clone(),
            Binding::Allocation {
                identity: id,
                wires: w,
                class: "any".into(),
                state: "clean".into(),
            },
        );
        (format!("qload.{role}"), name.clone())
    } else {
        if node.lhs.is_some() {
            return Err(Error::arg("memory update/release has no result"));
        }
        let value = &operands[2];
        let w = wires(value, env)?;
        if w.len() as u64 != cell {
            return Err(Error::arg("§4.6: coherent memory value width mismatch"));
        }
        if op == "qunload" {
            let Some(Binding::Allocation { identity, .. }) = env.get(value) else {
                return Err(Error::arg("qunload consumes a qload result"));
            };
            if !identity.starts_with("source-load/") || !identity.ends_with(&source_key) {
                return Err(Error::arg(
                    "§11 K81: qunload must pair with its own region/address qload",
                ));
            }
            consumed = Some(value.clone());
            (format!("qunload.{role}"), value.clone())
        } else {
            if role != "mut" {
                return Err(Error::arg("§5.1: memory write requires a mutable region"));
            }
            if op == "qexchm" && w.iter().any(|wire| readonly.contains(wire)) {
                return Err(Error::arg("qexchm cannot write an in wire"));
            }
            (op.into(), value.clone())
        }
    };
    let args = json!([address, cells_count, cell, a, operands[0], value]);
    Ok(MemoryBinding {
        kind,
        args,
        consumed,
    })
}

/// Bind the whole source before expanding any possibly enz-dependent helper,
/// so a later malformed address is not hidden by an earlier blocked qload.
fn preflight_memory(nodes: &[Node], env: &mut Env, readonly: &BTreeSet<String>) -> Result<()> {
    for node in nodes {
        match node.op.as_str() {
            "qload" | "qunload" | "qstore" | "qexchm" => {
                let plan = bind_memory(node, env, readonly, &format!("preflight/{}", node.point))?;
                if let Some(name) = plan.consumed {
                    env.remove(&name);
                }
            }
            "qcopy" => {
                let src = node
                    .operands
                    .first()
                    .ok_or_else(|| Error::arg("qcopy source missing"))?;
                let w = wires(src, env)?;
                let name = node
                    .lhs
                    .as_ref()
                    .ok_or_else(|| Error::arg("qcopy result missing"))?;
                if env.contains_key(name) {
                    return Err(Error::arg("qcopy result shadows a live name"));
                }
                let hash = canonical::hash(&canonical::to_vec(&json!(w))?);
                let identity = format!("source-copy/preflight/{hash}");
                env.insert(
                    name.clone(),
                    Binding::Allocation {
                        wires: region_wires(&identity, w.len())?,
                        identity,
                        class: "any".into(),
                        state: "clean".into(),
                    },
                );
            }
            "quncopy" => {
                if node.operands.len() != 2 {
                    return Err(Error::arg("quncopy operand count"));
                }
                let w = wires(&node.operands[0], env)?;
                let hash = canonical::hash(&canonical::to_vec(&json!(w))?);
                let Some(Binding::Allocation { identity, .. }) = env.remove(&node.operands[1])
                else {
                    return Err(Error::arg("quncopy consumes a qcopy result"));
                };
                if !identity.starts_with("source-copy/") || !identity.ends_with(&hash) {
                    return Err(Error::arg("§11 K81: quncopy must pair with its own qcopy"));
                }
            }
            "qalloc" | "qborrow" => {
                if node.operands.len() != 2 {
                    return Err(Error::arg("allocation operand count"));
                }
                let n = crate::schedule::uint(&json!(node.operands[1]))?;
                if n == 0 {
                    return Err(Error::arg("allocation width must be positive"));
                }
                let name = node
                    .lhs
                    .as_ref()
                    .ok_or_else(|| Error::arg("allocation result missing"))?;
                if env.contains_key(name) {
                    return Err(Error::arg("allocation shadows a live name"));
                }
                let identity = format!("preflight/{}", node.point);
                env.insert(
                    name.clone(),
                    Binding::Allocation {
                        wires: region_wires(
                            &identity,
                            usize::try_from(n).map_err(|_| {
                                Error::new(Status::NoMem, "allocation exceeds host size")
                            })?,
                        )?,
                        identity,
                        class: node.operands[0].clone(),
                        state: if node.op == "qborrow" {
                            "borrowed"
                        } else {
                            "clean"
                        }
                        .into(),
                    },
                );
            }
            "qfree" | "qreturn" => {
                if node.operands.len() != 1 {
                    return Err(Error::arg("release requires one allocation"));
                }
                let Some(Binding::Allocation { state, .. }) = env.remove(&node.operands[0]) else {
                    return Err(Error::arg("release of a non-live source allocation"));
                };
                if (node.op == "qreturn") != (state == "borrowed") {
                    return Err(Error::arg(
                        "§5.1: qfree matches qalloc and qreturn matches qborrow",
                    ));
                }
            }
            "qloop" => {
                if node.bodies.len() != 1 {
                    return Err(Error::arg("qloop requires exactly one body"));
                }
                let mut inner = env.clone();
                preflight_memory(&node.bodies[0], &mut inner, readonly)?;
                if live_types(&inner) != live_types(env) {
                    return Err(Error::arg("§4.6: loop body entry/exit live names differ"));
                }
            }
            _ => {
                for body in &node.bodies {
                    preflight_memory(body, env, readonly)?;
                }
            }
        }
    }
    Ok(())
}
impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn invoke(
        &mut self,
        f: &Function,
        mut env: Env,
        controls: Vec<String>,
        slots: Vec<Slot>,
        params: Value,
        budget: [u64; 4],
        chain: &[Occurrence],
        adjoint: bool,
        parent: Option<usize>,
    ) -> Result<()> {
        let bounds = object::flattened(&f.body)
            .into_iter()
            .filter(|n| n.op == "qloop")
            .map(|n| {
                crate::schedule::uint(&json!(
                    n.operands
                        .first()
                        .ok_or_else(|| Error::arg("missing qloop bound"))?
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        if f.signature["bounds"] != json!(bounds) {
            return Err(Error::arg(
                "§5.3: action bounds must equal source loop trip counts",
            ));
        }
        if !self.active.insert(f.name.clone()) {
            return Err(Error::arg("§3.1: cyclic action call graph"));
        }
        self.adjoint_depth += usize::from(adjoint);
        let owner = self.trace.invocations.len();
        let start = self.trace.events.len();
        let token_ids = f.groups[3]
            .iter()
            .filter(|p| p.ty == "token")
            .map(|p| crate::token_aliases::binding_identity(&env[&p.name]))
            .collect::<Result<Vec<_>>>()?;
        crate::token_aliases::validate_ids(f, &params, budget, &token_ids)?;
        self.trace.invocations.push(Invocation {
            name: f.name.clone(),
            hash: self.object.closure_hash(&f.name)?.into(),
            controls: controls.clone(),
            slots,
            params,
            budget,
            start,
            end: start,
            parent,
            adjoint: false,
            raw_inverse: false,
            token_ids,
            path: chain.to_vec(),
            containing: Coordinate::default(),
        });
        let locals = self.prepare_regions(f, &mut env, chain, owner)?;
        let readonly = self.trace.invocations[owner]
            .slots
            .iter()
            .filter(|s| s.role == "in" || s.role == "mem_in")
            .flat_map(|s| s.wires.iter().cloned())
            .collect::<BTreeSet<_>>();
        preflight_memory(&f.body, &mut env.clone(), &readonly)?;
        self.nodes(f, &f.body, &mut env, &controls, chain, &[], owner)?;
        let release = support_point("$release");
        let mut ordinal = 0;
        for region in locals.into_iter().rev() {
            let mut leaves = Vec::new();
            for gate in region.image.into_iter().rev() {
                leaves.push(Leaf {
                    ordinal,
                    calls: vec![],
                    origin: vec![],
                    op: Operation::Gate(gate),
                });
                ordinal += 1;
            }
            leaves.push(Leaf {
                ordinal,
                calls: vec![],
                origin: vec![],
                op: Operation::Release {
                    identity: region.identity,
                    wires: region.wires,
                    class: "any".into(),
                    state: "clean".into(),
                },
            });
            ordinal += 1;
            self.emit(f, &release, &[], chain, owner, leaves)?;
        }
        if adjoint {
            // §5.3 replaces oracle_call by the same involution. Preserve its
            // entire forward construction, including any descendants, while
            // reversing the enclosing action's source instruction order.
            let old_spans = self
                .trace
                .invocations
                .iter()
                .map(|i| (i.start, i.end))
                .collect::<Vec<_>>();
            let mut frozen = BTreeMap::new();
            for index in owner..self.trace.invocations.len() {
                let inv = &self.trace.invocations[index];
                let top = inv
                    .parent
                    .and_then(|p| frozen.get(&p).copied())
                    .or_else(|| {
                        (self.object.functions[&inv.name].kind == "oracle").then_some(index)
                    });
                if let Some(top) = top {
                    frozen.insert(index, top);
                }
            }
            for (index, inv) in self.trace.invocations.iter_mut().enumerate().skip(owner) {
                if !frozen.contains_key(&index) {
                    inv.adjoint = !inv.adjoint;
                }
            }
            let end = self.trace.events.len();
            for inv in self.trace.invocations.iter_mut().skip(owner + 1) {
                let (old_start, old_end) = (inv.start, inv.end);
                inv.start = start + end - old_end;
                inv.end = start + end - old_start;
            }
            let mut events = self.trace.events.split_off(start);
            events.reverse();
            let mut at = 0;
            while at < events.len() {
                if let Some(top) = frozen.get(&events[at].owner).copied() {
                    let end = at
                        + events[at..]
                            .iter()
                            .take_while(|e| frozen.get(&e.owner) == Some(&top))
                            .count();
                    events[at..end].reverse();
                    for (&index, &root) in &frozen {
                        if root == top {
                            self.trace.invocations[index].start =
                                start + at + old_spans[index].0 - old_spans[top].0;
                            self.trace.invocations[index].end =
                                start + at + old_spans[index].1 - old_spans[top].0;
                        }
                    }
                    at = end;
                    continue;
                }
                let key = instruction_key(&events[at]);
                if let Some(replacement) = self.replacements.remove(&key) {
                    // The counterpart was instantiated forward from its source
                    // replacement with the same resolved lexical bindings.
                    let end = at + replacement.len();
                    if end > events.len()
                        || events[at..end].iter().any(|e| instruction_key(e) != key)
                    {
                        return Err(Error::arg("source replacement expansion span mismatch"));
                    }
                    let previous = events[at..end]
                        .iter()
                        .rev()
                        .map(|e| e.leaf.clone())
                        .collect();
                    for (event, leaf) in events[at..end].iter_mut().zip(replacement) {
                        event.location.kind = match &leaf.op {
                            Operation::Gate(_) => Kind::Gate,
                            Operation::Alloc { .. } => Kind::Alloc,
                            Operation::Release { .. } => Kind::Release,
                            Operation::Classical { .. } => Kind::Classical,
                        };
                        event.leaf = leaf;
                    }
                    self.replacements.insert(key, previous);
                    at = end;
                    continue;
                }
                let e = &mut events[at];
                e.leaf = expansion::inverse(std::slice::from_ref(&e.leaf))
                    .pop()
                    .unwrap();
                e.location.kind = match &e.leaf.op {
                    Operation::Gate(_) => Kind::Gate,
                    Operation::Alloc { .. } => Kind::Alloc,
                    Operation::Release { .. } => Kind::Release,
                    Operation::Classical { .. } => Kind::Classical,
                };
                // K83 source adjoints form fresh forward entry/exit support
                // domains. Raw expansion::inverse retains its old addresses.
                if self.object.functions[&self.trace.invocations[e.owner].name].kind != "oracle" {
                    let point = &mut e.location.path.last_mut().unwrap().point;
                    match point.as_str() {
                        "$construct" => *point = "$release".into(),
                        "$release" => *point = "$construct".into(),
                        _ => {}
                    }
                }
                at += 1;
            }
            let old_keys = events.iter().map(instruction_key).collect::<Vec<_>>();
            remap_adjoint_iterations(
                &mut events,
                &mut self.trace.invocations[owner + 1..],
                self.object,
                chain.len(),
            )?;
            let mut moved = Vec::new();
            for (old, event) in old_keys.into_iter().zip(&events) {
                if let Some(leaves) = self.replacements.remove(&old) {
                    moved.push((instruction_key(event), leaves));
                }
            }
            self.replacements.extend(moved);
            qinplace::form_adjoint(
                &mut events,
                &mut self.trace.invocations,
                &self.trace.qinplaces,
                owner,
                &frozen,
            )?;
            form_while_adjoint(&mut events, &self.trace.invocations, &frozen)?;
            let mut ordinals = BTreeMap::new();
            for e in &mut events {
                if frozen.contains_key(&e.owner) {
                    continue;
                }
                let key = canonical::to_vec(&json!(
                    e.location
                        .path
                        .iter()
                        .map(Occurrence::json)
                        .collect::<Vec<_>>()
                ))?;
                let n = ordinals.entry(key).or_insert(0);
                e.location.generated = *n;
                e.leaf.ordinal = *n;
                *n += 1;
            }
            self.trace.events.extend(events);
            for index in owner + 1..self.trace.invocations.len() {
                let indices = self.trace.events[start..]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        is_descendant(e.owner, index, &self.trace.invocations).then_some(start + i)
                    })
                    .collect::<Vec<_>>();
                if let (Some(first), Some(last)) = (indices.first(), indices.last()) {
                    self.trace.invocations[index].start = *first;
                    self.trace.invocations[index].end = last + 1;
                }
            }
        }
        self.trace.invocations[owner].end = self.trace.events.len();
        self.active.remove(&f.name);
        self.adjoint_depth -= usize::from(adjoint);
        Ok(())
    }
    fn emit(
        &mut self,
        f: &Function,
        node: &Node,
        iteration: &[u64],
        chain: &[Occurrence],
        owner: usize,
        leaves: Vec<Leaf>,
    ) -> Result<()> {
        let prefix = canonical::hash(&canonical::to_vec(&json!([
            chain.iter().map(Occurrence::json).collect::<Vec<_>>(),
            node.point,
            iteration
        ]))?);
        for mut leaf in leaves {
            let kind = rebase_leaf(&mut leaf, &prefix);
            let mut loc = location(f, node, iteration, chain, leaf.ordinal, kind);
            loc.path.last_mut().unwrap().symbol =
                self.object.symbols[&f.name].interface_hash.clone();
            self.trace.events.push(Event {
                location: loc,
                leaf,
                owner,
            });
        }
        Ok(())
    }
    fn recipe(&self, kind: &str, args: Value, env: &Env, controls: &[String]) -> Result<Vec<Leaf>> {
        self.library
            .body(&[json!({"invoke":kind,"args":args})], env, controls)
    }
    fn prepare_regions(
        &mut self,
        f: &Function,
        env: &mut Env,
        chain: &[Occurrence],
        owner: usize,
    ) -> Result<Vec<LocalRegion>> {
        let declarations = object::flattened(&f.body)
            .into_iter()
            .filter(|n| n.op == "qregion")
            .collect::<Vec<_>>();
        let mut regions = Vec::new();
        let construct = support_point("$construct");
        let mut ordinal = 0;
        for node in declarations {
            if node.lhs.is_some()
                || !node.bodies.is_empty()
                || !(3..=4).contains(&node.operands.len())
            {
                return Err(Error::arg(
                    "§4.6: qregion requires name, address width, cell width and optional image",
                ));
            }
            let name = &node.operands[0];
            if !object::name(name) || env.contains_key(name) {
                return Err(Error::arg("duplicate or invalid local region name"));
            }
            let address = crate::schedule::uint(&json!(node.operands[1]))?;
            let cell = crate::schedule::uint(&json!(node.operands[2]))?;
            if cell == 0 {
                return Err(Error::arg("region cell width must be positive"));
            }
            let width = 1usize
                .checked_shl(u32::try_from(address).unwrap_or(u32::MAX))
                .and_then(|c| c.checked_mul(usize::try_from(cell).ok()?))
                .ok_or_else(|| {
                    Error::new(
                        Status::NoMem,
                        "region width exceeds host materialisation size",
                    )
                })?;
            let identity = format!(
                "source-region/{}/{name}",
                canonical::hash(&canonical::to_vec(&json!(
                    chain.iter().map(Occurrence::json).collect::<Vec<_>>()
                ))?)
            );
            let mut w = Vec::new();
            w.try_reserve_exact(width)
                .map_err(|_| Error::new(Status::NoMem, "region wire list exceeds host memory"))?;
            w.extend((0..width).map(|i| format!("{identity}[{i}]")));
            let image = if let Some(symbol) = node.operands.get(3) {
                let image = symbol
                    .strip_prefix('@')
                    .and_then(|n| self.object.data.get(n))
                    .ok_or_else(|| Error::arg("qregion image must name a linked data symbol"))?;
                if width % 8 != 0 || image.len() != width / 4 {
                    return Err(Error::arg(
                        "§4.6: region image size must equal its complete byte width",
                    ));
                }
                image.as_str()
            } else {
                ""
            };
            let mut gates = Vec::new();
            for (byte, hex) in image.as_bytes().chunks(2).enumerate() {
                let value = u8::from_str_radix(std::str::from_utf8(hex).unwrap(), 16)
                    .map_err(|_| Error::arg("invalid region image"))?;
                for bit in 0..8 {
                    if value & (1 << bit) != 0 {
                        gates.push(Gate {
                            op: "x".into(),
                            wires: vec![w[8 * byte + bit].clone()],
                            controls: vec![],
                            theta: None,
                        });
                    }
                }
            }
            let mut leaves = vec![Leaf {
                ordinal,
                calls: vec![],
                origin: vec![],
                op: Operation::Alloc {
                    identity: identity.clone(),
                    wires: w.clone(),
                    class: "any".into(),
                    state: "clean".into(),
                },
            }];
            leaves.extend(gates.iter().enumerate().map(|(i, g)| Leaf {
                ordinal: ordinal + i as u64 + 1,
                calls: vec![],
                origin: vec![],
                op: Operation::Gate(g.clone()),
            }));
            ordinal += leaves.len() as u64;
            self.emit(f, &construct, &[], chain, owner, leaves)?;
            env.insert(
                name.clone(),
                Binding::Region {
                    address,
                    cell,
                    role: "mut".into(),
                    origin: format!("local:{name}"),
                    wires: w.clone(),
                },
            );
            regions.push(LocalRegion {
                identity,
                wires: w,
                image: gates,
            });
        }
        Ok(regions)
    }
    fn expand_loop(
        &mut self,
        context: InstructionContext<'_>,
        operands: &[String],
        env: &mut Env,
        effective: Vec<String>,
    ) -> Result<()> {
        let InstructionContext {
            f,
            node,
            chain,
            iteration,
            owner,
        } = context;
        if operands.len() != 1 && !(operands.len() == 3 && operands[1] == "while") {
            return Err(Error::arg(
                "qloop requires a bound and optional while condition",
            ));
        }
        let n = crate::schedule::uint(&json!(operands[0]))?;
        if node.bodies.len() != 1 {
            return Err(Error::arg("qloop requires one body"));
        }
        let condition = if operands.len() == 3 {
            let w = wires(&operands[2], env)?;
            if w.len() as u64 != n {
                return Err(Error::arg(
                    "§4.6: while condition width must equal trip count",
                ));
            }
            Some(w)
        } else {
            None
        };
        let identity = format!(
            "source-flags/{}/{}/{:?}",
            canonical::hash(&canonical::to_vec(&json!(
                chain.iter().map(Occurrence::json).collect::<Vec<_>>()
            ))?),
            node.point,
            iteration
        );
        let flags = (0..n)
            .map(|i| format!("{identity}[{i}]"))
            .collect::<Vec<_>>();
        let mut ordinal = 0;
        if let Some(condition) = &condition {
            self.emit(
                f,
                node,
                iteration,
                chain,
                owner,
                vec![Leaf {
                    ordinal,
                    calls: vec![],
                    origin: vec![],
                    op: Operation::Alloc {
                        identity: identity.clone(),
                        wires: flags.clone(),
                        class: "any".into(),
                        state: "clean".into(),
                    },
                }],
            )?;
            ordinal += 1;
            if n > 0 {
                self.emit(
                    f,
                    node,
                    iteration,
                    chain,
                    owner,
                    vec![Leaf {
                        ordinal,
                        calls: vec![],
                        origin: vec![],
                        op: Operation::Gate(Gate {
                            op: "cx".into(),
                            wires: vec![condition[0].clone(), flags[0].clone()],
                            controls: effective.clone(),
                            theta: None,
                        }),
                    }],
                )?;
                ordinal += 1;
            }
        }
        for i in 0..n {
            let mut counts = iteration.to_vec();
            counts.push(i);
            let before = live_types(env);
            let mut controls = effective.clone();
            if condition.is_some() {
                union(&mut controls, &[flags[i as usize].clone()]);
            }
            let start = self.trace.events.len();
            self.nodes(f, &node.bodies[0], env, &controls, chain, &counts, owner)?;
            if before != live_types(env) {
                return Err(Error::arg(
                    "§4.6: loop body entry and exit live names/types must agree",
                ));
            }
            if let Some(condition) = &condition {
                if self.trace.events[start..].iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if g.wires[g.own_controls()..].iter().any(|w|condition.contains(w)))){return Err(Error::arg("§3.3: while condition modified inside loop body"));}
                if i + 1 < n {
                    self.emit(
                        f,
                        node,
                        iteration,
                        chain,
                        owner,
                        vec![Leaf {
                            ordinal,
                            calls: vec![],
                            origin: vec![],
                            op: Operation::Gate(Gate {
                                op: "ccx".into(),
                                wires: vec![
                                    flags[i as usize].clone(),
                                    condition[i as usize + 1].clone(),
                                    flags[i as usize + 1].clone(),
                                ],
                                controls: effective.clone(),
                                theta: None,
                            }),
                        }],
                    )?;
                    ordinal += 1;
                }
            }
        }
        if let Some(condition) = &condition {
            for i in (1..n).rev() {
                self.emit(
                    f,
                    node,
                    iteration,
                    chain,
                    owner,
                    vec![Leaf {
                        ordinal,
                        calls: vec![],
                        origin: vec![],
                        op: Operation::Gate(Gate {
                            op: "ccx".into(),
                            wires: vec![
                                flags[i as usize - 1].clone(),
                                condition[i as usize].clone(),
                                flags[i as usize].clone(),
                            ],
                            controls: effective.clone(),
                            theta: None,
                        }),
                    }],
                )?;
                ordinal += 1;
            }
            if n > 0 {
                self.emit(
                    f,
                    node,
                    iteration,
                    chain,
                    owner,
                    vec![Leaf {
                        ordinal,
                        calls: vec![],
                        origin: vec![],
                        op: Operation::Gate(Gate {
                            op: "cx".into(),
                            wires: vec![condition[0].clone(), flags[0].clone()],
                            controls: effective.clone(),
                            theta: None,
                        }),
                    }],
                )?;
                ordinal += 1;
            }
            self.emit(
                f,
                node,
                iteration,
                chain,
                owner,
                vec![Leaf {
                    ordinal,
                    calls: vec![],
                    origin: vec![],
                    op: Operation::Release {
                        identity,
                        wires: flags,
                        class: "any".into(),
                        state: "clean".into(),
                    },
                }],
            )?;
        }
        Ok(())
    }
    fn expand_if(
        &mut self,
        context: InstructionContext<'_>,
        operands: &[String],
        env: &mut Env,
        effective: Vec<String>,
    ) -> Result<()> {
        let InstructionContext {
            f,
            node,
            chain,
            iteration,
            owner,
        } = context;
        if operands.len() != 1 || node.bodies.len() != 2 {
            return Err(Error::arg("qif requires predicate and two bodies"));
        }
        let p = wires(&operands[0], env)?;
        if p.len() != 1 {
            return Err(Error::arg("qif requires one wire"));
        }
        let mut pred = effective.clone();
        union(&mut pred, &p);
        self.nodes(f, &node.bodies[0], env, &pred, chain, iteration, owner)?;
        let g = Gate {
            op: "x".into(),
            wires: p.clone(),
            controls: effective.clone(),
            theta: None,
        };
        self.emit(
            f,
            node,
            iteration,
            chain,
            owner,
            vec![Leaf {
                ordinal: 0,
                calls: vec![],
                origin: vec![],
                op: Operation::Gate(g.clone()),
            }],
        )?;
        self.nodes(f, &node.bodies[1], env, &pred, chain, iteration, owner)?;
        self.emit(
            f,
            node,
            iteration,
            chain,
            owner,
            vec![Leaf {
                ordinal: 1,
                calls: vec![],
                origin: vec![],
                op: Operation::Gate(g),
            }],
        )?;
        Ok(())
    }
    fn expand_action_call(
        &mut self,
        context: InstructionContext<'_>,
        operands: &[String],
        env: &mut Env,
        effective: Vec<String>,
        readonly: &BTreeSet<String>,
        adjoint: bool,
    ) -> Result<()> {
        let InstructionContext {
            f,
            node,
            chain,
            iteration,
            owner,
        } = context;
        if operands.len() != 5 {
            return Err(Error::arg("action_call requires symbol and four groups"));
        }
        let callee = operands[0]
            .strip_prefix('@')
            .and_then(|n| self.object.functions.get(n))
            .ok_or_else(|| Error::arg("unresolved action callee"))?;
        if !["action", "perm_action"].contains(&callee.kind.as_str()) {
            return Err(Error::arg("action_call tier mismatch"));
        }
        let mut child = Env::new();
        let mut child_slots = Vec::new();
        for (group, role) in [(0, "mut"), (1, "in")] {
            let actuals = object::list(&operands[group + 1])?;
            if actuals.len() != callee.groups[group].len() {
                return Err(Error::arg("call slot count mismatch"));
            }
            for (i, (p, a)) in callee.groups[group].iter().zip(actuals).enumerate() {
                let w = wires(&a, env)?;
                if w.len() as u64 != object::wire_width(&p.ty)? {
                    return Err(Error::arg("call width mismatch"));
                }
                child.insert(p.name.clone(), Binding::Wires(w.clone()));
                child_slots.push(Slot {
                    id: format!("{role}:{i}"),
                    role: role.into(),
                    wires: w,
                });
            }
        }
        let mut used = BTreeSet::new();
        for slot in &child_slots {
            for w in &slot.wires {
                if !used.insert(w.clone())
                    || (slot.role == "mut" && (readonly.contains(w) || effective.contains(w)))
                {
                    return Err(Error::arg(
                        "§3.3/§5.1: call interfaces overlap or write a read-only/control wire",
                    ));
                }
            }
        }
        let regions = object::list(&operands[3])?;
        if regions.len() != callee.groups[2].len() {
            return Err(Error::arg("call region count mismatch"));
        }
        for (index, (p, actual)) in callee.groups[2].iter().zip(regions).enumerate() {
            let Some(Binding::Region {
                address,
                cell,
                role,
                origin,
                wires,
            }) = env.get(&actual)
            else {
                return Err(Error::arg("unbound region call operand"));
            };
            let (a, w) = object::region_shape(&p.ty)?;
            let requested = p
                .role
                .as_deref()
                .ok_or_else(|| Error::arg("callee region role missing"))?;
            if (a, w) != (*address, *cell)
                || (!origin.starts_with("local:") && role != requested)
                || wires.iter().any(|wire| !used.insert(wire.clone()))
                || (requested == "mut"
                    && wires
                        .iter()
                        .any(|wire| readonly.contains(wire) || effective.contains(wire)))
            {
                return Err(Error::arg(
                    "§3.3/K26: region call shape, role or disjointness mismatch",
                ));
            }
            child.insert(
                p.name.clone(),
                Binding::Region {
                    address: *address,
                    cell: *cell,
                    role: requested.into(),
                    origin: origin.clone(),
                    wires: wires.clone(),
                },
            );
            child_slots.push(Slot {
                id: format!("mem:{index}"),
                role: format!("mem_{requested}"),
                wires: wires.clone(),
            });
        }
        let args = object::list(&operands[4])?;
        if args.len() != callee.groups[3].len() {
            return Err(Error::arg("call classical argument count mismatch"));
        }
        let mut values = Vec::new();
        for (p, a) in callee.groups[3].iter().zip(args) {
            let Some(Binding::Value {
                isa_type,
                value,
                token_identity,
            }) = env.get(&a)
            else {
                return Err(Error::arg("unbound call classical argument"));
            };
            if isa_type != &json!(p.ty) {
                return Err(Error::arg("call argument type mismatch"));
            }
            values.push(value.clone());
            child.insert(
                p.name.clone(),
                Binding::Value {
                    isa_type: isa_type.clone(),
                    value: value.clone(),
                    token_identity: *token_identity,
                },
            );
        }
        let mut path = chain.to_vec();
        path.push(Occurrence {
            symbol: self.object.symbols[&f.name].interface_hash.clone(),
            point: node.point.clone(),
            iteration: iteration.to_vec(),
            instance: vec![],
            call_index: Some(0),
            entered_oracle: None,
        });
        self.invoke(
            callee,
            child,
            effective,
            child_slots,
            json!(values),
            self.trace.invocations[owner].budget,
            &path,
            adjoint,
            Some(owner),
        )?;
        Ok(())
    }
    fn expand_memory(
        &mut self,
        context: InstructionContext<'_>,
        env: &mut Env,
        effective: Vec<String>,
        readonly: &BTreeSet<String>,
    ) -> Result<(Vec<Leaf>, Option<Vec<Leaf>>)> {
        let InstructionContext {
            node,
            chain,
            iteration,
            ..
        } = context;
        let mut replacement = None;
        let identity = format!(
            "{}/{}/{:?}",
            canonical::hash(&canonical::to_vec(&json!(
                chain.iter().map(Occurrence::json).collect::<Vec<_>>()
            ))?),
            node.point,
            iteration
        );
        let plan = bind_memory(node, env, &readonly, &identity)?;
        let leaves = self.recipe(&plan.kind, plan.args.clone(), env, &effective)?;
        if self.adjoint_depth > 0 {
            let inverse_kind = if let Some(role) = plan.kind.strip_prefix("qload.") {
                format!("qunload.{role}")
            } else if let Some(role) = plan.kind.strip_prefix("qunload.") {
                format!("qload.{role}")
            } else {
                plan.kind.clone()
            };
            replacement = Some(self.recipe(&inverse_kind, plan.args, env, &effective)?);
        }
        if let Some(name) = plan.consumed {
            env.remove(&name);
        }
        Ok((leaves, replacement))
    }
    fn expand_gate(
        &mut self,
        gate: &str,
        operands: Vec<String>,
        env: &Env,
        effective: Vec<String>,
        readonly: &BTreeSet<String>,
        adjoint: bool,
    ) -> Result<Vec<Leaf>> {
        let mut leaves = Vec::new();
        let mut params = operands;
        let theta = if ["rx", "ry", "rz"].contains(&gate) {
            if params.is_empty() {
                return Err(Error::arg("missing angle"));
            }
            let s = params.remove(0);
            let f = s
                .parse::<f64>()
                .map_err(|_| Error::arg("invalid gate angle"))?;
            if !f.is_finite() {
                return Err(Error::arg("nonfinite gate angle"));
            }
            Some(format!("0x{:016x}", f.to_bits()))
        } else {
            None
        };
        if gate == "mcx" {
            let [controls, target] = params.as_slice() else {
                return Err(Error::arg(
                    "§4.4: mcx requires one control list and one target",
                ));
            };
            object::list(controls)?;
            if wires(target, env)?.len() != 1 {
                return Err(Error::arg("§4.4: mcx target must denote exactly one wire"));
            }
        }
        let mut w = Vec::new();
        for p in params {
            if p.starts_with('(') {
                for p in object::list(&p)? {
                    w.extend(wires(&p, env)?);
                }
            } else {
                w.extend(wires(&p, env)?);
            }
        }
        let mut g = Gate {
            op: gate.into(),
            wires: w,
            controls: effective,
            theta,
        };
        g.validate()?;
        if !["s", "sdg", "t", "tdg", "rz", "cz"].contains(&gate)
            && g.wires[g.own_controls()..]
                .iter()
                .any(|w| readonly.contains(w))
        {
            return Err(Error::arg("§5.1: non-diagonal gate writes an in wire"));
        }
        if adjoint {
            g = g.adjoint();
        }
        leaves.push(Leaf {
            ordinal: 0,
            calls: vec![],
            origin: vec![],
            op: Operation::Gate(g),
        });
        Ok(leaves)
    }
    fn expand_bundle(
        &mut self,
        op: &str,
        operands: &[String],
        env: &Env,
        effective: Vec<String>,
        readonly: &BTreeSet<String>,
    ) -> Result<(Vec<Leaf>, Option<Vec<Leaf>>)> {
        let mut replacement = None;
        if operands.len() != 2 {
            return Err(Error::arg("bundle operation arity"));
        }
        if wires(&operands[1], env)?
            .iter()
            .any(|w| readonly.contains(w))
            || (op == "qexch"
                && wires(&operands[0], env)?
                    .iter()
                    .any(|w| readonly.contains(w)))
        {
            return Err(Error::arg("§5.1: bundle operation writes an in wire"));
        }
        let n = wires(&operands[0], env)?.len();
        let leaves = self.recipe(
            if op == "qxor" {
                "bind.qxor"
            } else {
                "bind.qexch"
            },
            json!([n, operands[0], operands[1]]),
            env,
            &effective,
        )?;
        if self.adjoint_depth > 0 {
            replacement = Some(leaves.clone());
        }
        Ok((leaves, replacement))
    }
    fn nodes(
        &mut self,
        f: &Function,
        nodes: &[Node],
        env: &mut Env,
        controls: &[String],
        chain: &[Occurrence],
        iteration: &[u64],
        owner: usize,
    ) -> Result<()> {
        let mut predicates: Vec<Vec<String>> = vec![controls.to_vec()];
        let mut pushed = Vec::new();
        let readonly = self.trace.invocations[owner]
            .slots
            .iter()
            .filter(|s| s.role == "in" || s.role == "mem_in")
            .flat_map(|s| s.wires.iter().cloned())
            .collect::<BTreeSet<_>>();
        for node in nodes {
            let controls = predicates.last().unwrap().clone();
            let mut operation = node.op.clone();
            let mut operands = node.operands.clone();
            let mut adjoint = false;
            let mut effective = controls.clone();
            while operation == "adj" || operation == "ctl" {
                if operation == "adj" {
                    adjoint = !adjoint;
                    operation = operands
                        .first()
                        .ok_or_else(|| Error::arg("adj requires instruction"))?
                        .clone();
                    operands.remove(0);
                } else {
                    let c = wires(
                        operands
                            .first()
                            .ok_or_else(|| Error::arg("ctl requires a wire"))?,
                        env,
                    )?;
                    if c.len() != 1 {
                        return Err(Error::arg("ctl requires one wire"));
                    }
                    union(&mut effective, &c);
                    operands.remove(0);
                    operation = operands
                        .first()
                        .ok_or_else(|| Error::arg("ctl requires call"))?
                        .clone();
                    operands.remove(0);
                }
            }
            let op = operation.as_str();
            let context = InstructionContext {
                f,
                node,
                chain,
                iteration,
                owner,
            };
            let mut leaves = Vec::new();
            let mut replacement = None;
            match op {
                "qpred" => {
                    if operands.len() != 1 {
                        return Err(Error::arg("qpred arity"));
                    }
                    let p = wires(&operands[0], env)?;
                    if p.len() != 1 {
                        return Err(Error::arg("qpred requires one wire"));
                    }
                    pushed.push(p.clone());
                    let mut next = effective;
                    union(&mut next, &p);
                    predicates.push(next);
                    continue;
                }
                "qunpred" => {
                    if operands.len() != 1
                        || predicates.len() == 1
                        || pushed.pop().as_ref() != Some(&wires(&operands[0], env)?)
                    {
                        return Err(Error::arg("§3.5: qunpred must pop the same predicate wire"));
                    }
                    predicates.pop();
                    continue;
                }
                "qloop" => {
                    self.expand_loop(context, &operands, env, effective)?;
                    continue;
                }
                "qif" => {
                    self.expand_if(context, &operands, env, effective)?;
                    continue;
                }
                "qalloc" | "qborrow" => {
                    if operands.len() != 2 {
                        return Err(Error::arg("qalloc requires class and width"));
                    }
                    let n = operands[1]
                        .parse::<u64>()
                        .map_err(|_| Error::arg("qalloc width"))?;
                    if n == 0 {
                        return Err(Error::arg("source qalloc width must be positive"));
                    }
                    let name = node
                        .lhs
                        .as_ref()
                        .ok_or_else(|| Error::arg("allocation requires result"))?;
                    if env.contains_key(name) {
                        return Err(Error::arg("allocation shadows live binding"));
                    }
                    let identity = format!(
                        "source/{}/{}/{:?}",
                        canonical::hash(&canonical::to_vec(&json!(
                            chain.iter().map(Occurrence::json).collect::<Vec<_>>()
                        ))?),
                        node.point,
                        iteration
                    );
                    let w = (0..n)
                        .map(|i| format!("{identity}[{i}]"))
                        .collect::<Vec<_>>();
                    env.insert(
                        name.clone(),
                        Binding::Allocation {
                            identity: identity.clone(),
                            wires: w.clone(),
                            class: operands[0].clone(),
                            state: if op == "qborrow" { "borrowed" } else { "clean" }.into(),
                        },
                    );
                    leaves.push(Leaf {
                        ordinal: 0,
                        calls: vec![],
                        origin: vec![],
                        op: Operation::Alloc {
                            identity,
                            wires: w,
                            class: operands[0].clone(),
                            state: if op == "qborrow" { "borrowed" } else { "clean" }.into(),
                        },
                    });
                }
                "qfree" | "qreturn" => {
                    if operands.len() != 1 {
                        return Err(Error::arg("release requires one bundle"));
                    }
                    let Some(Binding::Allocation {
                        identity,
                        wires,
                        class,
                        state,
                    }) = env.remove(&operands[0])
                    else {
                        return Err(Error::arg("release requires live source allocation"));
                    };
                    if (op == "qreturn") != (state == "borrowed") {
                        return Err(Error::arg(
                            "§5.1: qfree matches qalloc and qreturn matches qborrow",
                        ));
                    }
                    leaves.push(Leaf {
                        ordinal: 0,
                        calls: vec![],
                        origin: vec![],
                        op: Operation::Release {
                            identity,
                            wires,
                            class,
                            state,
                        },
                    });
                }
                "qcopy" => {
                    if operands.len() != 1 {
                        return Err(Error::arg("qcopy arity"));
                    }
                    let s = wires(&operands[0], env)?;
                    let name = node
                        .lhs
                        .as_ref()
                        .ok_or_else(|| Error::arg("qcopy result required"))?;
                    if env.contains_key(name) {
                        return Err(Error::arg("qcopy shadows binding"));
                    }
                    let identity = format!(
                        "source-copy/{}/{}/{:?}",
                        canonical::hash(&canonical::to_vec(&json!(
                            chain.iter().map(Occurrence::json).collect::<Vec<_>>()
                        ))?),
                        node.point,
                        iteration
                    );
                    let dest = (0..s.len())
                        .map(|i| format!("{identity}[{i}]"))
                        .collect::<Vec<_>>();
                    env.insert(
                        name.clone(),
                        Binding::Allocation {
                            identity,
                            wires: dest,
                            class: "any".into(),
                            state: "clean".into(),
                        },
                    );
                    leaves = self.recipe(
                        "qcopy.bound",
                        json!([s.len(), operands[0], name]),
                        env,
                        &effective,
                    )?;
                    if self.adjoint_depth > 0 {
                        replacement = Some(self.recipe(
                            "quncopy.bound",
                            json!([s.len(), operands[0], name]),
                            env,
                            &effective,
                        )?);
                    }
                }
                "quncopy" => {
                    if operands.len() != 2 {
                        return Err(Error::arg("quncopy arity"));
                    }
                    let n = wires(&operands[0], env)?.len();
                    leaves = self.recipe(
                        "quncopy.bound",
                        json!([n, operands[0], operands[1]]),
                        env,
                        &effective,
                    )?;
                    if self.adjoint_depth > 0 {
                        replacement = Some(self.recipe(
                            "qcopy.bound",
                            json!([n, operands[0], operands[1]]),
                            env,
                            &effective,
                        )?);
                    }
                    env.remove(&operands[1]);
                }
                "qxor" | "qexch" => {
                    (leaves, replacement) =
                        self.expand_bundle(op, &operands, env, effective, &readonly)?;
                }
                "action_call" => {
                    self.expand_action_call(
                        context, &operands, env, effective, &readonly, adjoint,
                    )?;
                    continue;
                }
                "oracle_call" => {
                    self.oracle_call(
                        f, node, &operands, env, &effective, &readonly, chain, iteration, owner, 0,
                    )?;
                    continue;
                }
                "qinplace" => {
                    self.qinplace(
                        f, node, &operands, env, &effective, &readonly, chain, iteration, owner,
                        adjoint,
                    )?;
                    continue;
                }
                "qregion" => continue,
                "qload" | "qunload" | "qstore" | "qexchm" => {
                    (leaves, replacement) =
                        self.expand_memory(context, env, effective, &readonly)?;
                }
                gate if GATES.contains(&gate) => {
                    leaves =
                        self.expand_gate(gate, operands, env, effective, &readonly, adjoint)?;
                }
                _ => {
                    return Err(Error::arg(format!(
                        "§4: unknown or misplaced source instruction {op}"
                    )));
                }
            }
            if let Some(mut replacement) = replacement {
                let prefix = canonical::hash(&canonical::to_vec(&json!([
                    chain.iter().map(Occurrence::json).collect::<Vec<_>>(),
                    node.point,
                    iteration
                ]))?);
                for leaf in &mut replacement {
                    rebase_leaf(leaf, &prefix);
                }
                self.replacements
                    .insert((owner, node.point.clone(), iteration.to_vec()), replacement);
            }
            self.emit(f, node, iteration, chain, owner, leaves)?;
        }
        if predicates.len() != 1 {
            return Err(Error::arg("unclosed source predicate stack"));
        }
        Ok(())
    }
}
pub(crate) fn is_descendant(mut owner: usize, ancestor: usize, invocations: &[Invocation]) -> bool {
    loop {
        if owner == ancestor {
            return true;
        }
        match invocations[owner].parent {
            Some(parent) => owner = parent,
            None => return false,
        }
    }
}

fn remap_adjoint_iterations(
    events: &mut [Event],
    descendants: &mut [Invocation],
    object: &Object,
    caller_depth: usize,
) -> Result<()> {
    fn collect(nodes: &[Node], stack: &[u64], out: &mut BTreeMap<String, Vec<u64>>) -> Result<()> {
        for node in nodes {
            out.insert(node.point.clone(), stack.to_vec());
            let mut nested = stack.to_vec();
            if node.op == "qloop" {
                nested.push(crate::schedule::uint(&json!(node.operands[0]))?);
            }
            for body in &node.bodies {
                collect(body, &nested, out)?;
            }
        }
        Ok(())
    }
    let mut maps = BTreeMap::new();
    for f in object.functions.values() {
        let mut points = BTreeMap::new();
        collect(&f.body, &[], &mut points)?;
        if ["action", "perm_action"].contains(&f.kind.as_str()) {
            points.insert("$construct".into(), vec![]);
            points.insert("$release".into(), vec![]);
        }
        maps.insert(object.symbols[&f.name].interface_hash.clone(), points);
    }
    let remap = |path: &mut [Occurrence]| -> Result<()> {
        // The call's caller prefix is outside this adjoint. Only this
        // invocation and its descendants reverse their local loop traversal.
        let path = path
            .get_mut(caller_depth..)
            .ok_or_else(|| Error::arg("adjoint event is missing its caller path"))?;
        for occurrence in path {
            let bounds = maps
                .get(&occurrence.symbol)
                .and_then(|m| m.get(&occurrence.point))
                .ok_or_else(|| Error::arg("adjoint source point is not in linked object"))?;
            if bounds.len() != occurrence.iteration.len() {
                return Err(Error::arg("adjoint source loop coordinate depth mismatch"));
            }
            for (i, bound) in occurrence.iteration.iter_mut().zip(bounds) {
                *i = bound
                    .checked_sub(1)
                    .and_then(|last| last.checked_sub(*i))
                    .ok_or_else(|| Error::arg("adjoint source iteration outside loop"))?;
            }
            if occurrence.entered_oracle.is_some() {
                // The call site is adjointed in this action; the entered
                // oracle and all its internal source coordinates stay forward.
                break;
            }
        }
        Ok(())
    };
    for event in events {
        remap(&mut event.location.path)?;
    }
    for invocation in descendants {
        remap(&mut invocation.path)?;
    }
    Ok(())
}

/// K62 forms the while action adjoint before recipe instantiation: all flags
/// are computed before the descending controlled bodies. Reversing an expanded
/// stream alone would interleave flag destruction with those bodies.
fn form_while_adjoint(
    events: &mut Vec<Event>,
    invocations: &[Invocation],
    frozen: &BTreeMap<usize, usize>,
) -> Result<()> {
    let ids = events
        .iter()
        .filter_map(|e| match &e.leaf.op {
            Operation::Alloc { identity, .. }
                if identity.starts_with("source-flags/") && !frozen.contains_key(&e.owner) =>
            {
                Some(identity.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for identity in ids {
        let start = events
            .iter()
            .position(|e| matches!(&e.leaf.op,Operation::Alloc{identity:id,..}if id==&identity))
            .ok_or_else(|| Error::arg("missing while allocation"))?;
        let end = events
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(i, e)| {
                matches!(&e.leaf.op,Operation::Release{identity:id,..}if id==&identity).then_some(i)
            })
            .ok_or_else(|| Error::arg("missing while release"))?;
        let allocation = events[start].clone();
        let release = events[end].clone();
        let Operation::Alloc { wires, .. } = &allocation.leaf.op else {
            unreachable!()
        };
        let n = wires.len();
        let mut support = Vec::new();
        let mut body = Vec::new();
        for e in events[start + 1..end].iter().cloned() {
            if e.owner == allocation.owner && e.location.path == allocation.location.path {
                support.push(e);
            } else {
                body.push(e);
            }
        }
        if support.len() != 2 * n
            || support
                .iter()
                .any(|e| !matches!(e.leaf.op, Operation::Gate(_)))
        {
            return Err(Error::arg(
                "while adjoint support does not match flag recipe",
            ));
        }
        let mut replacement = vec![allocation.clone()];
        if invocations[allocation.owner].adjoint {
            replacement.extend(support[..n].iter().cloned());
            replacement.extend(body);
        } else {
            if n > 0 {
                replacement.push(support[0].clone());
            }
            let nesting = allocation.location.path.last().unwrap().iteration.len();
            let symbol = &allocation.location.path.last().unwrap().symbol;
            for i in 0..n {
                for e in &body {
                    let occurrence = e
                        .location
                        .path
                        .iter()
                        .find(|p| &p.symbol == symbol && p.iteration.len() > nesting)
                        .ok_or_else(|| Error::arg("while body lacks its source iteration"))?;
                    if occurrence.iteration[nesting] == i as u64 {
                        replacement.push(e.clone());
                    }
                }
                if i + 1 < n {
                    replacement.push(support[i + 1].clone());
                }
            }
        }
        replacement.extend(support[n..].iter().cloned());
        replacement.push(release);
        events.splice(start..=end, replacement);
    }
    Ok(())
}

fn diagonal(g: &Gate) -> bool {
    ["s", "sdg", "t", "tdg", "rz", "cz"].contains(&g.op.as_str())
}
fn permutation(g: &Gate) -> bool {
    g.op.starts_with("$perm/") || ["x", "cx", "ccx", "mcx", "swap"].contains(&g.op.as_str())
}
fn gate_adjoint(g: &Gate) -> Gate {
    let mut n = g.adjoint();
    if let Some(s) = g.op.strip_prefix("$perm/call+/") {
        n.op = format!("$perm/call-/{s}");
    } else if let Some(s) = g.op.strip_prefix("$perm/call-/") {
        n.op = format!("$perm/call+/{s}");
    } else if let Some(s) = g.op.strip_prefix("$other/call+/") {
        n.op = format!("$other/call-/{s}");
    } else if let Some(s) = g.op.strip_prefix("$other/call-/") {
        n.op = format!("$other/call+/{s}");
    } else if let Some(s) = g.op.strip_prefix("$perm/qinplace+/") {
        n.op = format!("$perm/qinplace-/{s}");
    } else if let Some(s) = g.op.strip_prefix("$perm/qinplace-/") {
        n.op = format!("$perm/qinplace+/{s}");
    }
    n
}
fn normalized(g: &Gate) -> Gate {
    let mut n = g.clone();
    if ["x", "cx", "ccx", "mcx"].contains(&g.op.as_str()) {
        n.op = "mcx".into();
        n.wires = vec![g.wires.last().unwrap().clone()];
        n.controls = g.control_wires();
    }
    n.controls.sort();
    n.controls.dedup();
    n
}
/// Sufficient exact factorisation test. Reduce only literal adjoints whose event
/// is on top of every operand's causal stack, including readers. A failed test
/// is not a semantic counterexample and keeps the general proof explicitly open.
fn independent(events: &[Event], protected: &[String]) -> bool {
    let mut active: Vec<(Gate, bool)> = Vec::new();
    let mut stacks: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for e in events {
        match &e.leaf.op {
            Operation::Gate(g) => {
                let n = normalized(g);
                let wires = n.all_wires();
                let top = wires.first().and_then(|w| stacks.get(w)?.last()).copied();
                let matched = top.filter(|i| {
                    wires
                        .iter()
                        .all(|w| stacks.get(w).and_then(|s| s.last()) == Some(i))
                        && gate_adjoint(&active[*i].0) == n
                });
                if let Some(i) = matched {
                    for w in wires {
                        stacks.get_mut(&w).unwrap().pop();
                    }
                    active[i].1 = false;
                } else {
                    let i = active.len();
                    for w in wires {
                        stacks.entry(w).or_default().push(i);
                    }
                    active.push((n, true));
                }
            }
            Operation::Alloc { wires, .. } | Operation::Release { wires, .. } => {
                let mut connected = wires.iter().cloned().collect::<BTreeSet<_>>();
                loop {
                    let old = connected.len();
                    for (g, live) in &active {
                        if *live && g.all_wires().iter().any(|w| connected.contains(w)) {
                            connected.extend(g.all_wires());
                        }
                    }
                    if old == connected.len() {
                        break;
                    }
                }
                for w in connected {
                    stacks.remove(&w);
                }
            }
            Operation::Classical { .. } => {}
        }
    }
    !active
        .iter()
        .any(|(g, live)| *live && g.all_wires().iter().any(|w| protected.contains(w)))
}
/// Source occurrence/version restoration, independent of basis values.
fn verify_clean_lifetimes(events: &[Event]) -> Result<(BTreeMap<usize, usize>, BTreeSet<usize>)> {
    #[derive(Clone)]
    struct Change {
        gate: Gate,
        inputs: BTreeMap<String, u64>,
        targets: Vec<String>,
        prior: Vec<Option<usize>>,
        prior_perm: Vec<bool>,
        post_perm: bool,
        workspace: bool,
        at: usize,
    }
    let mut provenance: BTreeMap<String, bool> = BTreeMap::new();
    let mut matches: BTreeMap<usize, usize> = BTreeMap::new();
    let mut perm_pairs = BTreeSet::new();
    let mut windows = Vec::new();
    let mut versions: BTreeMap<String, u64> = BTreeMap::new();
    let mut latest: BTreeMap<String, usize> = BTreeMap::new();
    let mut changes: Vec<Change> = Vec::new();
    let mut next = 1u64;
    let mut live = BTreeMap::new();
    for (at, event) in events.iter().enumerate() {
        match &event.leaf.op {
            Operation::Alloc {
                identity, wires, ..
            } => {
                if live.insert(identity.clone(), wires.clone()).is_some() {
                    return Err(Error::arg("duplicate source allocation"));
                }
                for w in wires {
                    provenance.insert(w.clone(), true);
                    versions.insert(w.clone(), 0);
                    latest.remove(w);
                }
            }
            Operation::Release {
                identity,
                wires,
                state,
                ..
            } => {
                if live.remove(identity).is_none() {
                    return Err(Error::arg("unmatched source release"));
                }
                if state == "clean"
                    && wires
                        .iter()
                        .any(|w| versions.get(w).copied().unwrap_or(0) != 0)
                {
                    return Err(Error::arg(
                        "§6.2: release is not restored by matched source inverses",
                    ));
                }
                for w in wires {
                    versions.remove(w);
                    latest.remove(w);
                }
            }
            Operation::Gate(g) => {
                if diagonal(g) {
                    continue;
                }
                let normalized = normalized(g);
                let targets = g.wires[g.own_controls()..].to_vec();
                let prev = targets.first().and_then(|w| latest.get(w)).copied();
                let matched = prev
                    .filter(|i| targets.iter().all(|w| latest.get(w) == Some(i)))
                    .filter(|i| {
                        let c = &changes[*i];
                        c.targets == targets
                            && gate_adjoint(&c.gate) == normalized
                            && c.inputs.iter().all(|(w, v)| {
                                targets.contains(w) || versions.get(w).copied().unwrap_or(0) == *v
                            })
                    });
                if let Some(i) = matched {
                    let c = &changes[i];
                    if c.post_perm {
                        perm_pairs.insert(c.at);
                    }
                    matches.insert(c.at, at);
                    matches.insert(at, c.at);
                    if c.workspace && c.post_perm {
                        windows.push((
                            c.at,
                            at,
                            c.inputs
                                .keys()
                                .filter(|w| !targets.contains(w))
                                .cloned()
                                .collect::<Vec<_>>(),
                            targets.clone(),
                        ));
                    } else if c.workspace && !independent(&events[c.at + 1..at], &targets) {
                        return Err(deferred(
                            "joint other-workspace factorisation after composed intervening uses",
                        ));
                    }
                    for (w, prior) in targets.iter().zip(&c.prior) {
                        provenance.insert(
                            w.clone(),
                            c.prior_perm[c.targets.iter().position(|t| t == w).unwrap()],
                        );
                        versions.insert(w.clone(), c.inputs[w]);
                        if let Some(p) = prior {
                            latest.insert(w.clone(), *p);
                        } else {
                            latest.remove(w);
                        }
                    }
                } else {
                    let inputs = g
                        .all_wires()
                        .into_iter()
                        .map(|w| {
                            let v = versions.get(&w).copied().unwrap_or(0);
                            (w, v)
                        })
                        .collect();
                    let prior = targets.iter().map(|w| latest.get(w).copied()).collect();
                    let prior_perm = targets
                        .iter()
                        .map(|w| provenance.get(w).copied().unwrap_or(true))
                        .collect::<Vec<_>>();
                    let post_perm = permutation(g)
                        && g.all_wires()
                            .iter()
                            .all(|w| provenance.get(w).copied().unwrap_or(true));
                    let i = changes.len();
                    changes.push(Change {
                        gate: normalized,
                        inputs,
                        targets: targets.clone(),
                        prior,
                        prior_perm,
                        post_perm,
                        at,
                        workspace: targets
                            .iter()
                            .any(|w| live.values().any(|wires| wires.contains(w))),
                    });
                    for w in targets {
                        provenance.insert(w.clone(), post_perm);
                        versions.insert(w.clone(), next);
                        latest.insert(w, i);
                    }
                    next += 1;
                }
            }
            Operation::Classical { .. } => {}
        }
    }
    for (compute, cleanup, dependencies, value) in windows {
        for i in compute + 1..cleanup {
            let Operation::Gate(g) = &events[i].leaf.op else {
                continue;
            };
            if diagonal(g) {
                continue;
            }
            let writes = &g.wires[g.own_controls()..];
            if writes.iter().any(|w| {
                dependencies.contains(w)
                    && (w.starts_with("mem:") || w.starts_with("source-region/"))
            }) {
                return Err(Error::arg(
                    "§6.2: region cell version advance closes cleanup window even if restored",
                ));
            }
            if !permutation(g)
                && writes
                    .iter()
                    .any(|w| dependencies.contains(w) || value.contains(w))
            {
                return Err(Error::arg(
                    "§6.2: non-diagonal operation closes cleanup window",
                ));
            }
            if writes.iter().any(|w| dependencies.contains(w)) {
                let Some(partner) = matches.get(&i).copied() else {
                    return Err(Error::arg(
                        "§6.2: dependency changed without a matched undo in cleanup window",
                    ));
                };
                if partner <= compute || partner >= cleanup {
                    return Err(Error::arg(
                        "§6.2: dependency undo is outside cleanup window",
                    ));
                }
                if i<partner&&events[i+1..partner].iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if !diagonal(g)&&g.wires[g.own_controls()..].iter().any(|w|dependencies.contains(w)||value.contains(w)))){return Err(Error::arg("§6.2: dependency undo crossed a value change and closed cleanup window"));}
            }
        }
    }
    if !live.is_empty() {
        return Err(Error::arg("§6: source exits with live workspace"));
    }
    Ok((matches, perm_pairs))
}
