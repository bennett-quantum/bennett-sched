//! Source occurrence events used for §5.3 / §6 version matching. Semantic
//! calls remain atomic; instruction-private constructor scratch is not source
//! workspace. The emitted schedule still contains every prescribed leaf.
use crate::{
    Error, Result, canonical,
    expansion::{Leaf, Operation},
    object::{Object, flattened},
    schedule::Kind,
    source::{Event, Trace},
    target::Gate,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn synthetic(source: &Event, op: Operation) -> Event {
    let mut e = source.clone();
    e.leaf = Leaf {
        ordinal: 0,
        calls: vec![],
        origin: vec![],
        op,
    };
    e
}
fn fingerprint(events: &[Event], internal: &BTreeSet<String>) -> Result<String> {
    let mut names = BTreeMap::new();
    let mut operations = Vec::new();
    for e in events {
        if let Operation::Gate(g) = &e.leaf.op {
            let mut wire = |w: &String| {
                if internal.contains(w) {
                    let n = names.len();
                    names
                        .entry(w.clone())
                        .or_insert_with(|| format!("#{n}"))
                        .clone()
                } else {
                    w.clone()
                }
            };
            operations.push(json!([
                g.op,
                g.wires.iter().map(&mut wire).collect::<Vec<_>>(),
                g.controls.iter().map(&mut wire).collect::<Vec<_>>(),
                g.theta
            ]));
        }
    }
    Ok(canonical::hash(&canonical::to_vec(&json!(operations))?))
}
pub fn events(object: &Object, trace: &Trace, owner: usize) -> Result<Vec<Event>> {
    let invocation = &trace.invocations[owner];
    let function = &object.functions[&invocation.name];
    let points = flattened(&function.body)
        .into_iter()
        .map(|n| (n.point.clone(), n))
        .collect::<BTreeMap<_, _>>();
    let mut children = trace
        .invocations
        .iter()
        .enumerate()
        .filter(|(_, i)| i.parent == Some(owner))
        .collect::<Vec<_>>();
    children.sort_by_key(|(index, i)| {
        (
            i.start,
            if invocation.adjoint {
                usize::MAX - *index
            } else {
                *index
            },
        )
    });
    let mut child_index = 0;
    let mut result = Vec::new();
    let mut index = invocation.start;
    while index < invocation.end || child_index < children.len() {
        if let Some(proof)=trace.qinplaces.iter().find(|p|p.owner==owner&&matches!(trace.events.get(index).map(|e|&e.leaf.op),Some(Operation::Alloc{identity,..})if identity==&p.target_id)) {
            let end=trace.events.iter().enumerate().skip(index+1).find_map(|(i,e)|matches!(&e.leaf.op,Operation::Release{identity,..}if identity==&proof.fault_id).then_some(i+1)).ok_or_else(||Error::arg("qinplace private release missing"))?;
            let mut controls=proof.inputs.clone();controls.extend(proof.controls.clone());controls.sort();controls.dedup();
            let (low,high)=if proof.g<proof.ginv {(&proof.g,&proof.ginv)}else{(&proof.ginv,&proof.g)};
            let identity=canonical::hash(&canonical::to_vec(&json!([low,high,proof.budget,proof.params,proof.target,controls]))?);
            let first=children.iter().filter(|(_,c)|c.start>=index&&c.end<=end).min_by_key(|(_,c)|c.start).ok_or_else(||Error::arg("qinplace call metadata missing"))?.1;
            let op=if low==high {format!("$perm/qinplace/{identity}")}else{format!("$perm/qinplace{}/{identity}",if &first.hash==low{"+"}else{"-"})};
            result.push(synthetic(&trace.events[index],Operation::Gate(Gate{op,wires:proof.target.clone(),controls,theta:None})));
            while child_index<children.len()&&children[child_index].1.start<end {child_index+=1;}
            index=end;continue;
        }
        if let Some((_, child)) = children.get(child_index).filter(|(_, c)| c.start == index) {
            let mut location = crate::schedule::Location {
                root: Default::default(),
                path: child.path.clone(),
                generated: 0,
                kind: Kind::Gate,
            };
            location.path.last_mut().unwrap().call_index = None;
            let targets = child
                .slots
                .iter()
                .filter(|s| ["mut", "mem_mut", "tgt", "fault"].contains(&s.role.as_str()))
                .flat_map(|s| s.wires.clone())
                .collect::<Vec<_>>();
            let mut controls = child.controls.clone();
            for slot in &child.slots {
                if !["mut", "mem_mut", "tgt", "fault"].contains(&slot.role.as_str()) {
                    controls.extend(slot.wires.clone());
                }
            }
            controls.sort();
            controls.dedup();
            let identity = canonical::hash(&canonical::to_vec(&json!([
                child.hash,
                child.params,
                child.budget,
                child
                    .slots
                    .iter()
                    .map(|s| json!([s.id, s.wires]))
                    .collect::<Vec<_>>(),
                controls
            ]))?);
            let oracle = object.functions[&child.name].kind == "oracle";
            let kind = if oracle || object.functions[&child.name].kind == "perm_action" {
                "perm"
            } else {
                "other"
            };
            let sign = if child.adjoint { "-" } else { "+" };
            result.push(Event {
                location,
                owner,
                leaf: Leaf {
                    ordinal: 0,
                    calls: vec![],
                    origin: vec![],
                    op: Operation::Gate(Gate {
                        op: if oracle {
                            format!("$perm/oracle/{identity}")
                        } else {
                            format!("${kind}/call{sign}/{identity}")
                        },
                        wires: targets,
                        controls,
                        theta: None,
                    }),
                },
            });
            index = child.end;
            child_index += 1;
            continue;
        }
        let event = &trace.events[index];
        assert_eq!(
            event.owner, owner,
            "source event outside its direct invocation"
        );
        let point = &event.location.path.last().unwrap().point;
        // K83 image preparation/restoration is source lifetime support, not
        // an explicit computing/cleanup instruction. Keep every leaf for
        // version/lifetime verification without looking up a source row.
        if ["action", "perm_action"].contains(&function.kind.as_str())
            && ["$construct", "$release"].contains(&point.as_str())
        {
            result.push(event.clone());
            index += 1;
            continue;
        }
        let node = points.get(point).ok_or_else(|| {
            Error::arg(format!(
                "source event point {point} is not in {}",
                function.name
            ))
        })?;
        let op = node.op.as_str();
        if [
            "qcopy", "quncopy", "qload", "qunload", "qxor", "qexch", "qstore", "qexchm",
        ]
        .contains(&op)
        {
            let mut end = index + 1;
            while end < invocation.end
                && trace.events[end].owner == owner
                && trace.events[end].location.path == event.location.path
            {
                end += 1;
            }
            let emitted = &trace.events[index..end];
            let mut internal = BTreeSet::new();
            let mut output_alloc = None;
            let mut output_release = None;
            for e in emitted {
                match &e.leaf.op {
                    Operation::Alloc {
                        identity, wires, ..
                    }
                    | Operation::Release {
                        identity, wires, ..
                    } => {
                        if identity.starts_with("source-copy/")
                            || identity.starts_with("source-load/")
                        {
                            if matches!(e.leaf.op, Operation::Alloc { .. }) {
                                output_alloc = Some(e.clone());
                            } else {
                                output_release = Some(e.clone());
                            }
                        } else {
                            internal.extend(wires.clone());
                        }
                    }
                    _ => {}
                }
            }
            if let Some(allocation) = output_alloc.as_ref().or(output_release.as_ref()) {
                let (identity, wires) = match &allocation.leaf.op {
                    Operation::Alloc {
                        identity, wires, ..
                    }
                    | Operation::Release {
                        identity, wires, ..
                    } => (identity, wires),
                    _ => unreachable!(),
                };
                let inputs = emitted
                    .iter()
                    .filter_map(|e| {
                        if let Operation::Gate(g) = &e.leaf.op {
                            Some(g.all_wires())
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .filter(|w| !internal.contains(w) && !wires.contains(w))
                    .collect::<BTreeSet<_>>();
                if let Some(e) = output_alloc.as_ref() {
                    result.push(e.clone());
                }
                result.push(synthetic(
                    event,
                    Operation::Gate(Gate {
                        op: format!("$perm/intrinsic/{identity}"),
                        wires: wires.clone(),
                        controls: inputs.into_iter().collect(),
                        theta: None,
                    }),
                ));
                if let Some(e) = output_release {
                    result.push(e);
                }
            } else {
                let targets = emitted
                    .iter()
                    .filter_map(|e| {
                        if let Operation::Gate(g) = &e.leaf.op {
                            Some(g.wires[g.own_controls()..].to_vec())
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .filter(|w| !internal.contains(w))
                    .collect::<BTreeSet<_>>();
                let inputs = emitted
                    .iter()
                    .filter_map(|e| {
                        if let Operation::Gate(g) = &e.leaf.op {
                            Some(g.all_wires())
                        } else {
                            None
                        }
                    })
                    .flatten()
                    .filter(|w| !internal.contains(w) && !targets.contains(w))
                    .collect::<BTreeSet<_>>();
                result.push(synthetic(
                    event,
                    Operation::Gate(Gate {
                        op: format!("$perm/{op}/{}", fingerprint(emitted, &internal)?),
                        wires: targets.into_iter().collect(),
                        controls: inputs.into_iter().collect(),
                        theta: None,
                    }),
                ));
            }
            index = end;
        } else {
            result.push(event.clone());
            index += 1;
        }
    }
    Ok(result)
}
