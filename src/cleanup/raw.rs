//! A raw inverse reverses an already selected forward construction. Selection
//! views keep actual schedule coordinates while a captured subtree is normalised.
use crate::{
    Error, Result, Status,
    expansion::inverse,
    schedule::{Location, Phase},
    source::{Trace, is_descendant},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
pub(crate) struct View {
    inversions: Vec<usize>,
}
fn flip(location: &mut Location, floor: usize) {
    for point in location.path.iter_mut().skip(floor) {
        if let Some(coordinate) = &mut point.entered_oracle {
            coordinate.phase = coordinate.phase.map(|p| match p {
                Phase::Fwd => Phase::Rev,
                Phase::Rev => Phase::Fwd,
            });
        }
    }
}
impl View {
    pub(super) fn project(&self, location: &Location) -> Location {
        let mut location = location.clone();
        for floor in &self.inversions {
            flip(&mut location, *floor);
        }
        location
    }
}
fn extract(source: &Trace, root: usize) -> Trace {
    let invocation = &source.invocations[root];
    let start = invocation.start;
    let end = invocation.end;
    let owners = (0..source.invocations.len())
        .filter(|i| is_descendant(*i, root, &source.invocations))
        .enumerate()
        .map(|(new, old)| (old, new))
        .collect::<BTreeMap<_, _>>();
    let mut local = Trace {
        object: source.object.clone(),
        events: source.events[start..end].to_vec(),
        invocations: source
            .invocations
            .iter()
            .enumerate()
            .filter(|(i, _)| owners.contains_key(i))
            .map(|(_, inv)| inv.clone())
            .collect(),
        slots: invocation.slots.clone(),
        inherited: invocation.controls.clone(),
        borrow_proofs: source
            .borrow_proofs
            .iter()
            .filter(|p| owners.contains_key(&p.owner))
            .cloned()
            .collect(),
        cleanups: source
            .cleanups
            .iter()
            .filter(|p| owners.contains_key(&p.owner))
            .cloned()
            .collect(),
        qinplaces: source
            .qinplaces
            .iter()
            .filter(|p| owners.contains_key(&p.owner))
            .cloned()
            .collect(),
        replaced_invocations: BTreeSet::new(),
    };
    for event in &mut local.events {
        event.owner = owners[&event.owner];
    }
    for (i, inv) in local.invocations.iter_mut().enumerate() {
        inv.start -= start;
        inv.end -= start;
        inv.parent = if i == 0 {
            None
        } else {
            inv.parent.map(|p| owners[&p])
        };
    }
    for p in &mut local.cleanups {
        p.owner = owners[&p.owner];
        p.lifted_oracle = p.lifted_oracle.map(|i| owners[&i]);
    }
    for p in &mut local.borrow_proofs {
        p.owner = owners[&p.owner];
    }
    for p in &mut local.qinplaces {
        p.owner = owners[&p.owner];
    }
    local
}
fn invert_scope(trace: &mut Trace, floor: usize) {
    let length = trace.events.len();
    trace.events.reverse();
    for event in &mut trace.events {
        event.leaf = inverse(std::slice::from_ref(&event.leaf)).pop().unwrap();
        flip(&mut event.location, floor);
    }
    for inv in &mut trace.invocations {
        (inv.start, inv.end) = (length - inv.end, length - inv.start);
        inv.raw_inverse = !inv.raw_inverse;
        let mut location = Location {
            root: inv.containing,
            path: std::mem::take(&mut inv.path),
            generated: 0,
            kind: crate::schedule::Kind::Gate,
        };
        flip(&mut location, floor);
        inv.path = location.path;
    }
    for pair in &mut trace.cleanups {
        flip(&mut pair.carrier, floor);
        for location in pair.computation.iter_mut().chain(&mut pair.cleanup) {
            flip(location, floor);
        }
    }
    for proof in &mut trace.borrow_proofs {
        if let crate::borrow::Evidence::Translation { adjoint, .. } = &mut proof.evidence {
            *adjoint = !*adjoint;
        }
    }
}
pub(super) fn replace(
    source: &Trace,
    staged: &mut Trace,
    owner: usize,
    entries: &[crate::schedule::Entry],
    view: &View,
) -> Result<()> {
    let mut root = owner;
    while let Some(parent) = source.invocations[root].parent {
        if !source.invocations[parent].raw_inverse {
            break;
        }
        root = parent;
    }
    let floor = source.invocations[root]
        .path
        .len()
        .checked_sub(1)
        .expect("raw source call has an invocation edge");
    let mut local = extract(source, root);
    invert_scope(&mut local, floor);
    let mut nested_view = view.clone();
    nested_view.inversions.push(floor);
    local.realise_cleanup_with_view(entries, &nested_view)?;
    invert_scope(&mut local, floor);

    super::replay::refresh_spans(staged);
    let positions = staged
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| is_descendant(event.owner, root, &staged.invocations))
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let (start, end) = match (positions.first(), positions.last()) {
        (Some(first), Some(last)) => (*first, last + 1),
        _ => (staged.invocations[root].start, staged.invocations[root].end),
    };
    assert_eq!(
        end - start,
        positions.len(),
        "invocation events remain contiguous"
    );
    let replacement_len = local.events.len();
    staged
        .events
        .try_reserve(replacement_len.saturating_sub(end - start))
        .map_err(|_| Error::new(Status::NoMem, "raw cleanup replacement exceeds host memory"))?;
    let ancestors = (0..staged.invocations.len())
        .map(|i| is_descendant(root, i, &staged.invocations))
        .collect::<Vec<_>>();
    let descendants = (0..staged.invocations.len())
        .map(|i| is_descendant(i, root, &staged.invocations))
        .collect::<Vec<_>>();
    let shift = |position: usize| position - end + start + replacement_len;
    for (i, inv) in staged.invocations.iter_mut().enumerate() {
        if ancestors[i] {
            inv.end = shift(inv.end);
        } else if descendants[i] {
            // Keep retired records representable until the final owner remap.
            inv.start = start;
            inv.end = start;
        } else if inv.start >= end && !(start == end && inv.start == end && i < root) {
            inv.start = shift(inv.start);
            inv.end = shift(inv.end);
        }
    }
    let parent = staged.invocations[root].parent;
    let base = staged.invocations.len();
    for mut inv in local.invocations {
        inv.start += start;
        inv.end += start;
        inv.parent = inv.parent.map(|p| base + p).or(parent);
        staged.invocations.push(inv);
    }
    for event in &mut local.events {
        event.owner += base;
    }
    staged.events.splice(start..end, local.events);
    for mut p in local.cleanups {
        p.owner += base;
        p.lifted_oracle = p.lifted_oracle.map(|i| base + i);
        staged.cleanups.push(p);
    }
    for mut p in local.borrow_proofs {
        p.owner += base;
        staged.borrow_proofs.push(p);
    }
    for mut p in local.qinplaces {
        p.owner += base;
        staged.qinplaces.push(p);
    }
    staged.replaced_invocations.insert(root);
    Ok(())
}
