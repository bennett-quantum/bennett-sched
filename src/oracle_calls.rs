//! Lifted semantic rows bind source interfaces before the ref forwarding call.
use crate::{
    Error, Result,
    expansion::{Binding, Env, Library},
    object::{self, Node},
    oracle::Source,
    source_registry::OracleInvocation,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub(crate) fn row(
    node: &Node,
    index: u64,
    env: &Env,
    locations: &BTreeMap<String, u64>,
    source: &Source<'_>,
    library: &mut Library,
    bindings: &mut Env,
) -> Result<Value> {
    let oracle = node.op != "lift.perm_call";
    let operands = &node.operands;
    if node.lhs.is_some() || operands.len() != if oracle { 7 } else { 5 } {
        return Err(Error::arg("§4.2: lifted semantic call operand count"));
    }
    let callee = operands[0]
        .strip_prefix('@')
        .and_then(|n| source.object.functions.get(n))
        .ok_or_else(|| Error::arg("unresolved lifted semantic callee"))?;
    if callee.kind != if oracle { "oracle" } else { "perm_action" } {
        return Err(Error::arg("lifted semantic call tier mismatch"));
    }
    let actuals = if oracle {
        let mut inputs = object::list(&operands[2])?;
        if inputs.len() != callee.groups[0].len() {
            return Err(Error::arg("lifted oracle input slot count mismatch"));
        }
        inputs.extend([operands[3].clone(), operands[4].clone()]);
        inputs
    } else {
        let mut mutable = object::list(&operands[1])?;
        if mutable.len() != callee.groups[0].len() {
            return Err(Error::arg("lifted permutation mutable slot count mismatch"));
        }
        let inputs = object::list(&operands[2])?;
        if inputs.len() != callee.groups[1].len() {
            return Err(Error::arg("lifted permutation input slot count mismatch"));
        }
        mutable.extend(inputs);
        mutable
    };
    let wire_groups = if oracle { 3 } else { 2 };
    let mut values = Vec::new();
    let mut used = BTreeSet::new();
    let mut position = 0;
    for (group, parameters) in callee.groups[..wire_groups].iter().enumerate() {
        for p in parameters {
            let actual = &actuals[position];
            position += 1;
            let mutable = if oracle { group > 0 } else { group == 0 };
            if mutable && !locations.contains_key(actual) {
                return Err(Error::arg(
                    "§4.2/§5.1: lifted mutable operands must be declared locations",
                ));
            }
            let wires = crate::oracle::read(actual, env)?;
            if wires.len() as u64 != object::wire_width(&p.ty)? {
                return Err(Error::arg("lifted semantic wire width mismatch"));
            }
            if wires.iter().any(|w| !used.insert(w.clone())) {
                return Err(Error::arg("lifted semantic interfaces overlap"));
            }
            values.push(Binding::Wires(wires));
        }
    }
    let memory = object::list(&operands[if oracle { 5 } else { 3 }])?;
    if memory.len() != callee.groups[wire_groups].len() {
        return Err(Error::arg("lifted semantic region count mismatch"));
    }
    for (p, name) in callee.groups[wire_groups].iter().zip(memory) {
        let Some(
            binding @ Binding::Region {
                address,
                cell,
                role,
                wires,
                ..
            },
        ) = env.get(&name)
        else {
            return Err(Error::arg(
                "lifted memory operand must be a forwarded interface region",
            ));
        };
        if object::region_shape(&p.ty)? != (*address, *cell)
            || p.role.as_deref() != Some(role.as_str())
            || wires.iter().any(|w| !used.insert(w.clone()))
        {
            return Err(Error::arg(
                "lifted semantic region shape/role/disjointness mismatch",
            ));
        }
        values.push(binding.clone());
    }
    let actual_args = object::list(&operands[if oracle { 6 } else { 4 }])?;
    if actual_args.len() != callee.groups[wire_groups + 1].len() {
        return Err(Error::arg(
            "lifted semantic classical argument count mismatch",
        ));
    }
    let mut params = Vec::new();
    for (p, name) in callee.groups[wire_groups + 1].iter().zip(actual_args) {
        let Some(Binding::Value { isa_type, value }) = env.get(&name) else {
            return Err(Error::arg(
                "§4.2: lifted args must be fixed classical arguments, never locations",
            ));
        };
        let expected = if p.ty == "ptr" {
            json!({"ptr":source.object.ptr_width})
        } else {
            json!(p.ty)
        };
        if isa_type != &expected && !(p.ty == "ptr" && isa_type == "ptr") {
            return Err(Error::arg(
                "lifted semantic classical argument type mismatch",
            ));
        }
        params.push(value.clone());
        values.push(Binding::Value {
            isa_type: expected,
            value: value.clone(),
        });
    }
    source
        .object
        .validate_params(&callee.name, &json!(params))?;
    let context = if oracle {
        let Some(Binding::Value { isa_type, value }) = env.get(&operands[1]) else {
            return Err(Error::arg("lifted oracle requires a fixed token argument"));
        };
        if isa_type != "token" {
            return Err(Error::arg("lifted oracle requires a token argument"));
        }
        let invocation = OracleInvocation::parse(&json!({"token":value}))?;
        source.object.validate_budget(invocation.token())?;
        Some(invocation)
    } else {
        None
    };
    let hash = source.object.closure_hash(&callee.name)?.to_owned();
    if !library.symbols.contains_key(&hash) {
        library.link_source(Arc::new(source.object.clone()), &callee.name)?;
    }
    let symbol = format!("semantic-row/{index}/symbol");
    let arguments = format!("semantic-row/{index}/arguments");
    bindings.insert(symbol.clone(), Binding::Symbol(hash.clone()));
    bindings.insert(
        arguments.clone(),
        Binding::Arguments {
            symbol: hash,
            values,
            oracle_invocation: context,
        },
    );
    Ok(json!({"invoke":node.op,"args":[symbol,arguments]}))
}

#[derive(Default)]
pub(crate) struct Projection {
    pub invocations: Vec<crate::source::Invocation>,
    pub cleanups: Vec<crate::cleanup::Pair>,
    pub borrow_proofs: Vec<crate::borrow::Proof>,
    pub qinplaces: Vec<crate::qinplace::Proof>,
    groups: BTreeMap<usize, (usize, usize)>,
}
fn reversed_coordinate(
    mut coordinate: crate::schedule::Coordinate,
    reverse: bool,
) -> crate::schedule::Coordinate {
    if reverse {
        coordinate.phase = coordinate.phase.map(|p| match p {
            crate::schedule::Phase::Fwd => crate::schedule::Phase::Rev,
            crate::schedule::Phase::Rev => crate::schedule::Phase::Fwd,
        });
    }
    coordinate
}
fn rebind_location(
    location: &mut crate::schedule::Location,
    prefix: &[crate::schedule::Occurrence],
    root: crate::schedule::Coordinate,
    reverse: bool,
) {
    for point in &mut location.path {
        point.entered_oracle = point
            .entered_oracle
            .map(|c| reversed_coordinate(c, reverse));
    }
    let mut edge = prefix.to_vec();
    if edge.last().is_some_and(|p| p.entered_oracle.is_some()) {
        edge.last_mut().unwrap().entered_oracle = Some(reversed_coordinate(location.root, reverse));
    }
    edge.extend(std::mem::take(&mut location.path));
    location.path = edge;
    location.root = root;
}
impl Projection {
    pub(crate) fn register(
        &mut self,
        call: &crate::source_registry::SourceCall,
        parent: &crate::schedule::Location,
    ) -> Result<()> {
        let trace = &call.trace;
        let reverse = call.reverse;
        let index = call.start;
        assert_eq!(call.end - call.start, trace.events.len());
        let key = Arc::as_ptr(trace) as usize;
        if self.groups.contains_key(&key) {
            return Err(Error::new(
                crate::Status::InvalidState,
                "source call provenance was registered twice",
            ));
        }
        let mut edge = parent.path.clone();
        edge.last_mut().unwrap().call_index = Some(0);
        let oracle = trace.object.functions[&trace.invocations[0].name].kind == "oracle";
        edge.last_mut().unwrap().entered_oracle = oracle.then_some(Default::default());
        if reverse && !trace.borrow_proofs.is_empty() {
            return Err(Error::new(
                crate::Status::GateUnsupported,
                "§11: raw-inverse lifted callee borrow-frame proof rebinding remains bennett-sched-y0n",
            ));
        }
        let base = 1 + self.invocations.len();
        for old in &trace.invocations {
            let mut inv = old.clone();
            let (start, end) = if reverse {
                (trace.events.len() - old.end, trace.events.len() - old.start)
            } else {
                (old.start, old.end)
            };
            inv.start = index + start;
            inv.end = index + end;
            inv.parent = Some(old.parent.map_or(0, |p| base + p));
            let mut path = edge.clone();
            path.extend(inv.path);
            inv.path = path;
            for p in inv.path.iter_mut().skip(edge.len()) {
                p.entered_oracle = p.entered_oracle.map(|c| reversed_coordinate(c, reverse));
            }
            self.invocations.push(inv);
        }
        for old in &trace.cleanups {
            let mut pair = old.clone();
            pair.owner += base;
            rebind_location(&mut pair.carrier, &edge, parent.root, reverse);
            for l in pair.computation.iter_mut().chain(&mut pair.cleanup) {
                rebind_location(l, &edge, parent.root, reverse);
            }
            self.cleanups.push(pair);
        }
        for old in &trace.qinplaces {
            let mut p = old.clone();
            p.owner += base;
            self.qinplaces.push(p);
        }
        for old in &trace.borrow_proofs {
            let mut p = old.clone();
            p.owner += base;
            self.borrow_proofs.push(p);
        }
        self.groups.insert(key, (base, index));

        Ok(())
    }
    pub(crate) fn event(
        &mut self,
        leaf: &crate::expansion::Leaf,
        parent: &crate::schedule::Location,
        index: usize,
    ) -> Result<Option<(crate::schedule::Location, usize)>> {
        let Some(origin) = leaf.origin.iter().rev().find(|o| o.source.is_some()) else {
            return Ok(None);
        };
        let source = origin.source.as_ref().unwrap();
        let trace = &source.trace;
        let reverse = origin.reverse;
        let mut edge = parent.path.clone();
        edge.last_mut().unwrap().call_index = Some(0);
        let oracle = trace.object.functions[&trace.invocations[0].name].kind == "oracle";
        edge.last_mut().unwrap().entered_oracle = oracle.then_some(Default::default());
        let key = Arc::as_ptr(&source.trace) as usize;
        let base = self
            .groups
            .get(&key)
            .map(|(base, _)| *base)
            .ok_or_else(|| {
                Error::new(
                    crate::Status::InvalidState,
                    "native source event has no call metadata",
                )
            })?;
        let (_, start) = self.groups[&key];
        let expected = if reverse {
            trace.events.len() - 1 - (index - start)
        } else {
            index - start
        };
        if expected != source.event {
            return Err(Error::new(
                crate::Status::InvalidState,
                "linked source native trace lost contiguous event provenance",
            ));
        }
        let event = &trace.events[source.event];
        let mut location = event.location.clone();
        rebind_location(&mut location, &edge, parent.root, reverse);
        location.generated = leaf.ordinal;
        location.kind = match leaf.op {
            crate::expansion::Operation::Gate(_) => crate::schedule::Kind::Gate,
            crate::expansion::Operation::Alloc { .. } => crate::schedule::Kind::Alloc,
            crate::expansion::Operation::Release { .. } => crate::schedule::Kind::Release,
            crate::expansion::Operation::Classical { .. } => crate::schedule::Kind::Classical,
        };
        Ok(Some((location, base + event.owner)))
    }
}

pub(crate) fn constructor_location(
    provenance: &[crate::expansion::Origin],
    rows: &[(String, String, Vec<u64>)],
    root: &str,
) -> (crate::schedule::Location, usize) {
    use crate::schedule::{Coordinate, Kind, Location, Occurrence, Phase};
    let state = provenance
        .iter()
        .rev()
        .find(|o| o.constructor == "visit" && o.name == "State");
    let frame = provenance
        .iter()
        .rev()
        .find(|o| o.constructor == "visit" && o.name == "Frames");
    let (point, coordinate, space) = if let Some(state) = state {
        (
            if state.reverse {
                "$release"
            } else {
                "$construct"
            }
            .into(),
            Coordinate::default(),
            rows.len() + usize::from(state.reverse),
        )
    } else if let Some(frame) = frame {
        let row = provenance
            .iter()
            .rev()
            .find(|o| o.constructor == "apply" && (o.name == "Rows" || o.name == "Next"))
            .map(|o| o.index)
            .unwrap_or(usize::MAX);
        (
            rows.get(row)
                .map(|r| r.1.clone())
                .unwrap_or_else(|| "$step".into()),
            Coordinate {
                step: Some(frame.index as u64 + 1),
                phase: Some(if frame.reverse {
                    Phase::Rev
                } else {
                    Phase::Fwd
                }),
            },
            row,
        )
    } else {
        ("$commit".into(), Coordinate::default(), rows.len() + 2)
    };
    (
        Location {
            root: coordinate,
            path: vec![Occurrence {
                symbol: rows
                    .get(space)
                    .map(|r| r.0.clone())
                    .unwrap_or_else(|| root.into()),
                point,
                iteration: vec![],
                instance: rows.get(space).map(|r| r.2.clone()).unwrap_or_default(),
                call_index: None,
                entered_oracle: None,
            }],
            generated: 0,
            kind: Kind::Gate,
        },
        space,
    )
}
