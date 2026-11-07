//! §4.5's literal sequence, with the K4 obligation discharged before emission.
use super::*;
impl Builder<'_> {
    fn bind_qinplace_invocation(
        &self,
        g: &Function,
        args: &[String],
        actuals: &[String],
        env: &Env,
    ) -> Result<([u64; 4], Value, Vec<u64>)> {
        let Some(Binding::Value {
            isa_type, value, ..
        }) = env.get(&args[2])
        else {
            return Err(Error::arg("qinplace requires a token"));
        };
        if isa_type != &json!("token") {
            return Err(Error::arg("qinplace requires a token"));
        }
        let budget: [u64; 4] = crate::schedule::array(value)?
            .iter()
            .map(crate::schedule::uint)
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| Error::arg("invalid qinplace token"))?;
        self.object.validate_budget(budget)?;
        let mut token_ids = vec![crate::token_aliases::binding_identity(&env[&args[2]])?];
        let mut params = Vec::new();
        for (p, a) in g.groups[4].iter().zip(actuals) {
            let Some(Binding::Value {
                isa_type, value, ..
            }) = env.get(a)
            else {
                return Err(Error::arg("qinplace classical argument is not a value"));
            };
            if isa_type != &json!(p.ty) {
                return Err(Error::arg("qinplace classical type mismatch"));
            }
            params.push(value.clone());
            if p.ty == "token" {
                token_ids.push(crate::token_aliases::binding_identity(&env[a])?);
            }
        }
        let params = json!(params);
        Ok((budget, params, token_ids))
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn qinplace(
        &mut self,
        f: &Function,
        node: &Node,
        args: &[String],
        env: &Env,
        controls: &[String],
        readonly: &BTreeSet<String>,
        chain: &[Occurrence],
        iteration: &[u64],
        owner: usize,
        adjoint: bool,
    ) -> Result<()> {
        if node.lhs.is_some() || ![4, 7].contains(&args.len()) {
            return Err(Error::arg(
                "qinplace requires two oracles, token, x and three optional lists",
            ));
        }
        let g = args[0]
            .strip_prefix('@')
            .and_then(|n| self.object.functions.get(n))
            .ok_or_else(|| Error::arg("unresolved qinplace oracle"))?;
        let h = args[1]
            .strip_prefix('@')
            .and_then(|n| self.object.functions.get(n))
            .ok_or_else(|| Error::arg("unresolved qinplace inverse"))?;
        if g.kind != "oracle" || h.kind != "oracle" {
            return Err(Error::arg("qinplace requires oracle callees"));
        }
        for field in ["in", "tgt", "fault", "mem", "args"] {
            if g.signature[field] != h.signature[field] {
                return Err(Error::arg("§4.5: qinplace oracle signature mismatch"));
            }
        }
        let x = wires(&args[3], env)?;
        let n = x.len() as u64;
        if x.iter().any(|w| readonly.contains(w))
            || g.groups[0].is_empty()
            || object::wire_width(&g.groups[0][0].ty)? != n
            || object::wire_width(&g.groups[1][0].ty)? != n
        {
            return Err(Error::arg(
                "§4.5: qinplace x must be mutable and match oracle input/target widths",
            ));
        }
        let ins = if args.len() == 7 {
            object::list(&args[4])?
        } else {
            vec![]
        };
        let mem = if args.len() == 7 {
            object::list(&args[5])?
        } else {
            vec![]
        };
        let actuals = if args.len() == 7 {
            object::list(&args[6])?
        } else {
            vec![]
        };
        if ins.len() + 1 != g.groups[0].len()
            || mem.len() != g.groups[3].len()
            || actuals.len() != g.groups[4].len()
        {
            return Err(Error::arg(
                "§4.5: qinplace argument groups do not match signature",
            ));
        }
        let mut used = controls.iter().cloned().collect::<BTreeSet<_>>();
        let mut inputs = Vec::new();
        for w in &x {
            if !used.insert(w.clone()) {
                return Err(Error::arg("qinplace x overlaps controls"));
            }
        }
        for (p, a) in g.groups[0][1..].iter().zip(&ins) {
            let w = wires(a, env)?;
            if w.len() as u64 != object::wire_width(&p.ty)?
                || w.iter().any(|w| !used.insert(w.clone()))
            {
                return Err(Error::arg("qinplace input width/overlap mismatch"));
            }
            inputs.extend(w);
        }
        for (p, a) in g.groups[3].iter().zip(&mem) {
            let Some(Binding::Region {
                address,
                cell,
                role,
                origin,
                wires,
            }) = env.get(a)
            else {
                return Err(Error::arg("unbound qinplace region"));
            };
            if object::region_shape(&p.ty)? != (*address, *cell)
                || p.role.as_deref() != Some("in")
                || (!origin.starts_with("local:") && role != "in")
                || wires.iter().any(|w| !used.insert(w.clone()))
            {
                return Err(Error::arg("qinplace region shape/role/overlap mismatch"));
            }
            inputs.extend(wires.clone());
        }
        let (budget, params, token_ids) = self.bind_qinplace_invocation(g, &args, &actuals, env)?;
        let gh = self.object.closure_hash(&g.name)?.to_owned();
        let hh = self.object.closure_hash(&h.name)?.to_owned();
        let (domain, arg_tokens) = if let Some(p) = self.trace.qinplaces.iter().find(|p| {
            p.g == gh
                && p.ginv == hh
                && p.budget == budget
                && p.params == params
                && p.token_ids == token_ids
        }) {
            (p.domain.clone(), p.arg_tokens.clone())
        } else {
            crate::qinplace::verify_with_ids(
                self.object,
                &g.name,
                &h.name,
                budget,
                &params,
                &token_ids,
            )?
        };
        let key = canonical::hash(&canonical::to_vec(&json!([
            chain.iter().map(Occurrence::json).collect::<Vec<_>>(),
            node.point,
            iteration
        ]))?);
        let target_id = format!("source-qinplace/{key}/target");
        let fault_id = format!("source-qinplace/{key}/fault");
        let fault_width = object::wire_width(&g.groups[2][0].ty)?;
        let target = region_wires(&target_id, n as usize)?;
        let fault = region_wires(
            &fault_id,
            usize::try_from(fault_width)
                .map_err(|_| Error::new(Status::NoMem, "qinplace fault width exceeds host size"))?,
        )?;
        let mut local = env.clone();
        for (name, id, w) in [
            ("$qinplace.target", &target_id, &target),
            ("$qinplace.fault", &fault_id, &fault),
        ] {
            local.insert(
                name.into(),
                Binding::Allocation {
                    identity: id.clone(),
                    wires: w.clone(),
                    class: "any".into(),
                    state: "clean".into(),
                },
            );
        }
        let leaf = |ordinal, op| Leaf {
            ordinal,
            calls: vec![],
            origin: vec![],
            op,
        };
        self.emit(
            f,
            node,
            iteration,
            chain,
            owner,
            vec![
                leaf(
                    0,
                    Operation::Alloc {
                        identity: target_id.clone(),
                        wires: target.clone(),
                        class: "any".into(),
                        state: "clean".into(),
                    },
                ),
                leaf(
                    1,
                    Operation::Alloc {
                        identity: fault_id.clone(),
                        wires: fault.clone(),
                        class: "any".into(),
                        state: "clean".into(),
                    },
                ),
            ],
        )?;
        let in_list = format!(
            "({})",
            std::iter::once(args[3].clone())
                .chain(ins)
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mem_list = format!("({})", mem.join(", "));
        let arg_list = format!("({})", actuals.join(", "));
        let call_args = |name: String| {
            vec![
                name,
                args[2].clone(),
                in_list.clone(),
                "$qinplace.target".into(),
                "$qinplace.fault".into(),
                mem_list.clone(),
                arg_list.clone(),
            ]
        };
        self.oracle_call(
            f,
            node,
            &call_args(args[usize::from(adjoint)].clone()),
            &local,
            controls,
            readonly,
            chain,
            iteration,
            owner,
            0,
        )?;
        for (bit, (a, b)) in x.iter().zip(&target).enumerate() {
            self.emit(
                f,
                node,
                iteration,
                chain,
                owner,
                vec![leaf(
                    2 + bit as u64,
                    Operation::Gate(Gate {
                        op: "swap".into(),
                        wires: vec![a.clone(), b.clone()],
                        controls: controls.to_vec(),
                        theta: None,
                    }),
                )],
            )?;
        }
        self.oracle_call(
            f,
            node,
            &call_args(args[usize::from(!adjoint)].clone()),
            &local,
            controls,
            readonly,
            chain,
            iteration,
            owner,
            1,
        )?;
        self.emit(
            f,
            node,
            iteration,
            chain,
            owner,
            vec![
                leaf(
                    n + 2,
                    Operation::Release {
                        identity: target_id.clone(),
                        wires: target.clone(),
                        class: "any".into(),
                        state: "clean".into(),
                    },
                ),
                leaf(
                    n + 3,
                    Operation::Release {
                        identity: fault_id.clone(),
                        wires: fault,
                        class: "any".into(),
                        state: "clean".into(),
                    },
                ),
            ],
        )?;
        self.trace.qinplaces.push(crate::qinplace::Proof {
            owner,
            g: gh,
            ginv: hh,
            budget,
            params,
            domain,
            arg_tokens,
            token_ids,
            target_id,
            fault_id,
            target: x,
            inputs,
            controls: controls.to_vec(),
        });
        Ok(())
    }
}

pub(super) fn form_adjoint(
    events: &mut [Event],
    invocations: &mut [Invocation],
    proofs: &[crate::qinplace::Proof],
    owner: usize,
    frozen: &BTreeMap<usize, usize>,
) -> Result<()> {
    let active = proofs
        .iter()
        .filter(|p| is_descendant(p.owner, owner, invocations) && !frozen.contains_key(&p.owner))
        .collect::<Vec<_>>();
    for proof in active {
        let find = |id: &str, alloc: bool| {
            events
                .iter()
                .position(|e| match &e.leaf.op {
                    Operation::Alloc { identity, .. } if alloc => identity == id,
                    Operation::Release { identity, .. } if !alloc => identity == id,
                    _ => false,
                })
                .ok_or_else(|| Error::arg("missing qinplace private lifetime boundary"))
        };
        let at = find(&proof.target_id, true)?;
        let af = find(&proof.fault_id, true)?;
        let rt = find(&proof.target_id, false)?;
        let rf = find(&proof.fault_id, false)?;
        if at > af {
            events.swap(at, af);
        }
        if rt > rf {
            events.swap(rt, rf);
        }
        let target = events[at.min(af)].clone();
        let Operation::Alloc { wires, .. } = &target.leaf.op else {
            unreachable!()
        };
        let mut swap_indices = Vec::new();
        let mut swaps = Vec::new();
        for (a, b) in proof.target.iter().zip(wires) {
            let index=events.iter().position(|e|e.owner==proof.owner&&matches!(&e.leaf.op,Operation::Gate(g)if g.op=="swap"&&g.wires==vec![a.clone(),b.clone()])).ok_or_else(||Error::arg("missing qinplace exchange bit"))?;
            swap_indices.push(index);
            swaps.push(events[index].clone());
        }
        swap_indices.sort();
        for (index, event) in swap_indices.into_iter().zip(swaps) {
            events[index] = event;
        }
        let prefix = &target.location.path;
        let depth = prefix.len();
        let remap = |path: &mut [Occurrence]| -> Result<()> {
            if path.len() < depth {
                return Ok(());
            }
            let matches = prefix
                .iter()
                .zip(path.iter())
                .enumerate()
                .all(|(i, (a, b))| {
                    a.symbol == b.symbol
                        && a.point == b.point
                        && a.iteration == b.iteration
                        && a.instance == b.instance
                        && (i + 1 == depth || a.call_index == b.call_index)
                });
            if matches {
                if let Some(index) = path[depth - 1].call_index {
                    path[depth - 1].call_index = Some(
                        1u64.checked_sub(index)
                            .ok_or_else(|| Error::arg("invalid qinplace call occurrence"))?,
                    );
                }
            }
            Ok(())
        };
        for event in events.iter_mut() {
            remap(&mut event.location.path)?;
        }
        for invocation in invocations.iter_mut() {
            remap(&mut invocation.path)?;
        }
    }
    Ok(())
}
