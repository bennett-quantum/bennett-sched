//! §5.1's region graph includes calls hidden in zero-trip structure and lprocs.
use super::*;

fn instruction(node: &Node) -> Result<(&str, &[String])> {
    let mut op = node.op.as_str();
    let mut args = node.operands.as_slice();
    while op == "adj" || op == "ctl" {
        if op == "ctl" {
            args = args
                .get(1..)
                .ok_or_else(|| Error::arg("ctl requires a control"))?;
        }
        let (next, rest) = args
            .split_first()
            .ok_or_else(|| Error::arg("missing wrapped instruction"))?;
        op = next;
        args = rest;
    }
    Ok((op, args))
}

pub(super) fn validate(object: &Object, name: &str) -> Result<()> {
    let mut graph = BTreeMap::<String, BTreeSet<String>>::new();
    let mut roots = vec![name.to_owned()];
    while let Some(name) = roots.pop() {
        if graph.contains_key(&name) {
            continue;
        }
        let root = object
            .functions
            .get(&name)
            .ok_or_else(|| Error::arg("unlinked region root"))?;
        let mut edges = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut pending = vec![root.name.clone()];
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            let function = &object.functions[&name];
            let action = ["action", "perm_action"].contains(&function.kind.as_str());
            for node in object::flattened(&function.body) {
                let (op, args) = instruction(node)?;
                if action
                    && !GATES.contains(&op)
                    && ![
                        "qcopy",
                        "quncopy",
                        "qxor",
                        "qexch",
                        "qinplace",
                        "qload",
                        "qunload",
                        "qstore",
                        "qexchm",
                        "qpred",
                        "qunpred",
                        "qif",
                        "qloop",
                        "oracle_call",
                        "action_call",
                        "qalloc",
                        "qborrow",
                        "qfree",
                        "qreturn",
                        "qregion",
                    ]
                    .contains(&op)
                {
                    return Err(Error::arg(format!(
                        "§4/§5.3: unknown or misplaced action instruction {op}"
                    )));
                }
                let (kind, count) = match op {
                    "lift.call" => ("lproc", 1),
                    "oracle_call" if action => ("oracle", 1),
                    "action_call" if action => ("action", 1),
                    "qinplace" if action => ("oracle", 2),
                    "lift.oracle_call" | "lift.oracle_uncall" if !action => ("oracle", 1),
                    "lift.perm_call" if !action => ("perm_action", 1),
                    "oracle_call" | "action_call" | "qinplace" => {
                        return Err(Error::arg("§3.1: structured call in a lifted row"));
                    }
                    _ => continue,
                };
                for actual in args.iter().take(count) {
                    let callee = actual
                        .strip_prefix('@')
                        .and_then(|n| object.functions.get(n))
                        .ok_or_else(|| Error::arg("unresolved region/lproc call target"))?;
                    if callee.kind != kind && !(kind == "action" && callee.kind == "perm_action") {
                        return Err(Error::arg("§3.1: region/lproc call tier mismatch"));
                    }
                    if kind == "lproc" {
                        pending.push(callee.name.clone());
                    } else {
                        edges.insert(callee.name.clone());
                        roots.push(callee.name.clone());
                    }
                }
                if args.len() < count {
                    return Err(Error::arg("missing region/lproc call target"));
                }
            }
        }
        graph.insert(root.name.clone(), edges);
    }
    // Explicit DFS avoids imposing a host-stack bound on a legal deep DAG.
    let mut state = BTreeMap::<String, u8>::new();
    for root in graph.keys() {
        let mut stack = vec![(root.clone(), false)];
        while let Some((name, exit)) = stack.pop() {
            if exit {
                state.insert(name, 2);
                continue;
            }
            match state.get(&name) {
                Some(1) => return Err(Error::arg("§5.1: cyclic region call graph")),
                Some(2) => continue,
                _ => {}
            }
            state.insert(name.clone(), 1);
            stack.push((name.clone(), true));
            for next in graph[&name].iter().rev() {
                stack.push((next.clone(), false));
            }
        }
    }
    Ok(())
}
