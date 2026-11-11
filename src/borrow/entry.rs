//! K76/K78 constants and all-operand causal stacks; no Boolean simplification.
use super::*;

#[derive(Clone)]
struct Cause {
    gate: Gate,
    before: BTreeMap<String, Option<bool>>,
}
#[derive(Clone)]
pub(super) struct Allocation {
    pub event: Event,
    pub order: usize,
    pub wires: Vec<String>,
    pub borrowed: bool,
}
#[derive(Clone, Default)]
pub(super) struct State {
    pub values: BTreeMap<String, Option<bool>>,
    pub allocations: BTreeMap<String, Allocation>,
    causes: Vec<Cause>,
    stacks: BTreeMap<String, Vec<usize>>,
}
impl State {
    fn boundary(&mut self, wires: &[String]) {
        let mut connected = wires.iter().cloned().collect::<BTreeSet<_>>();
        loop {
            let mut next = connected.clone();
            for wire in &connected {
                if let Some(stack) = self.stacks.get(wire) {
                    for index in stack {
                        next.extend(self.causes[*index].before.keys().cloned());
                    }
                }
            }
            if next == connected {
                break;
            }
            connected = next;
        }
        for wire in connected {
            self.stacks.remove(&wire);
        }
    }
    fn gate(&mut self, gate: &Gate) -> Result<()> {
        let gate = crate::source::normalized(gate);
        let wires = gate.all_wires();
        let before = wires
            .iter()
            .map(|w| {
                self.values
                    .get(w)
                    .copied()
                    .map(|v| (w.clone(), v))
                    .ok_or_else(|| Error::arg(format!("borrow entry wire {w} is not live")))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        if gate
            .control_wires()
            .iter()
            .any(|w| before[w] == Some(false))
        {
            return Ok(());
        }
        let matched = wires
            .first()
            .and_then(|w| self.stacks.get(w)?.last())
            .copied()
            .filter(|i| {
                wires
                    .iter()
                    .all(|w| self.stacks.get(w).and_then(|s| s.last()) == Some(i))
                    && crate::source::gate_adjoint(&self.causes[*i].gate) == gate
            });
        if let Some(index) = matched {
            for (wire, value) in &self.causes[index].before {
                self.values.insert(wire.clone(), *value);
                self.stacks
                    .get_mut(wire)
                    .expect("matched operand stack")
                    .pop();
            }
            return Ok(());
        }
        let targets = &gate.wires[gate.own_controls()..];
        let known = before.values().all(Option::is_some);
        match gate.op.as_str() {
            "mcx" => {
                let target = targets.last().expect("validated X target");
                self.values.insert(
                    target.clone(),
                    if known {
                        before[target].map(|v| !v)
                    } else {
                        None
                    },
                );
            }
            "swap" if known => {
                self.values.insert(targets[0].clone(), before[&targets[1]]);
                self.values.insert(targets[1].clone(), before[&targets[0]]);
            }
            "s" | "sdg" | "t" | "tdg" | "rz" | "cz" => {}
            _ => {
                for target in targets {
                    self.values.insert(target.clone(), None);
                }
            }
        }
        let index = self.causes.len();
        for wire in wires {
            self.stacks.entry(wire).or_default().push(index);
        }
        self.causes.push(Cause { gate, before });
        Ok(())
    }
    fn step(&mut self, event: &Event, order: usize) -> Result<()> {
        match &event.leaf.op {
            Operation::Alloc {
                identity,
                wires,
                state,
                ..
            } => {
                self.boundary(wires);
                for wire in wires {
                    if self
                        .values
                        .insert(
                            wire.clone(),
                            if state == "clean" { Some(false) } else { None },
                        )
                        .is_some()
                    {
                        return Err(Error::arg("borrow entry allocation aliases a live wire"));
                    }
                }
                self.allocations.insert(
                    identity.clone(),
                    Allocation {
                        event: event.clone(),
                        order,
                        wires: wires.clone(),
                        borrowed: state == "borrowed",
                    },
                );
            }
            Operation::Release {
                identity, wires, ..
            } => {
                self.boundary(wires);
                self.allocations
                    .remove(identity)
                    .ok_or_else(|| Error::arg("borrow entry release has no allocation"))?;
                for wire in wires {
                    self.values
                        .remove(wire)
                        .ok_or_else(|| Error::arg("borrow entry release wire is not live"))?;
                }
            }
            Operation::Gate(g) => self.gate(g)?,
            Operation::Classical { .. } => {
                return Err(Error::new(
                    Status::GateUnsupported,
                    "§5.6: classical borrow-frame entry analysis is postponed",
                ));
            }
        }
        Ok(())
    }
}
pub(super) fn at(object: &Object, trace: &Trace, owner: usize, end: usize) -> Result<State> {
    let invocation = &trace.invocations[owner];
    if trace.invocations.iter().any(|i| {
        i.start >= invocation.start
            && i.start < end
            && i.end <= end
            && object.functions[&i.name].kind == "oracle"
    }) || trace
        .qinplaces
        .iter()
        .filter(|p| p.owner == owner && p.execution.is_some())
        .any(|p| {
            trace.events[invocation.start..end].iter().any(
                |e| matches!(&e.leaf.op,Operation::Alloc{identity,..}if identity==&p.execution.as_ref().expect("executed pair").target_id),
            )
        })
    {
        return Err(Error::new(
            Status::GateUnsupported,
            "§5.6: opaque semantic-call borrow-frame entry analysis is postponed",
        ));
    }
    let mut state = State::default();
    for wire in invocation
        .controls
        .iter()
        .chain(invocation.slots.iter().flat_map(|s| s.wires.iter()))
    {
        state.values.insert(wire.clone(), None);
    }
    for (order, event) in trace
        .events
        .iter()
        .enumerate()
        .take(end)
        .skip(invocation.start)
    {
        state.step(event, order)?;
    }
    // Only allocated locals retain entry constants; interfaces and B are full.
    for wire in invocation
        .controls
        .iter()
        .chain(invocation.slots.iter().flat_map(|s| s.wires.iter()))
    {
        state.values.insert(wire.clone(), None);
    }
    for a in state.allocations.values().filter(|a| a.borrowed) {
        for wire in &a.wires {
            state.values.insert(wire.clone(), None);
        }
    }
    Ok(state)
}
