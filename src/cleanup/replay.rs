//! K80 replay uses source callees, not flattened inverse constructor leaves.
use super::{Pair, rebased, same_source};
use crate::{
    Error, Result, Status, canonical,
    expansion::{Operation, inverse},
    schedule::{Location, Occurrence},
    source::{Event, Trace},
    target::Gate,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
enum Unit {
    Leaf(usize),
    Call(usize),
}

fn at_source(path: &[Occurrence], source: &Location) -> bool {
    path.len() >= source.path.len()
        && path
            .iter()
            .zip(&source.path)
            .enumerate()
            .all(|(n, (a, b))| {
                if n + 1 == source.path.len() {
                    a.symbol == b.symbol
                        && a.point == b.point
                        && a.iteration == b.iteration
                        && a.instance == b.instance
                } else {
                    a == b
                }
            })
}

fn units(trace: &Trace, owner: usize, locations: &[Location]) -> Vec<Unit> {
    let mut selected = Vec::new();
    for (i, inv) in trace.invocations.iter().enumerate() {
        if inv.parent == Some(owner) && locations.iter().any(|l| at_source(&inv.path, l)) {
            selected.push((inv.start, 0, i, Unit::Call(i)));
        }
    }
    for (i, e) in trace.events.iter().enumerate() {
        if e.owner == owner && locations.iter().any(|l| same_source(&e.location, l)) {
            selected.push((i, 1, i, Unit::Leaf(i)));
        }
    }
    selected.sort_by_key(|(offset, kind, index, _)| (*offset, *kind, *index));
    selected.into_iter().map(|(_, _, _, u)| u).collect()
}

fn writes(trace: &Trace, unit: &Unit) -> Vec<String> {
    match unit {
        Unit::Leaf(i) => super::writes(&trace.events[*i]),
        Unit::Call(i) => trace.invocations[*i]
            .slots
            .iter()
            .filter(|s| ["mut", "tgt", "fault", "mem_mut"].contains(&s.role.as_str()))
            .flat_map(|s| s.wires.clone())
            .collect(),
    }
}
fn reads(trace: &Trace, unit: &Unit) -> Vec<String> {
    match unit {
        Unit::Leaf(i) => match &trace.events[*i].leaf.op {
            Operation::Gate(g) => g.wires.iter().chain(&g.controls).cloned().collect(),
            _ => vec![],
        },
        Unit::Call(i) => trace.invocations[*i]
            .slots
            .iter()
            .flat_map(|s| s.wires.clone())
            .chain(trace.invocations[*i].controls.clone())
            .collect(),
    }
}

/// Find the exact allocated input history, without taking ownership of it.
/// Interfaces that have no source reconstruction remain explicitly tracked.
fn history(trace: &Trace, pair: &Pair, before: usize) -> Result<Vec<Unit>> {
    let mut locations = Vec::new();
    for e in &trace.events[..before] {
        if e.owner == pair.owner && !locations.iter().any(|l| same_source(l, &e.location)) {
            locations.push(e.location.clone());
        }
    }
    for i in &trace.invocations {
        if i.parent == Some(pair.owner) && i.start < before {
            let mut l = Location {
                root: Default::default(),
                path: i.path.clone(),
                generated: 0,
                kind: crate::schedule::Kind::Gate,
            };
            l.path.last_mut().unwrap().call_index = None;
            locations.push(l);
        }
    }
    let prefix = units(trace, pair.owner, &locations);
    let mut needed = pair.targets.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = Vec::new();
    let mut roots = BTreeSet::new();
    for slot in &trace.invocations[pair.owner].slots {
        if ["in", "mem_in"].contains(&slot.role.as_str()) {
            roots.extend(slot.wires.clone());
        }
    }
    roots.extend(trace.invocations[pair.owner].controls.clone());
    for unit in prefix.into_iter().rev() {
        if let Unit::Leaf(i) = &unit {
            match &trace.events[*i].leaf.op {
                Operation::Alloc { wires, state, .. }
                    if wires.iter().any(|w| needed.contains(w)) =>
                {
                    if state != "clean" {
                        return Err(deferred("replaying borrowed entry history"));
                    }
                    for w in wires {
                        needed.remove(w);
                    }
                    selected.push(unit);
                    continue;
                }
                Operation::Release { wires, .. } if wires.iter().any(|w| needed.contains(w)) => {
                    return Err(Error::arg("recompute history crosses a released lifetime"));
                }
                _ => {}
            }
        }
        if writes(trace, &unit).iter().any(|w| needed.contains(w)) {
            needed.extend(reads(trace, &unit));
            selected.push(unit);
        }
    }
    if !needed.is_subset(&roots) {
        return Err(deferred(
            "pre-call history of an arbitrary mutable interface",
        ));
    }
    selected.reverse();
    selected.retain(|unit| !matches!(unit,Unit::Leaf(i) if matches!(&trace.events[*i].leaf.op,Operation::Alloc{wires,..} if wires.iter().any(|w|pair.targets.contains(w)))));
    for unit in &selected {
        let permutation = match unit {
            Unit::Call(i) => ["oracle", "perm_action"].contains(
                &trace.object.functions[&trace.invocations[*i].name]
                    .kind
                    .as_str(),
            ),
            Unit::Leaf(i) => match &trace.events[*i].leaf.op {
                Operation::Gate(g) => ["x", "cx", "ccx", "mcx", "swap"].contains(&g.op.as_str()),
                Operation::Alloc { .. } | Operation::Release { .. } => true,
                Operation::Classical { .. } => false,
            },
        };
        if !permutation {
            return Err(Error::arg(
                "§6.4/K81: recompute input history must be a permutation computation",
            ));
        }
    }
    Ok(selected)
}
fn deferred(feature: &str) -> Error {
    Error::new(
        Status::GateUnsupported,
        format!(
            "§11 K81: {feature} requires the remaining source-history algorithm (bennett-sched-7va)"
        ),
    )
}

struct Emitter<'a> {
    source: &'a Trace,
    staged: &'a mut Trace,
    pair: &'a Pair,
    entries: &'a [crate::schedule::Entry],
    identity: String,
    events: Vec<Event>,
    ordinal: u64,
    call_index: u64,
}
impl Emitter<'_> {
    fn reserve(&mut self, count: usize) -> Result<()> {
        if count > 1_000_000usize.saturating_sub(self.events.len()) {
            return Err(Error::new(
                Status::NoMem,
                "recompute leaf descriptors exhaust host workspace (bennett-sched-2u4)",
            ));
        }
        self.events.try_reserve(count).map_err(|_| {
            Error::new(
                Status::NoMem,
                "recompute leaf descriptors exceed host memory",
            )
        })
    }
    fn caller(&mut self, op: Operation) -> Result<()> {
        self.reserve(1)?;
        let source = self
            .source
            .events
            .iter()
            .find(|e| at_source(&e.location.path, &self.pair.carrier))
            .unwrap_or(&self.source.events[0]);
        let mut event = rebased(source, &self.pair.carrier, op, self.ordinal);
        event.owner = self.pair.owner;
        self.ordinal += 1;
        self.events.push(event);
        Ok(())
    }
    fn sequence(
        &mut self,
        units: &[Unit],
        mapping: &mut BTreeMap<String, String>,
        backwards: bool,
        scope: &str,
    ) -> Result<()> {
        let ordered = if backwards {
            units.iter().rev().collect::<Vec<_>>()
        } else {
            units.iter().collect()
        };
        for unit in ordered {
            match unit {
                Unit::Call(i) => self.call(*i, mapping, backwards)?,
                Unit::Leaf(i) => {
                    let original = &self.source.events[*i];
                    let op = if backwards {
                        inverse(std::slice::from_ref(&original.leaf))
                            .pop()
                            .unwrap()
                            .op
                    } else {
                        original.leaf.op.clone()
                    };
                    if let Operation::Alloc {
                        identity, wires, ..
                    }
                    | Operation::Release {
                        identity, wires, ..
                    } = &op
                    {
                        // The enclosing fresh buffer supplies target lifetimes.
                        if wires.iter().any(|w| self.pair.targets.contains(w)) {
                            continue;
                        }
                        let id = format!("{}/{scope}/{identity}", self.identity);
                        mapping
                            .entry(identity.clone())
                            .or_insert_with(|| id.clone());
                        for (bit, w) in wires.iter().enumerate() {
                            mapping
                                .entry(w.clone())
                                .or_insert_with(|| format!("{id}[{bit}]"));
                        }
                    }
                    self.caller(map_op(op, mapping)?)?;
                }
            }
        }
        Ok(())
    }
    fn call(
        &mut self,
        index: usize,
        mapping: &BTreeMap<String, String>,
        backwards: bool,
    ) -> Result<()> {
        let original = &self.source.invocations[index];
        let mut child = Trace::build_oriented(
            &self.source.object,
            &original.name,
            u32::try_from(original.controls.len())
                .map_err(|_| Error::new(Status::NoMem, "replay controls exceed host"))?,
            &original.params,
            original.budget,
            original.adjoint ^ backwards,
        )?;
        let number = self.call_index;
        self.call_index += 1;
        let prefix = format!("{}/call/{number}/", self.identity);
        let mut names = BTreeMap::new();
        for (old, new) in child.inherited.iter().zip(&original.controls) {
            names.insert(old.clone(), mapped(new, mapping));
        }
        for (old, new) in child.slots.iter().zip(&original.slots) {
            if old.id != new.id || old.wires.len() != new.wires.len() {
                return Err(Error::arg("recompute source interface changed"));
            }
            for (old, new) in old.wires.iter().zip(&new.wires) {
                names.insert(old.clone(), mapped(new, mapping));
            }
        }
        for e in &child.events {
            if let Operation::Alloc {
                identity, wires, ..
            } = &e.leaf.op
            {
                names.insert(identity.clone(), format!("{prefix}{identity}"));
                for w in wires {
                    names.insert(w.clone(), format!("{prefix}{w}"));
                }
            }
        }
        let oracle = self.source.object.functions[&original.name].kind == "oracle";
        let mut edge = self.pair.carrier.path.clone();
        edge.last_mut().unwrap().call_index = Some(number);
        edge.last_mut().unwrap().entered_oracle = oracle.then_some(Default::default());
        child.inherited = child.inherited.iter().map(|w| mapped(w, &names)).collect();
        for slot in &mut child.slots {
            slot.wires = slot.wires.iter().map(|w| mapped(w, &names)).collect();
        }
        for inv in &mut child.invocations {
            inv.controls = inv.controls.iter().map(|w| mapped(w, &names)).collect();
            for slot in &mut inv.slots {
                slot.wires = slot.wires.iter().map(|w| mapped(w, &names)).collect();
            }
            inv.path = with_prefix(&edge, &inv.path);
        }
        for e in &mut child.events {
            e.leaf.op = map_op(e.leaf.op.clone(), &names)?;
            let mut prefix = edge.clone();
            if oracle {
                prefix.last_mut().unwrap().entered_oracle = Some(e.location.root);
            }
            e.location.path = with_prefix(&prefix, &e.location.path);
            e.location.root = self.pair.carrier.root;
        }
        for pair in &mut child.cleanups {
            pair.targets = pair.targets.iter().map(|w| mapped(w, &names)).collect();
            bind_location(&mut pair.carrier, &edge, self.pair.carrier.root);
            for l in pair.computation.iter_mut().chain(&mut pair.cleanup) {
                bind_location(l, &edge, self.pair.carrier.root);
            }
        }
        for proof in &mut child.borrow_proofs {
            map_witness(&mut proof.witness, &names);
        }
        for proof in &mut child.qinplaces {
            proof.target_id = mapped(&proof.target_id, &names);
            proof.fault_id = mapped(&proof.fault_id, &names);
            for wires in [&mut proof.target, &mut proof.inputs, &mut proof.controls] {
                for w in wires {
                    *w = mapped(w, &names);
                }
            }
        }
        // Fresh paths are already concrete when selecting nested cleanup.
        child.realise_cleanup_entries(self.entries)?;
        self.reserve(child.events.len())?;
        let base = self.staged.invocations.len();
        let offset = self.events.len();
        for mut inv in child.invocations {
            inv.parent = Some(inv.parent.map_or(self.pair.owner, |p| base + p));
            inv.start += offset;
            inv.end += offset;
            self.staged.invocations.push(inv);
        }
        for mut e in child.events {
            e.owner += base;
            self.events.push(e);
        }
        for mut pair in child.cleanups {
            pair.owner += base;
            self.staged.cleanups.push(pair);
        }
        for mut proof in child.borrow_proofs {
            proof.owner += base;
            self.staged.borrow_proofs.push(proof);
        }
        for mut proof in child.qinplaces {
            proof.owner += base;
            self.staged.qinplaces.push(proof);
        }
        Ok(())
    }
}
fn mapped(w: &str, names: &BTreeMap<String, String>) -> String {
    names.get(w).cloned().unwrap_or_else(|| w.into())
}
fn map_op(mut op: Operation, names: &BTreeMap<String, String>) -> Result<Operation> {
    match &mut op {
        Operation::Gate(g) => {
            for w in g.wires.iter_mut().chain(&mut g.controls) {
                *w = mapped(w, names);
            }
        }
        Operation::Alloc {
            identity, wires, ..
        }
        | Operation::Release {
            identity, wires, ..
        } => {
            *identity = mapped(identity, names);
            for w in wires {
                *w = mapped(w, names);
            }
        }
        Operation::Classical { .. } => return Err(Error::arg("classical leaf in quantum replay")),
    }
    Ok(op)
}
fn with_prefix(prefix: &[Occurrence], path: &[Occurrence]) -> Vec<Occurrence> {
    prefix.iter().chain(path).cloned().collect()
}
fn bind_location(l: &mut Location, prefix: &[Occurrence], root: crate::schedule::Coordinate) {
    l.path = with_prefix(prefix, &l.path);
    l.root = root;
}
fn map_witness(value: &mut serde_json::Value, names: &BTreeMap<String, String>) {
    match value {
        serde_json::Value::String(w) => *w = mapped(w, names),
        serde_json::Value::Array(a) => {
            for v in a {
                map_witness(v, names);
            }
        }
        serde_json::Value::Object(o) => {
            for v in o.values_mut() {
                map_witness(v, names);
            }
        }
        _ => {}
    }
}
fn buffer(identity: String, wires: Vec<String>, release: bool) -> Operation {
    if release {
        Operation::Release {
            identity,
            wires,
            class: "any".into(),
            state: "clean".into(),
        }
    } else {
        Operation::Alloc {
            identity,
            wires,
            class: "any".into(),
            state: "clean".into(),
        }
    }
}

pub(super) fn replace(
    source: &Trace,
    staged: &mut Trace,
    pair_index: usize,
    entries: &[crate::schedule::Entry],
) -> Result<()> {
    let pair = &source.cleanups[pair_index];
    let computation = units(source, pair.owner, &pair.computation);
    let first = computation
        .first()
        .map(|u| match u {
            Unit::Leaf(i) => *i,
            Unit::Call(i) => source.invocations[*i].start,
        })
        .ok_or_else(|| Error::arg("semantic replay has no computation"))?;
    let history = history(source, pair, first)?;
    let identity = format!(
        "recompute/{pair_index}/{}",
        canonical::hash(&canonical::to_vec(&json!(
            pair.carrier
                .path
                .iter()
                .map(Occurrence::json)
                .collect::<Vec<_>>()
        ))?)
    );
    let fresh = pair
        .targets
        .iter()
        .enumerate()
        .map(|(i, _)| format!("{identity}[{i}]"))
        .collect::<Vec<_>>();
    let mut mapping = pair
        .targets
        .iter()
        .cloned()
        .zip(fresh.clone())
        .collect::<BTreeMap<_, _>>();
    let start_inv = staged.invocations.len();
    let mut emitter = Emitter {
        source,
        staged,
        pair,
        entries,
        identity: identity.clone(),
        events: vec![],
        ordinal: 0,
        call_index: 0,
    };
    emitter.caller(buffer(identity.clone(), fresh.clone(), false))?;
    emitter.sequence(&history, &mut mapping, false, "input-history")?;
    emitter.sequence(&computation, &mut mapping, false, "computation")?;
    // A zero-based replay computes f(h) XOR h, with exact reconstructed h.
    // Its inverse after the XOR leaves the fresh buffer clean and retains h.
    let contexts = computation
        .iter()
        .filter_map(|unit| match unit {
            Unit::Call(i) => Some(source.invocations[*i].controls.clone()),
            Unit::Leaf(i) => match &source.events[*i].leaf.op {
                Operation::Gate(g) => Some(g.controls.clone()),
                _ => None,
            },
        })
        .collect::<Vec<_>>();
    let mut context = contexts
        .first()
        .cloned()
        .unwrap_or_else(|| source.invocations[pair.owner].controls.clone());
    for controls in contexts.iter().skip(1) {
        context.retain(|w| controls.contains(w));
    }
    for pass in 0..2 {
        let h_id = format!("{identity}/baseline/{pass}");
        let h = pair
            .targets
            .iter()
            .enumerate()
            .map(|(i, _)| format!("{h_id}[{i}]"))
            .collect::<Vec<_>>();
        if !history.is_empty() {
            let mut hm = pair.targets.iter().cloned().zip(h.clone()).collect();
            emitter.caller(buffer(h_id.clone(), h.clone(), false))?;
            emitter.sequence(&history, &mut hm, false, &format!("baseline/{pass}"))?;
            for (h, u) in h.iter().zip(&fresh) {
                emitter.caller(Operation::Gate(Gate {
                    op: "cx".into(),
                    wires: vec![h.clone(), u.clone()],
                    controls: context.clone(),
                    theta: None,
                }))?;
            }
            emitter.sequence(&history, &mut hm, true, &format!("baseline/{pass}"))?;
            emitter.caller(buffer(h_id, h, true))?;
        }
        if pass == 0 {
            for (u, target) in fresh.iter().zip(&pair.targets) {
                emitter.caller(Operation::Gate(Gate {
                    op: "cx".into(),
                    wires: vec![u.clone(), target.clone()],
                    controls: context.clone(),
                    theta: None,
                }))?;
            }
        }
    }
    emitter.sequence(&computation, &mut mapping, true, "computation")?;
    emitter.sequence(&history, &mut mapping, true, "input-history")?;
    emitter.caller(buffer(identity, fresh, true))?;
    for unit in units(source, pair.owner, &pair.cleanup) {
        if let Unit::Leaf(i) = unit {
            if matches!(&source.events[i].leaf.op,Operation::Release{wires,..}if wires.iter().any(|w|pair.targets.contains(w)))
            {
                emitter.caller(source.events[i].leaf.op.clone())?;
            }
        }
    }
    let insertion = emitter
        .staged
        .events
        .iter()
        .position(|e| at_source(&e.location.path, &pair.carrier))
        .or_else(|| {
            emitter
                .staged
                .invocations
                .iter()
                .take(start_inv)
                .find(|i| i.parent == Some(pair.owner) && at_source(&i.path, &pair.carrier))
                .map(|i| i.start)
        })
        .ok_or_else(|| Error::arg("recompute carrier has no emitted source occurrence"))?;
    let replacement = std::mem::take(&mut emitter.events);
    let mut emitted = Some(replacement);
    let mut events = Vec::new();
    let mut boundaries = vec![0; emitter.staged.events.len() + 1];
    let mut output_insertion = None;
    for (i, e) in emitter.staged.events.iter().enumerate() {
        boundaries[i] = events.len();
        if i == insertion {
            output_insertion = Some(events.len());
            events.extend(emitted.take().unwrap());
        }
        if !pair.cleanup.iter().any(|l| at_source(&e.location.path, l)) {
            events.push(e.clone());
        }
    }
    boundaries[emitter.staged.events.len()] = events.len();
    if let Some(replacement) = emitted {
        if insertion != emitter.staged.events.len() {
            return Err(Error::arg(
                "recompute insertion is outside the emitted trace",
            ));
        }
        output_insertion = Some(events.len());
        events.extend(replacement);
    }
    for inv in &mut emitter.staged.invocations[..start_inv] {
        if inv.start == inv.end {
            inv.start = boundaries[inv.start];
            inv.end = inv.start;
        }
    }
    let offset = output_insertion.expect("recompute insertion emitted");
    for inv in &mut emitter.staged.invocations[start_inv..] {
        inv.start += offset;
        inv.end += offset;
    }
    emitter.staged.events = events;
    Ok(())
}
pub(super) fn owner_replaced(source: &Trace, staged: &Trace, owner: usize) -> bool {
    if owner >= source.invocations.len() {
        return false;
    }
    staged
        .cleanups
        .iter()
        .take(source.cleanups.len())
        .any(|pair| {
            pair.realisation == "recompute"
                && source.invocations[owner].parent.is_some()
                && pair
                    .cleanup
                    .iter()
                    .any(|l| at_source(&source.invocations[owner].path, l))
                && crate::source::is_descendant(owner, pair.owner, &source.invocations)
        })
}
pub(super) fn remove_replaced_invocations(source: &Trace, staged: &mut Trace) -> Result<()> {
    let removed = (0..source.invocations.len())
        .filter(|i| owner_replaced(source, staged, *i))
        .collect::<BTreeSet<_>>();
    let map = (0..staged.invocations.len())
        .filter(|i| !removed.contains(i))
        .enumerate()
        .map(|(new, old)| (old, new))
        .collect::<BTreeMap<_, _>>();
    for e in &mut staged.events {
        e.owner = *map
            .get(&e.owner)
            .ok_or_else(|| Error::arg("removed call retained an emitted leaf"))?;
    }
    staged.invocations = staged
        .invocations
        .iter()
        .enumerate()
        .filter_map(|(i, inv)| {
            map.get(&i).map(|_| {
                let mut inv = inv.clone();
                inv.parent = inv.parent.map(|p| map[&p]);
                inv
            })
        })
        .collect();
    staged.cleanups.retain(|p| !removed.contains(&p.owner));
    for p in &mut staged.cleanups {
        p.owner = map[&p.owner];
    }
    staged.borrow_proofs.retain(|p| !removed.contains(&p.owner));
    for p in &mut staged.borrow_proofs {
        p.owner = map[&p.owner];
    }
    staged.qinplaces.retain(|p| !removed.contains(&p.owner));
    for p in &mut staged.qinplaces {
        p.owner = map[&p.owner];
    }
    Ok(())
}
