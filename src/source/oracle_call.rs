//! Semantic oracle calls reuse the same bound construction as oracle roots.
use super::*;

impl Builder<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn oracle_call(
        &mut self,
        function: &Function,
        node: &Node,
        operands: &[String],
        env: &Env,
        controls: &[String],
        readonly: &BTreeSet<String>,
        chain: &[Occurrence],
        iteration: &[u64],
        owner: usize,
        call_index: u64,
    ) -> Result<()> {
        if node.lhs.is_some() || operands.len() != 7 {
            return Err(Error::arg(
                "oracle_call requires symbol, token, inputs, target, fault, memory and arguments",
            ));
        }
        let callee = operands[0]
            .strip_prefix('@')
            .and_then(|n| self.object.functions.get(n))
            .ok_or_else(|| Error::arg("unresolved oracle callee"))?;
        if callee.kind != "oracle" {
            return Err(Error::arg("oracle_call tier mismatch"));
        }
        let Some(Binding::Value { isa_type, value }) = env.get(&operands[1]) else {
            return Err(Error::arg("oracle_call requires a token value"));
        };
        if isa_type != &json!("token") {
            return Err(Error::arg("oracle_call requires a token value"));
        }
        let tuple = value
            .as_array()
            .filter(|v| v.len() == 4)
            .ok_or_else(|| Error::arg("invalid oracle token tuple"))?
            .iter()
            .map(crate::schedule::uint)
            .collect::<Result<Vec<_>>>()?;
        let budget: [u64; 4] = tuple.try_into().expect("validated token arity");
        self.object.validate_budget(budget)?;
        let actual_inputs = object::list(&operands[2])?;
        if actual_inputs.len() != callee.groups[0].len() {
            return Err(Error::arg("oracle input slot count mismatch"));
        }
        let mut actuals = actual_inputs;
        actuals.extend([operands[3].clone(), operands[4].clone()]);
        let mut slots = Vec::new();
        for (group, role) in [(0, "in"), (1, "tgt"), (2, "fault")] {
            for (index, p) in callee.groups[group].iter().enumerate() {
                let actual = &actuals[slots.len()];
                let w = wires(actual, env)?;
                if w.len() as u64 != object::wire_width(&p.ty)? {
                    return Err(Error::arg("oracle call width mismatch"));
                }
                slots.push(Slot {
                    id: format!("{role}:{index}"),
                    role: role.into(),
                    wires: w,
                });
            }
        }
        let mut used = BTreeSet::new();
        for slot in &slots {
            for wire in &slot.wires {
                if !used.insert(wire.clone())
                    || (slot.role != "in" && (readonly.contains(wire) || controls.contains(wire)))
                {
                    return Err(Error::arg(
                        "§3.3/§5.1: oracle interfaces overlap or write a read-only/control wire",
                    ));
                }
            }
        }
        let regions = object::list(&operands[5])?;
        if regions.len() != callee.groups[3].len() {
            return Err(Error::arg("oracle call region count mismatch"));
        }
        for (index, (p, actual)) in callee.groups[3].iter().zip(regions).enumerate() {
            let Some(Binding::Region {
                address,
                cell,
                role,
                origin,
                wires,
            }) = env.get(&actual)
            else {
                return Err(Error::arg("unbound oracle region operand"));
            };
            if object::region_shape(&p.ty)? != (*address, *cell)
                || p.role.as_deref() != Some("in")
                || (!origin.starts_with("local:") && role != "in")
                || wires.iter().any(|w| !used.insert(w.clone()))
            {
                return Err(Error::arg(
                    "§3.3/K26: oracle region shape, role or disjointness mismatch",
                ));
            }
            slots.push(Slot {
                id: format!("mem:{index}"),
                role: "mem_in".into(),
                wires: wires.clone(),
            });
        }
        let args = object::list(&operands[6])?;
        if args.len() != callee.groups[4].len() {
            return Err(Error::arg("oracle argument count mismatch"));
        }
        let mut values = Vec::new();
        for (p, actual) in callee.groups[4].iter().zip(args) {
            let Some(Binding::Value { isa_type, value }) = env.get(&actual) else {
                return Err(Error::arg("unbound oracle classical argument"));
            };
            if isa_type != &json!(p.ty) {
                return Err(Error::arg("oracle argument type mismatch"));
            }
            values.push(value.clone());
        }
        let params = json!(values);
        self.object.validate_params(&callee.name, &params)?;
        let inherited = u32::try_from(controls.len())
            .map_err(|_| Error::new(Status::NoMem, "control count exceeds host size"))?;
        let child = crate::oracle::build(self.object, &callee.name, inherited, &params, budget)?;
        if !child.borrow_proofs.is_empty() || !child.cleanups.is_empty() {
            return Err(Error::new(
                Status::InvalidState,
                "oracle construction returned source proof/cleanup metadata requiring call rebinding",
            ));
        }
        if child.slots.len() != slots.len() || child.inherited.len() != controls.len() {
            return Err(Error::new(
                Status::InvalidState,
                "oracle construction interface count disagrees with call binding",
            ));
        }
        let mut path = chain.to_vec();
        path.push(Occurrence {
            symbol: self.object.symbols[&function.name].interface_hash.clone(),
            point: node.point.clone(),
            iteration: iteration.to_vec(),
            instance: vec![],
            call_index: Some(call_index),
            entered_oracle: Some(Coordinate::default()),
        });
        let prefix = format!(
            "oracle-call/{}/",
            canonical::hash(&canonical::to_vec(&json!(
                path.iter().map(Occurrence::json).collect::<Vec<_>>()
            ))?)
        );
        let mut names = BTreeMap::new();
        for (old, new) in child.inherited.iter().zip(controls) {
            names.insert(old.clone(), new.clone());
        }
        for (old, new) in child.slots.iter().zip(&slots) {
            if old.id != new.id || old.wires.len() != new.wires.len() {
                return Err(Error::arg(
                    "oracle source interface disagrees with call binding",
                ));
            }
            for (old, new) in old.wires.iter().zip(&new.wires) {
                names.insert(old.clone(), new.clone());
            }
        }
        let wire = |w: &str| {
            names
                .get(w)
                .cloned()
                .unwrap_or_else(|| format!("{prefix}{w}"))
        };
        let offset = self.trace.events.len();
        let base = self.trace.invocations.len();
        for mut invocation in child.invocations {
            invocation.controls = invocation.controls.iter().map(|w| wire(w)).collect();
            for slot in &mut invocation.slots {
                slot.wires = slot.wires.iter().map(|w| wire(w)).collect();
            }
            invocation.start += offset;
            invocation.end += offset;
            invocation.parent = Some(invocation.parent.map_or(owner, |p| base + p));
            let mut full = path.clone();
            full.extend(invocation.path);
            invocation.path = full;
            self.trace.invocations.push(invocation);
        }
        for mut event in child.events {
            match &mut event.leaf.op {
                Operation::Gate(g) => {
                    for w in g.wires.iter_mut().chain(&mut g.controls) {
                        *w = wire(w);
                    }
                }
                Operation::Alloc {
                    identity, wires, ..
                }
                | Operation::Release {
                    identity, wires, ..
                } => {
                    *identity = format!("{prefix}{identity}");
                    for w in wires {
                        *w = wire(w);
                    }
                }
                Operation::Classical { .. } => {
                    return Err(Error::arg("classical event in oracle construction"));
                }
            }
            let mut full = path.clone();
            full.last_mut().unwrap().entered_oracle = Some(event.location.root);
            full.extend(event.location.path);
            event.location.path = full;
            event.location.root = Coordinate::default();
            event.owner += base;
            self.trace.events.push(event);
        }
        Ok(())
    }
}
