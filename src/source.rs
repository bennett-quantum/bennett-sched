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
    pub parent:Option<usize>,
    pub adjoint:bool,
}
#[derive(Clone, Debug)]
pub struct Trace {
    pub events: Vec<Event>,
    pub invocations: Vec<Invocation>,
    pub slots: Vec<Slot>,
    pub inherited: Vec<String>,
}
fn deferred(feature: &str) -> Error {
    Error::new(
        Status::GateUnsupported,
        format!(
            "§11: source binding for {feature} is postponed (bennett-sched-ark); oracle helper binding also requires contract decision bennett-sched-enz"
        ),
    )
}
fn union(a: &mut Vec<String>, b: &[String]) {
    for w in b {
        if !a.contains(w) {
            a.push(w.clone());
        }
    }
}
fn live_types(env:&Env)->BTreeMap<String,Value>{env.iter().map(|(name,b)|{let ty=match b{Binding::Wires(w)=>json!(["wire",w.len()]),Binding::Allocation{wires,class,state,..}=>json!(["allocation",wires.len(),class,state]),Binding::Region{address,cell,role,origin,..}=>json!(["region",address,cell,role,origin]),Binding::Value{isa_type,..}=>json!(["value",isa_type]),_=>json!("internal")};(name.clone(),ty)}).collect()}
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
        Some(
            Binding::Wires(w)
            | Binding::Allocation { wires: w, .. }
            | Binding::Region { wires: w, .. },
        ) => Ok(w.clone()),
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
        object.validate_params(name, params)?;
        let f = object
            .functions
            .get(name)
            .ok_or_else(|| Error::arg("unknown source region"))?;
        if f.kind == "oracle" {
            return Err(deferred(
                "oracle construction, padded dispatch, fixed-binding lproc instances and support points",
            ));
        }
        if !["action", "perm_action"].contains(&f.kind.as_str()) {
            return Err(Error::arg("§11: root must be an oracle or action"));
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
        if !f.groups[2].is_empty() {
            return Err(deferred(
                "source interface regions and coherent-memory instructions",
            ));
        }
        for (p, v) in f.groups[3]
            .iter()
            .zip(params.as_array().expect("validated params"))
        {
            env.insert(
                p.name.clone(),
                Binding::Value {
                    isa_type: json!(p.ty),
                    value: v.clone(),
                },
            );
        }
        let inherited = (0..inherited)
            .map(|i| format!("control:{i}"))
            .collect::<Vec<_>>();
        let mut trace = Self {
            events: vec![],
            invocations: vec![],
            slots: slots.clone(),
            inherited: inherited.clone(),
        };
        let lib = Library::published()?;
        let mut builder = Builder {
            object,
            library: &lib,
            trace: &mut trace,
            active: BTreeSet::new(),
        };
        builder.invoke(f, env, inherited, slots, params.clone(), budget, &[], false,None)?;
        if f.kind=="perm_action"&&trace.events.iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if !["x","cx","ccx","mcx","swap"].contains(&g.op.as_str()))){return Err(Error::arg("§5.3: permutation actions require permutation gates transitively"));}
        let workspace = trace
            .events
            .iter()
            .any(|e| matches!(&e.leaf.op, Operation::Alloc {identity,..}if !identity.starts_with("source-flags/")));
        if workspace&&trace.events.iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if !["x","cx","ccx","mcx","swap"].contains(&g.op.as_str()))){return Err(deferred("general non-permutation workspace factorisation and cleanup-window proof"));}
        if workspace && trace.invocations.len() > 1 {
            return Err(deferred(
                "source semantic-call cleanup-pair partition and version proof",
            ));
        }
        require_stable_workspace_inputs(&trace.events)?;
        verify_clean_lifetimes(&trace.events)?;
        Ok(trace)
    }
}
struct Builder<'a> {
    object: &'a Object,
    library: &'a Library,
    trace: &'a mut Trace,
    active: BTreeSet<String>,
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
        parent:Option<usize>,
    ) -> Result<()> {
        let bounds=object::flattened(&f.body).into_iter().filter(|n|n.op=="qloop").map(|n|crate::schedule::uint(&json!(n.operands.first().ok_or_else(||Error::arg("missing qloop bound"))?))).collect::<Result<Vec<_>>>()?;
        if f.signature["bounds"]!=json!(bounds){return Err(Error::arg("§5.3: action bounds must equal source loop trip counts"));}
        if !self.active.insert(f.name.clone()) {
            return Err(Error::arg("§3.1: cyclic action call graph"));
        }
        let owner = self.trace.invocations.len();
        let start = self.trace.events.len();
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
            adjoint:false,
        });
        self.nodes(f, &f.body, &mut env, &controls, chain, &[], owner)?;
        if adjoint {
            for inv in self.trace.invocations.iter_mut().skip(owner){inv.adjoint=!inv.adjoint;}
            let end = self.trace.events.len();
            for inv in self.trace.invocations.iter_mut().skip(owner + 1) {
                let (old_start, old_end) = (inv.start, inv.end);
                inv.start = start + end - old_end;
                inv.end = start + end - old_start;
            }
            let mut events = self.trace.events.split_off(start);
            events.reverse();
            for e in &mut events{e.leaf=expansion::inverse(std::slice::from_ref(&e.leaf)).pop().unwrap();}
            remap_adjoint_iterations(&mut events,self.object)?;
            form_while_adjoint(&mut events,&self.trace.invocations)?;
            let mut ordinals = BTreeMap::new();
            for e in &mut events {
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
            for index in owner+1..self.trace.invocations.len(){let indices=self.trace.events[start..].iter().enumerate().filter_map(|(i,e)|is_descendant(e.owner,index,&self.trace.invocations).then_some(start+i)).collect::<Vec<_>>();if let (Some(first),Some(last))=(indices.first(),indices.last()){self.trace.invocations[index].start=*first;self.trace.invocations[index].end=last+1;}}
        }
        self.trace.invocations[owner].end = self.trace.events.len();
        self.active.remove(&f.name);
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
            fn rebase(w: &mut String, prefix: &str) {
                if w.starts_with("scratch/") || w.starts_with("declaration/") {
                    *w = format!("{prefix}/{w}");
                }
            }
            let kind = match &mut leaf.op {
                Operation::Gate(g) => {
                    for w in g.wires.iter_mut().chain(&mut g.controls) {
                        rebase(w, &prefix);
                    }
                    Kind::Gate
                }
                Operation::Alloc {
                    identity, wires, ..
                } => {
                    rebase(identity, &prefix);
                    for w in wires {
                        rebase(w, &prefix);
                    }
                    Kind::Alloc
                }
                Operation::Release {
                    identity, wires, ..
                } => {
                    rebase(identity, &prefix);
                    for w in wires {
                        rebase(w, &prefix);
                    }
                    Kind::Release
                }
                Operation::Classical { .. } => Kind::Classical,
            };
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
            .filter(|s| s.role == "in")
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
            let mut leaves = Vec::new();
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
                    if operands.len()!=1 && !(operands.len()==3 && operands[1]=="while"){return Err(Error::arg("qloop requires a bound and optional while condition"));}
                    let n=crate::schedule::uint(&json!(operands[0]))?;
                    if node.bodies.len() != 1 {
                        return Err(Error::arg("qloop requires one body"));
                    }
                    let condition=if operands.len()==3{let w=wires(&operands[2],env)?;if w.len() as u64!=n{return Err(Error::arg("§4.6: while condition width must equal trip count"));}Some(w)}else{None};
                    let identity=format!("source-flags/{}/{}/{:?}",canonical::hash(&canonical::to_vec(&json!(chain.iter().map(Occurrence::json).collect::<Vec<_>>()))?),node.point,iteration);
                    let flags=(0..n).map(|i|format!("{identity}[{i}]")).collect::<Vec<_>>();let mut ordinal=0;
                    if let Some(condition)=&condition {
                        self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Alloc{identity:identity.clone(),wires:flags.clone(),class:"any".into(),state:"clean".into()}}])?;ordinal+=1;
                        if n>0{self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Gate(Gate{op:"cx".into(),wires:vec![condition[0].clone(),flags[0].clone()],controls:effective.clone(),theta:None})}])?;ordinal+=1;}
                    }
                    for i in 0..n {
                        let mut counts = iteration.to_vec();
                        counts.push(i);
                        let before=live_types(env);let mut controls=effective.clone();if condition.is_some(){union(&mut controls,&[flags[i as usize].clone()]);}
                        let start=self.trace.events.len();self.nodes(f, &node.bodies[0], env, &controls, chain, &counts, owner)?;
                        if before!=live_types(env){return Err(Error::arg("§4.6: loop body entry and exit live names/types must agree"));}
                        if let Some(condition)=&condition {
                            if self.trace.events[start..].iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if g.wires[g.own_controls()..].iter().any(|w|condition.contains(w)))){return Err(Error::arg("§3.3: while condition modified inside loop body"));}
                            if i+1<n{self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Gate(Gate{op:"ccx".into(),wires:vec![flags[i as usize].clone(),condition[i as usize+1].clone(),flags[i as usize+1].clone()],controls:effective.clone(),theta:None})}])?;ordinal+=1;}
                        }
                    }
                    if let Some(condition)=&condition{
                        for i in (1..n).rev(){self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Gate(Gate{op:"ccx".into(),wires:vec![flags[i as usize-1].clone(),condition[i as usize].clone(),flags[i as usize].clone()],controls:effective.clone(),theta:None})}])?;ordinal+=1;}
                        if n>0{self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Gate(Gate{op:"cx".into(),wires:vec![condition[0].clone(),flags[0].clone()],controls:effective.clone(),theta:None})}])?;ordinal+=1;}
                        self.emit(f,node,iteration,chain,owner,vec![Leaf{ordinal,calls:vec![],op:Operation::Release{identity,wires:flags,class:"any".into(),state:"clean".into()}}])?;
                    }
                    continue;
                }
                "qif" => {
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
                            op: Operation::Gate(g),
                        }],
                    )?;
                    continue;
                }
                "qalloc" | "qborrow" => {
                    if op == "qborrow" {
                        return Err(deferred(
                            "borrow-frame source proof and caller idle-set binding",
                        ));
                    }
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
                            state: "clean".into(),
                        },
                    );
                    leaves.push(Leaf {
                        ordinal: 0,
                        calls: vec![],
                        op: Operation::Alloc {
                            identity,
                            wires: w,
                            class: operands[0].clone(),
                            state: "clean".into(),
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
                    leaves.push(Leaf {
                        ordinal: 0,
                        calls: vec![],
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
                    env.remove(&operands[1]);
                }
                "qxor" | "qexch" => {
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
                    leaves = self.recipe(
                        if op == "qxor" {
                            "bind.qxor"
                        } else {
                            "bind.qexch"
                        },
                        json!([n, operands[0], operands[1]]),
                        env,
                        &effective,
                    )?;
                }
                "action_call" => {
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
                                || (slot.role == "mut"
                                    && (readonly.contains(w) || effective.contains(w)))
                            {
                                return Err(Error::arg(
                                    "§3.3/§5.1: call interfaces overlap or write a read-only/control wire",
                                ));
                            }
                        }
                    }
                    if !object::list(&operands[3])?.is_empty() || !callee.groups[2].is_empty() {
                        return Err(deferred("source action region forwarding"));
                    }
                    let args = object::list(&operands[4])?;
                    if args.len() != callee.groups[3].len() {
                        return Err(Error::arg("call classical argument count mismatch"));
                    }
                    let mut values = Vec::new();
                    for (p, a) in callee.groups[3].iter().zip(args) {
                        let Some(Binding::Value { isa_type, value }) = env.get(&a) else {
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
                    continue;
                }
                "oracle_call" | "qinplace" => {
                    return Err(deferred(
                        "oracle token context, construction and inverse-pair coverage",
                    ));
                }
                "qregion" | "qload" | "qunload" | "qstore" | "qexchm" => {
                    return Err(deferred(
                        "local region image lifetimes and coherent-memory source adapters",
                    ));
                }
                gate if GATES.contains(&gate) => {
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
                        op: Operation::Gate(g),
                    });
                }
                lifted
                    if lifted.starts_with("lift.")
                        || ["qbranch", "qjump", "fault"].contains(&lifted) =>
                {
                    return Err(deferred("lifted source row and successor templates"));
                }
                _ => {
                    return Err(Error::arg(format!(
                        "§4: unknown or misplaced source instruction {op}"
                    )));
                }
            }
            self.emit(f, node, iteration, chain, owner, leaves)?;
        }
        if predicates.len() != 1 {
            return Err(Error::arg("unclosed source predicate stack"));
        }
        Ok(())
    }
}
fn is_descendant(mut owner:usize,ancestor:usize,invocations:&[Invocation])->bool{loop{if owner==ancestor{return true;}match invocations[owner].parent{Some(parent)=>owner=parent,None=>return false}}}

fn remap_adjoint_iterations(events:&mut [Event],object:&Object)->Result<()>{
    fn collect(nodes:&[Node],stack:&[u64],out:&mut BTreeMap<String,Vec<u64>>)->Result<()>{for node in nodes{out.insert(node.point.clone(),stack.to_vec());let mut nested=stack.to_vec();if node.op=="qloop"{nested.push(crate::schedule::uint(&json!(node.operands[0]))?);}for body in &node.bodies{collect(body,&nested,out)?;}}Ok(())}
    let mut maps=BTreeMap::new();for f in object.functions.values(){let mut points=BTreeMap::new();collect(&f.body,&[],&mut points)?;maps.insert(object.symbols[&f.name].interface_hash.clone(),points);}
    for event in events{for occurrence in &mut event.location.path{let bounds=maps.get(&occurrence.symbol).and_then(|m|m.get(&occurrence.point)).ok_or_else(||Error::arg("adjoint source point is not in linked object"))?;if bounds.len()!=occurrence.iteration.len(){return Err(Error::arg("adjoint source loop coordinate depth mismatch"));}for (i,bound) in occurrence.iteration.iter_mut().zip(bounds){*i=bound.checked_sub(1).and_then(|last|last.checked_sub(*i)).ok_or_else(||Error::arg("adjoint source iteration outside loop"))?;}}}Ok(())
}

/// K62 forms the while action adjoint before recipe instantiation: all flags
/// are computed before the descending controlled bodies. Reversing an expanded
/// stream alone would interleave flag destruction with those bodies.
fn form_while_adjoint(events:&mut Vec<Event>,invocations:&[Invocation])->Result<()>{
    let ids=events.iter().filter_map(|e|match &e.leaf.op{Operation::Alloc{identity,..}if identity.starts_with("source-flags/")=>Some(identity.clone()),_=>None}).collect::<Vec<_>>();
    for identity in ids{
        let start=events.iter().position(|e|matches!(&e.leaf.op,Operation::Alloc{identity:id,..}if id==&identity)).ok_or_else(||Error::arg("missing while allocation"))?;
        let end=events.iter().enumerate().skip(start+1).find_map(|(i,e)|matches!(&e.leaf.op,Operation::Release{identity:id,..}if id==&identity).then_some(i)).ok_or_else(||Error::arg("missing while release"))?;
        let allocation=events[start].clone();let release=events[end].clone();let Operation::Alloc{wires,..}=&allocation.leaf.op else{unreachable!()};let n=wires.len();
        let mut support=Vec::new();let mut body=Vec::new();for e in events[start+1..end].iter().cloned(){if e.owner==allocation.owner&&e.location.path==allocation.location.path{support.push(e);}else{body.push(e);}}
        if support.len()!=2*n||support.iter().any(|e|!matches!(e.leaf.op,Operation::Gate(_))){return Err(Error::arg("while adjoint support does not match flag recipe"));}
        let mut replacement=vec![allocation.clone()];
        if invocations[allocation.owner].adjoint{replacement.extend(support[..n].iter().cloned());replacement.extend(body);}
        else{
            if n>0{replacement.push(support[0].clone());}
            let nesting=allocation.location.path.last().unwrap().iteration.len();let symbol=&allocation.location.path.last().unwrap().symbol;
            for i in 0..n{for e in &body{let occurrence=e.location.path.iter().find(|p|&p.symbol==symbol&&p.iteration.len()>nesting).ok_or_else(||Error::arg("while body lacks its source iteration"))?;if occurrence.iteration[nesting]==i as u64{replacement.push(e.clone());}}if i+1<n{replacement.push(support[i+1].clone());}}
        }
        replacement.extend(support[n..].iter().cloned());replacement.push(release);events.splice(start..=end,replacement);
    }Ok(())
}

/// A version-restored input is insufficient to prove every §6.2 window rule.
/// Keep the remaining mutable-input proof explicit instead of accepting it by
/// primitive cancellation. The unrestricted algorithm is tracked by ark.
fn require_stable_workspace_inputs(events: &[Event]) -> Result<()> {
    for (start, event) in events.iter().enumerate() {
        let Operation::Alloc {
            identity, wires, ..
        } = &event.leaf.op
        else {
            continue;
        };
        let Some(end) = events
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(i, e)| {
                matches!(&e.leaf.op,Operation::Release{identity:id,..}if id==identity).then_some(i)
            })
        else {
            continue;
        };
        let owned = wires.iter().cloned().collect::<BTreeSet<_>>();
        let mut dependencies = BTreeSet::new();
        for e in &events[start + 1..end] {
            if let Operation::Gate(g) = &e.leaf.op {
                if g.wires[g.own_controls()..]
                    .iter()
                    .any(|w| owned.contains(w))
                {
                    dependencies.extend(g.all_wires().into_iter().filter(|w| !owned.contains(w)));
                }
            }
        }
        for e in &events[start + 1..end] {
            if let Operation::Gate(g) = &e.leaf.op {
                if g.wires[g.own_controls()..]
                    .iter()
                    .any(|w| dependencies.contains(w))
                {
                    return Err(deferred(
                        "cleanup-window proof with changing workspace input versions",
                    ));
                }
            }
        }
    }
    Ok(())
}
/// Source occurrence/version restoration, independent of basis values.
fn verify_clean_lifetimes(events: &[Event]) -> Result<()> {
    #[derive(Clone)]
    struct Change {
        gate: Gate,
        inputs: BTreeMap<String, u64>,
        targets: Vec<String>,
        prior: Vec<Option<usize>>,
    }
    let mut versions: BTreeMap<String, u64> = BTreeMap::new();
    let mut latest: BTreeMap<String, usize> = BTreeMap::new();
    let mut changes: Vec<Change> = Vec::new();
    let mut next = 1u64;
    let mut live = BTreeMap::new();
    for event in events {
        match &event.leaf.op {
            Operation::Alloc {
                identity, wires, ..
            } => {
                if live.insert(identity.clone(), wires.clone()).is_some() {
                    return Err(Error::arg("duplicate source allocation"));
                }
                for w in wires {
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
                let mut normalized = g.clone();
                if ["x", "cx", "ccx", "mcx"].contains(&g.op.as_str()) {
                    normalized.op = "mcx".into();
                    normalized.wires = vec![g.wires.last().unwrap().clone()];
                    normalized.controls = g.control_wires();
                }
                normalized.controls.sort();
                normalized.controls.dedup();
                let targets = g.wires[g.own_controls()..].to_vec();
                let prev = targets.first().and_then(|w| latest.get(w)).copied();
                let matched = prev
                    .filter(|i| targets.iter().all(|w| latest.get(w) == Some(i)))
                    .filter(|i| {
                        let c = &changes[*i];
                        c.targets == targets
                            && c.gate.adjoint() == normalized
                            && c.inputs.iter().all(|(w, v)| {
                                targets.contains(w) || versions.get(w).copied().unwrap_or(0) == *v
                            })
                    });
                if let Some(i) = matched {
                    let c = &changes[i];
                    for (w, prior) in targets.iter().zip(&c.prior) {
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
                    let i = changes.len();
                    changes.push(Change {
                        gate: normalized,
                        inputs,
                        targets: targets.clone(),
                        prior,
                    });
                    for w in targets {
                        versions.insert(w.clone(), next);
                        latest.insert(w, i);
                    }
                    next += 1;
                }
            }
            Operation::Classical { .. } => {}
        }
    }
    if !live.is_empty() {
        return Err(Error::arg("§6: source exits with live workspace"));
    }
    Ok(())
}
