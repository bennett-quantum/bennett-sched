//! Complete finite-domain tensor-factor replay for selected borrow frames.
use super::*;
use crate::schedule::Phase;
use num_bigint::BigUint;

// Host work allowance, not a limit on the legal frame/domain. Exhaustion is
// explicit NOMEM and produces no successful proof or narrowed evidence.
const MAX_REPLAY_LEAVES: u128 = 100_000_000;

pub(super) fn occurrence(event: &Event) -> Value {
    json!(
        event
            .location
            .path
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut value = p.json();
                let coordinate = if i == 0 {
                    event.location.root
                } else {
                    event.location.path[i - 1]
                        .entered_oracle
                        .unwrap_or_default()
                };
                if let Some(step) = coordinate.step {
                    value["step"] = json!(step);
                    value["phase"] = json!(match coordinate.phase.expect("oracle phase") {
                        Phase::Fwd => "fwd",
                        Phase::Rev => "rev",
                    });
                }
                value
            })
            .collect::<Vec<_>>()
    )
}
fn full(name: &str, width: usize) -> Value {
    let high = (BigUint::from(1u8) << width) - BigUint::from(1u8);
    let encoded = crate::canonical::unsigned_integer(&high);
    json!({"name":name,"width":width,"lo":0,"hi":encoded})
}
fn domain(object: &Object, trace: &Trace, owner: usize, state: &entry::State) -> Result<Value> {
    let inv = &trace.invocations[owner];
    let function = &object.functions[&inv.name];
    let wire_groups = if function.kind == "oracle" { 3 } else { 2 };
    let mut bundles = inv
        .controls
        .iter()
        .enumerate()
        .map(|(i, _)| full(&format!("control:{i}"), 1))
        .collect::<Vec<_>>();
    for p in function.groups[..wire_groups].iter().flatten() {
        bundles.push(full(&p.name, crate::object::wire_width(&p.ty)? as usize));
    }
    let regions = function.groups[wire_groups]
        .iter()
        .map(|p| json!({"name":p.name,"cells":"all"}))
        .collect::<Vec<_>>();
    let mut allocations = state.allocations.values().collect::<Vec<_>>();
    allocations.sort_by_key(|a| a.order);
    let mut borrowed = Vec::new();
    let mut workspace = Vec::new();
    for a in allocations {
        if a.borrowed {
            borrowed.push(full(&format!("borrowed:{}", borrowed.len()), a.wires.len()));
            continue;
        }
        if a.event.location.generated != 0 {
            return Err(Error::new(
                Status::GateUnsupported,
                "§5.6: generated workspace allocation selectors for borrow frames are postponed",
            ));
        }
        for (offset, wire) in a.wires.iter().enumerate() {
            let value = state.values[wire];
            workspace.push(json!({"allocation":occurrence(&a.event),"offset":offset,"width":1,"lo":u8::from(value==Some(true)),"hi":u8::from(value!=Some(false))}));
        }
    }
    Ok(json!({"bundles":bundles,"borrowed":borrowed,"regions":regions,"workspace":workspace}))
}
fn replay(events: &[&Event], state: &entry::State, borrowed: &BTreeSet<String>) -> Result<()> {
    let mut variables = state
        .values
        .iter()
        .filter(|(w, v)| v.is_none() && !borrowed.contains(*w))
        .map(|(w, _)| w.clone())
        .collect::<Vec<_>>();
    let nonborrowed = variables.len();
    variables.extend(borrowed.iter().cloned());
    let cases = 1usize
        .checked_shl(u32::try_from(variables.len()).unwrap_or(u32::MAX))
        .ok_or_else(|| {
            Error::new(
                Status::NoMem,
                "borrow frame complete domain exceeds host counter",
            )
        })?;
    if cases as u128 * events.len() as u128 > MAX_REPLAY_LEAVES {
        return Err(Error::new(
            Status::NoMem,
            "borrow frame complete-domain replay exceeds host work allowance",
        ));
    }
    let leaves = events.iter().map(|e| e.leaf.clone()).collect::<Vec<_>>();
    for input in 0..(1usize << nonborrowed) {
        let mut output = None;
        for dirty in 0..(1usize << borrowed.len()) {
            let tuple = input | (dirty << nonborrowed);
            let mut bits = state
                .values
                .iter()
                .map(|(w, v)| (w.clone(), v.unwrap_or(false)))
                .collect::<BTreeMap<_, _>>();
            for (bit, wire) in variables.iter().enumerate() {
                bits.insert(wire.clone(), tuple & (1usize << bit) != 0);
            }
            let initial = bits.clone();
            crate::expansion::simulate_basis(&leaves, &mut bits).map_err(|e| {
                Error::infeasible(format!("§5.6: borrow frame replay: {}", e.reason))
            })?;
            for wire in borrowed {
                if bits.remove(wire) != Some(initial[wire]) {
                    return Err(Error::infeasible(
                        "§5.6: borrow frame changes a borrowed output",
                    ));
                }
            }
            if output.as_ref().is_some_and(|previous| previous != &bits) {
                return Err(Error::infeasible(
                    "§5.6: borrow frame remaining outputs depend on borrowed inputs",
                ));
            }
            output = Some(bits);
        }
    }
    Ok(())
}
pub(super) fn prove(
    object: &Object,
    trace: &Trace,
    owner: usize,
    allocation: usize,
    first: usize,
    last: usize,
    visits: &[(usize, usize)],
) -> Result<Evidence> {
    let events = trace.events[first..=last].iter().collect::<Vec<_>>();
    if events.iter().any(|e|matches!(&e.leaf.op,Operation::Gate(g)if !["x","cx","ccx","mcx","swap"].contains(&g.op.as_str())) || matches!(&e.leaf.op,Operation::Alloc{state,..}if state=="borrowed")) {
        return Err(Error::new(Status::GateUnsupported,"§5.6: non-permutation or nested-borrow frame proof is postponed"));
    }
    let Operation::Alloc { wires, .. } = &trace.events[allocation].leaf.op else {
        unreachable!("borrow allocation")
    };
    let borrowed = wires.iter().cloned().collect::<BTreeSet<_>>();
    let current = entry::at(object, trace, owner, first)?;
    let mut products = Vec::new();
    for &(start, _) in visits {
        let state = entry::at(object, trace, owner, start)?;
        if state.values.keys().ne(current.values.keys()) {
            return Err(Error::new(
                Status::GateUnsupported,
                "§5.6: borrow frame repeated-visit lifetime correspondence is postponed",
            ));
        }
        replay(&events, &state, &borrowed)?;
        let product = domain(object, trace, owner, &state)?;
        if !products.contains(&product) {
            products.push(product);
        }
    }
    let domain = if products.len() == 1 {
        products.pop().expect("one entry product")
    } else {
        json!({"entry_products":products})
    };
    let lifetime = trace.events[trace.invocations[owner].start..allocation]
        .iter()
        .filter(|e| {
            e.owner == owner && matches!(&e.leaf.op,Operation::Alloc{state,..}if state=="borrowed")
        })
        .count();
    Ok(Evidence::Exhaustive {
        domain,
        frame: json!({"lifetime":lifetime,"first":occurrence(&trace.events[first]),"last":occurrence(&trace.events[last]),"context_key":trace.invocations[owner].context_key()}),
    })
}
