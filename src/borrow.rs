//! Syntactic §5.6 translation-lemma proofs over resolved source operations.
use crate::{
    Error, Result, Status,
    expansion::Operation,
    object::{Node, Object},
    source::{Event, Trace},
    target::Gate,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Proof {
    pub owner: usize,
    pub witness: Value,
}

fn flip(g: &Gate) -> Option<(String, BTreeSet<String>)> {
    ["x", "cx", "ccx", "mcx"].contains(&g.op.as_str()).then(|| {
        (
            g.wires.last().unwrap().clone(),
            g.control_wires().into_iter().collect(),
        )
    })
}

/// Return a universal witness only when every side condition holds. Primitive
/// identities include inherited controls, so an adjoint cannot change context.
fn translation(events: &[&Event], borrowed: &BTreeSet<String>) -> Option<Value> {
    let gates = events
        .iter()
        .map(|e| match &e.leaf.op {
            Operation::Gate(g) => flip(g),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if gates.len() % 2 != 0 {
        return None;
    }
    let half = gates.len() / 2;
    for v_len in 1..half {
        let w_len = half - v_len;
        let v = &gates[..v_len];
        let w = &gates[v_len..half];
        let inverse = &gates[half..half + v_len];
        let second = &gates[half + v_len..];
        if !v.iter().rev().eq(inverse) || w != second {
            continue;
        }
        let x = v
            .iter()
            .flat_map(|(_, c)| c.iter().cloned())
            .collect::<BTreeSet<_>>();
        let w_controls = w
            .iter()
            .flat_map(|(_, c)| c.iter().cloned())
            .collect::<BTreeSet<_>>();
        if !x.is_disjoint(borrowed)
            || v.iter().any(|(t, _)| !borrowed.contains(t))
            || w.iter().any(|(t, c)| {
                x.contains(t)
                    || borrowed.contains(t)
                    || w_controls.contains(t)
                    || c.intersection(borrowed).count() > 1
            })
        {
            continue;
        }
        let point = |e: &Event| {
            let p = e.location.path.last().unwrap();
            let (block, ordinal) = p
                .point
                .rsplit_once(':')
                .expect("parsed source program point");
            json!([
                p.symbol,
                block,
                ordinal.parse::<u64>().expect("parsed source ordinal")
            ])
        };
        return Some(
            json!({"V":events[..v_len].iter().map(|e|point(e)).collect::<Vec<_>>(),"W":events[v_len..v_len+w_len].iter().map(|e|point(e)).collect::<Vec<_>>(),"X":x,"B":borrowed,"frame":[point(events[0]),point(events[events.len()-1])]}),
        );
    }
    None
}
fn scopes(nodes: &[Node], scope: &str, map: &mut BTreeMap<String, String>) {
    for node in nodes {
        map.insert(node.point.clone(), scope.into());
        for (i, body) in node.bodies.iter().enumerate() {
            scopes(body, &format!("{}:{i}", node.point), map);
        }
    }
}
fn group(event: &Event, maps: &BTreeMap<String, BTreeMap<String, String>>) -> Result<Vec<u8>> {
    let path = &event.location.path;
    let p = path.last().unwrap();
    crate::canonical::to_vec(&json!([
        event.owner,
        path[..path.len() - 1]
            .iter()
            .map(|p| p.json())
            .collect::<Vec<_>>(),
        p.iteration,
        maps[&p.symbol][&p.point]
    ]))
}
pub fn prove(object: &Object, trace: &Trace) -> Result<Vec<Proof>> {
    let mut maps = BTreeMap::new();
    for f in object.functions.values() {
        let mut map = BTreeMap::new();
        scopes(&f.body, "root", &mut map);
        maps.insert(object.symbols[&f.name].interface_hash.clone(), map);
    }
    let mut proofs = Vec::new();
    for (start, event) in trace.events.iter().enumerate() {
        let Operation::Alloc {
            identity,
            wires,
            state,
            ..
        } = &event.leaf.op
        else {
            continue;
        };
        if state != "borrowed" {
            continue;
        }
        let end = trace
            .events
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(i, e)| {
                matches!(&e.leaf.op,Operation::Release{identity:id,..}if id==identity).then_some(i)
            })
            .ok_or_else(|| Error::arg("unreturned borrowed bundle"))?;
        let borrowed = wires.iter().cloned().collect::<BTreeSet<_>>();
        let mut groups: BTreeMap<Vec<u8>, (usize, usize)> = BTreeMap::new();
        for (i, e) in trace.events.iter().enumerate().take(end).skip(start + 1) {
            if matches!(&e.leaf.op,Operation::Gate(g)if g.all_wires().iter().any(|w|borrowed.contains(w)))
            {
                let key = group(e, &maps)?;
                groups
                    .entry(key)
                    .and_modify(|range| range.1 = i)
                    .or_insert((i, i));
            }
        }
        for (key, (first, last)) in groups {
            let events = trace.events[first..=last].iter().collect::<Vec<_>>();
            if events
                .iter()
                .any(|e| group(e, &maps).ok().as_ref() != Some(&key))
            {
                return Err(Error::new(
                    Status::GateUnsupported,
                    "§5.6: borrow-frame proof across nested static sequences is postponed (bennett-sched-ark)",
                ));
            }
            let witness = translation(&events, &borrowed).ok_or_else(|| {
                Error::new(
                    Status::GateUnsupported,
                    "§5.6: non-translation borrow-frame T3 proof is postponed (bennett-sched-ark)",
                )
            })?;
            proofs.push(Proof {
                owner: event.owner,
                witness,
            });
        }
    }
    Ok(proofs)
}
