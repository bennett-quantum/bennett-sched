//! K65/K81 source cleanup carriers. Source inverses are assigned once, before
//! selecting realisations and without entering semantic callee subtrees.
use crate::{
    expansion::Operation,
    schedule::{Kind, Location},
    source::Event,
};
use std::collections::{BTreeMap, BTreeSet};
mod raw;
mod replay;
#[derive(Clone, Debug)]
pub struct Pair {
    pub owner: usize,
    pub carrier: Location,
    pub computation: Vec<Location>,
    pub cleanup: Vec<Location>,
    pub realisation: String,
    pub targets: Vec<String>,
    pub permutation: bool,
    pub(crate) lifted_oracle: Option<usize>,
}
fn writes(event: &Event) -> Vec<String> {
    match &event.leaf.op {
        Operation::Gate(g) => g.wires[g.own_controls()..].to_vec(),
        Operation::Release { wires, .. } => wires.clone(),
        _ => vec![],
    }
}
fn intrinsic(e: &Event) -> bool {
    matches!(&e.leaf.op,Operation::Gate(g)if g.op.starts_with("$perm/intrinsic/"))
}
fn call(e: &Event) -> bool {
    matches!(&e.leaf.op,Operation::Gate(g)if g.op.starts_with("$perm/call")||g.op.starts_with("$other/call")||g.op.starts_with("$perm/oracle/")||g.op.starts_with("$perm/qinplace"))
}
fn pair(
    owner: usize,
    events: &[Event],
    compute: &[usize],
    cleanup: &[usize],
    carrier: usize,
    permutation: &BTreeSet<usize>,
) -> Pair {
    fn locations(events: &[Event], indices: &[usize]) -> Vec<Location> {
        let mut result = Vec::new();
        for i in indices {
            let mut location = events[*i].location.clone();
            location.generated = 0;
            if !result.iter().any(|l: &Location| l.path == location.path) {
                result.push(location);
            }
        }
        result
    }
    let mut carrier = events[carrier].location.clone();
    carrier.generated = 0;
    carrier.kind = Kind::Cleanup;
    Pair {
        owner,
        carrier,
        computation: locations(events, compute),
        cleanup: locations(events, cleanup),
        realisation: "reverse".into(),
        targets: compute
            .iter()
            .chain(
                cleanup
                    .iter()
                    .filter(|i| matches!(events[**i].leaf.op, Operation::Release { .. })),
            )
            .flat_map(|i| writes(&events[*i]))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        permutation: compute.iter().all(|i| permutation.contains(i)),
        lifted_oracle: None,
    }
}
pub fn partition(
    owner: usize,
    events: &[Event],
    matching: &BTreeMap<usize, usize>,
    permutation: &BTreeSet<usize>,
) -> Vec<Pair> {
    let inverse = matching
        .iter()
        .filter(|(i, j)| i > j)
        .map(|(i, j)| (*i, *j))
        .collect::<BTreeMap<_, _>>();
    let mut owned = BTreeSet::new();
    let mut records = Vec::new();
    for (&cleanup, &compute) in &inverse {
        if intrinsic(&events[cleanup]) {
            let mut indices = vec![cleanup];
            if let Some(e) = events.get(cleanup + 1) {
                if matches!(e.leaf.op, Operation::Release { .. })
                    && e.location.path == events[cleanup].location.path
                {
                    indices.push(cleanup + 1);
                }
            }
            owned.extend(indices.iter().copied());
            records.push((
                cleanup,
                pair(owner, events, &[compute], &indices, cleanup, permutation),
            ));
        }
    }
    for (release, event) in events.iter().enumerate() {
        let Operation::Release {
            identity, wires, ..
        } = &event.leaf.op
        else {
            continue;
        };
        if owned.contains(&release)
            || identity.starts_with("source-flags/")
            || identity.starts_with("source-region/")
        {
            continue;
        }
        let mut cleanups = vec![release];
        let mut computation = Vec::new();
        for i in (0..release).rev() {
            let event = &events[i];
            if let Operation::Alloc { identity: id, .. } = &event.leaf.op {
                if id == identity {
                    break;
                }
            }
            if intrinsic(event) {
                if writes(event).iter().any(|w| wires.contains(w)) {
                    break;
                }
                continue;
            }
            if !writes(event).iter().any(|w| wires.contains(w)) {
                continue;
            }
            if let Some(compute) = inverse.get(&i) {
                if !owned.contains(&i) {
                    cleanups.push(i);
                    computation.push(*compute);
                }
            } else {
                break;
            }
        }
        cleanups.sort();
        computation.sort();
        owned.extend(cleanups.iter().copied());
        records.push((
            release,
            pair(owner, events, &computation, &cleanups, release, permutation),
        ));
    }
    for (&cleanup, &compute) in &inverse {
        if !owned.contains(&cleanup) && call(&events[cleanup]) {
            owned.insert(cleanup);
            records.push((
                cleanup,
                pair(owner, events, &[compute], &[cleanup], cleanup, permutation),
            ));
        }
    }
    records.sort_by_key(|(index, _)| *index);
    records.into_iter().map(|(_, p)| p).collect()
}

fn same_source(a: &Location, b: &Location) -> bool {
    a.path == b.path && a.root == b.root
}
fn rebased(original: &Event, carrier: &Location, op: Operation, ordinal: u64) -> Event {
    let mut event = original.clone();
    event.location = carrier.clone();
    event.location.generated = ordinal;
    event.location.kind = match &op {
        Operation::Alloc { .. } => Kind::Alloc,
        Operation::Release { .. } => Kind::Release,
        Operation::Gate(_) => Kind::Gate,
        Operation::Classical { .. } => Kind::Classical,
    };
    event.leaf.op = op;
    event.leaf.ordinal = ordinal;
    event
}

impl crate::source::Trace {
    /// Select all non-emitting carriers first, so an invalid tie cannot partially
    /// mutate the trace. Gate/alloc/start entries never participate in selection.
    pub fn realise_cleanup_entries(
        &mut self,
        entries: &[crate::schedule::Entry],
    ) -> crate::Result<()> {
        self.realise_cleanup_with_view(entries, &raw::View::default())
    }
    pub(crate) fn realise_cleanup_with_view(
        &mut self,
        entries: &[crate::schedule::Entry],
        view: &raw::View,
    ) -> crate::Result<()> {
        use crate::{
            Error, Status,
            expansion::{Leaf, inverse},
            schedule::resolve,
            target::Gate,
        };
        let choices = self
            .cleanups
            .iter()
            .map(|p| {
                Ok(resolve(entries, &view.project(&p.carrier), Kind::Cleanup)?
                    .map(|(_, e)| e.decision["realisation"].as_str().unwrap())
                    .unwrap_or("reverse")
                    .to_owned())
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let mut staged = self.clone();
        for (pair_index, choice) in choices.iter().enumerate() {
            if replay::owner_replaced(self, &staged, self.cleanups[pair_index].owner) {
                continue;
            }
            if choice == "reverse" {
                continue;
            }
            let pair = &self.cleanups[pair_index];
            if !pair.permutation {
                return Err(Error::arg("§6.4: recompute requires a permutation value"));
            }
            if self.invocations[pair.owner].raw_inverse {
                raw::replace(self, &mut staged, pair.owner, entries, view)?;
                continue;
            }
            if pair.lifted_oracle.is_some() {
                replay::replace(self, &mut staged, pair_index, entries, view)?;
                staged.cleanups[pair_index].realisation = choice.clone();
                continue;
            }
            let original = self
                .events
                .iter()
                .filter(|e| pair.computation.iter().any(|l| same_source(&e.location, l)))
                .cloned()
                .collect::<Vec<_>>();
            if self.invocations.iter().any(|i| {
                i.parent == Some(pair.owner)
                    && pair.computation.iter().any(|l| {
                        i.path.len() == l.path.len()
                            && i.path.iter().zip(&l.path).all(|(a, b)| {
                                a.symbol == b.symbol
                                    && a.point == b.point
                                    && a.iteration == b.iteration
                            })
                    })
            }) {
                replay::replace(self, &mut staged, pair_index, entries, view)?;
                staged.cleanups[pair_index].realisation = choice.clone();
                continue;
            }
            let targets = pair.targets.iter().cloned().collect::<BTreeSet<_>>();
            let mut mapping = BTreeMap::new();
            let identity = format!(
                "recompute/{pair_index}/{}",
                crate::canonical::hash(&crate::canonical::to_vec(&serde_json::json!(
                    pair.carrier
                        .path
                        .iter()
                        .map(|p| p.json())
                        .collect::<Vec<_>>()
                ))?)
            );
            let fresh = pair
                .targets
                .iter()
                .enumerate()
                .map(|(i, _)| format!("{identity}[{i}]"))
                .collect::<Vec<_>>();
            for (old, new) in pair.targets.iter().zip(&fresh) {
                mapping.insert(old.clone(), new.clone());
            }
            let first = original
                .first()
                .and_then(|e| {
                    self.events
                        .iter()
                        .position(|s| same_source(&s.location, &e.location))
                })
                .unwrap_or_else(|| {
                    self.events
                        .iter()
                        .position(|e| same_source(&e.location, &pair.carrier))
                        .unwrap()
                });
            for target in &targets {
                let allocation = self.events[..=first].iter().find(
                    |e| matches!(&e.leaf.op,Operation::Alloc{wires,..}if wires.contains(target)),
                );
                let Some(allocation) = allocation else {
                    return Err(Error::new(
                        Status::GateUnsupported,
                        "§11 K81: pre-call input history for an arbitrary interface target is postponed",
                    ));
                };
                let at = self
                    .events
                    .iter()
                    .position(|e| std::ptr::eq(e, allocation))
                    .unwrap();
                if at<first&&self.events[at+1..first].iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if g.wires[g.own_controls()..].contains(target))){return Err(Error::new(Status::GateUnsupported,"§11 K81: replay of nonzero pre-call target history is postponed"));}
            }
            let internal = original
                .iter()
                .filter_map(|e| match &e.leaf.op {
                    Operation::Alloc {
                        identity, wires, ..
                    } if !wires.iter().any(|w| targets.contains(w)) => {
                        Some((identity.clone(), wires.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (number, (id, wires)) in internal.iter().enumerate() {
                let new = format!("{identity}/scratch/{number}");
                mapping.insert(id.clone(), new.clone());
                for (i, w) in wires.iter().enumerate() {
                    mapping.insert(w.clone(), format!("{new}[{i}]"));
                }
            }
            let mut computation = Vec::new();
            for event in &original {
                let mut op = event.leaf.op.clone();
                match &mut op {
                    Operation::Alloc {
                        identity, wires, ..
                    }
                    | Operation::Release {
                        identity, wires, ..
                    } => {
                        if wires.iter().any(|w| targets.contains(w)) {
                            continue;
                        }
                        *identity = mapping.get(identity).unwrap_or(identity).clone();
                        for w in wires {
                            *w = mapping.get(w).unwrap_or(w).clone();
                        }
                    }
                    Operation::Gate(g) => {
                        for w in g.wires.iter_mut().chain(&mut g.controls) {
                            *w = mapping.get(w).unwrap_or(w).clone();
                        }
                    }
                    Operation::Classical { .. } => {
                        return Err(Error::arg("classical event in quantum cleanup"));
                    }
                }
                computation.push(Leaf {
                    ordinal: 0,
                    calls: vec![],
                    origin: vec![],
                    op,
                });
            }
            let mut controls = original.iter().filter_map(|e| {
                if let Operation::Gate(g) = &e.leaf.op {
                    Some(g.controls.clone())
                } else {
                    None
                }
            });
            let mut context = controls
                .next()
                .unwrap_or_else(|| self.invocations[pair.owner].controls.clone());
            for c in controls {
                context.retain(|w| c.contains(w));
            }
            let source = self
                .events
                .iter()
                .find(|e| same_source(&e.location, &pair.carrier))
                .unwrap();
            let mut replacement = vec![Operation::Alloc {
                identity: identity.clone(),
                wires: fresh.clone(),
                class: "any".into(),
                state: "clean".into(),
            }];
            replacement.extend(computation.iter().map(|l| l.op.clone()));
            for (u, g) in fresh.iter().zip(&pair.targets) {
                replacement.push(Operation::Gate(Gate {
                    op: "cx".into(),
                    wires: vec![u.clone(), g.clone()],
                    controls: context.clone(),
                    theta: None,
                }));
            }
            replacement.extend(inverse(&computation).into_iter().map(|l| l.op));
            replacement.push(Operation::Release {
                identity,
                wires: fresh,
                class: "any".into(),
                state: "clean".into(),
            });
            for event in self
                .events
                .iter()
                .filter(|e| pair.cleanup.iter().any(|l| same_source(&e.location, l)))
            {
                if matches!(&event.leaf.op,Operation::Release{wires,..}if wires.iter().any(|w|targets.contains(w)))
                {
                    replacement.push(event.leaf.op.clone());
                }
            }
            let mut emitted = Some(
                replacement
                    .into_iter()
                    .enumerate()
                    .map(|(i, op)| rebased(source, &pair.carrier, op, i as u64))
                    .collect::<Vec<_>>(),
            );
            let mut events = Vec::new();
            let mut boundaries = vec![0; staged.events.len() + 1];
            for (i, event) in staged.events.iter().enumerate() {
                boundaries[i] = events.len();
                if pair.cleanup.iter().any(|l| same_source(&event.location, l)) {
                    if same_source(&event.location, &pair.carrier) {
                        if let Some(replacement) = emitted.take() {
                            events.extend(replacement);
                        }
                    }
                } else {
                    events.push(event.clone());
                }
            }
            if emitted.is_some() {
                return Err(Error::arg("cleanup carrier has no source occurrence"));
            }
            boundaries[staged.events.len()] = events.len();
            for inv in &mut staged.invocations {
                inv.start = boundaries[inv.start];
                inv.end = boundaries[inv.end];
            }
            staged.events = events;
            staged.cleanups[pair_index].realisation = choice.clone();
        }
        replay::remove_replaced_invocations(self, &mut staged)?;
        replay::refresh_spans(&mut staged);
        *self = staged;
        Ok(())
    }
}
