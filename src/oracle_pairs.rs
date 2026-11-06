//! §5.5 source CFG matching. Version identities are canonical up to renaming;
//! a write cannot regain an old identity, while a matched uncall restores it.
use crate::{
    Error, Result, Status, canonical,
    expansion::{Binding, Env},
    object::{self, Node},
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone)]
struct Call {
    key: usize,
    inputs: Vec<usize>,
    targets: Vec<usize>,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Frame {
    site: usize,
    previous: Vec<u32>,
    produced: Vec<u32>,
    dirty: bool,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    versions: Vec<u32>,
    stacks: Vec<Vec<Frame>>,
}
impl State {
    fn canonicalize(&mut self) {
        let mut names = BTreeMap::new();
        let rename = |v: &mut u32, names: &mut BTreeMap<u32, u32>| {
            let next = names.len() as u32;
            *v = *names.entry(*v).or_insert(next);
        };
        for v in &mut self.versions {
            rename(v, &mut names);
        }
        for stack in &mut self.stacks {
            for frame in stack {
                for v in frame.previous.iter_mut().chain(&mut frame.produced) {
                    rename(v, &mut names);
                }
            }
        }
    }
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.versions.len() * 4
            + self.stacks.len() * std::mem::size_of::<Vec<Frame>>()
            + self
                .stacks
                .iter()
                .flatten()
                .map(|f| std::mem::size_of::<Frame>() + (f.previous.len() + f.produced.len()) * 4)
                .sum::<usize>()
    }
    fn mark_inputs(&mut self, writes: &[usize], calls: &[Option<Call>]) {
        for frame in self.stacks.iter_mut().flatten() {
            if calls[frame.site]
                .as_ref()
                .unwrap()
                .inputs
                .iter()
                .any(|w| writes.contains(w))
            {
                frame.dirty = true;
            }
        }
    }
    fn write(&mut self, writes: &[usize]) {
        let mut next = self
            .versions
            .iter()
            .chain(
                self.stacks
                    .iter()
                    .flatten()
                    .flat_map(|f| f.previous.iter().chain(&f.produced)),
            )
            .copied()
            .max()
            .unwrap_or(0)
            + 1;
        for w in writes {
            self.versions[*w] = next;
            next += 1;
        }
    }
}
fn fail(reason: &str) -> Error {
    Error::arg(format!("§5.5: {reason}"))
}
fn wire_names(binding: &Binding) -> Option<&[String]> {
    match binding {
        Binding::Wires(w) | Binding::Allocation { wires: w, .. } => Some(w),
        _ => None,
    }
}
fn actual(name: &str, env: &Env, ids: &BTreeMap<String, usize>) -> Result<Vec<usize>> {
    let wires = crate::oracle::read(name, env)?;
    Ok(wires.iter().map(|w| ids[w]).collect())
}
fn list_wires(operand: &str, env: &Env, ids: &BTreeMap<String, usize>) -> Result<Vec<usize>> {
    object::list(operand)?
        .into_iter()
        .map(|n| actual(&n, env, ids))
        .collect::<Result<Vec<_>>>()
        .map(|v| v.into_iter().flatten().collect())
}
fn key(
    node: &Node,
    env: &Env,
    ids: &BTreeMap<String, usize>,
    tokens: &BTreeMap<String, u64>,
) -> Result<(Vec<u8>, Vec<usize>, Vec<usize>)> {
    let args = &node.operands;
    if node.lhs.is_some() || args.len() != 7 {
        return Err(fail("invalid oracle call operand count"));
    }
    let inputs = list_wires(&args[2], env, ids)?;
    let mut targets = actual(&args[3], env, ids)?;
    targets.extend(actual(&args[4], env, ids)?);
    let token = tokens
        .get(&args[1])
        .ok_or_else(|| fail("oracle call token must be a fixed token argument"))?;
    let Some(Binding::Value {
        value: tuple,
        isa_type,
        ..
    }) = env.get(&args[1])
    else {
        return Err(fail("invalid oracle token"));
    };
    if isa_type != "token" {
        return Err(fail("invalid oracle token type"));
    }
    let memory = object::list(&args[5])?
        .iter()
        .map(|name| {
            let Some(Binding::Region {
                address,
                cell,
                role,
                origin,
                wires,
            }) = env.get(name)
            else {
                return Err(fail("invalid oracle memory argument"));
            };
            Ok(json!([address, cell, role, origin, wires]))
        })
        .collect::<Result<Vec<_>>>()?;
    let values = object::list(&args[6])?
        .iter()
        .map(|name| {
            let Some(Binding::Value {
                isa_type, value, ..
            }) = env.get(name)
            else {
                return Err(fail("oracle args must be fixed classical arguments"));
            };
            if isa_type == "token" {
                Ok(json!([
                    isa_type,
                    value,
                    tokens
                        .get(name)
                        .ok_or_else(|| fail("unbound token argument identity"))?
                ]))
            } else {
                Ok(json!([isa_type, value]))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((
        canonical::to_vec(&json!([args[0], token, tuple, inputs, memory, values]))?,
        inputs,
        targets,
    ))
}
fn successors(
    node: &Node,
    index: usize,
    n: usize,
    first: u64,
    labels: &BTreeMap<String, u64>,
) -> Result<Vec<usize>> {
    let label = |text: &str| -> Result<usize> {
        let value = text
            .strip_prefix('^')
            .and_then(|s| labels.get(s))
            .and_then(|v| v.checked_sub(first))
            .ok_or_else(|| fail("invalid source CFG label"))? as usize;
        if value >= n {
            return Err(fail("source CFG label outside its function"));
        }
        Ok(value)
    };
    Ok(match node.op.as_str() {
        "qjump" => vec![label(
            node.operands
                .first()
                .ok_or_else(|| fail("missing jump label"))?,
        )?],
        "qbranch" => {
            if node.operands.len() != 3 {
                return Err(fail("invalid branch operands"));
            }
            vec![label(&node.operands[1])?, label(&node.operands[2])?]
        }
        "qjumpr" => (0..n).collect(),
        "lift.retval" | "lift.ret" | "fault" => vec![],
        _ => {
            if index + 1 < n {
                vec![index + 1]
            } else {
                vec![]
            }
        }
    })
}
/// Check one lexical activation at a time. lift.call snapshots its arguments
/// and restores the caller view on return; only its result writes that view.
pub(crate) fn validate(
    nodes: &[Node],
    env: &Env,
    first: u64,
    labels: &BTreeMap<String, u64>,
    tokens: &BTreeMap<String, u64>,
) -> Result<BTreeMap<usize, usize>> {
    if !nodes.iter().any(|n| n.op == "lift.oracle_uncall") {
        return Ok(BTreeMap::new());
    }
    let wires = env
        .iter()
        .filter_map(|(_, b)| wire_names(b))
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    let ids = wires
        .into_iter()
        .enumerate()
        .map(|(i, w)| (w, i))
        .collect::<BTreeMap<_, _>>();
    let mut keys = BTreeMap::new();
    let mut calls = Vec::new();
    let mut writes = Vec::new();
    let mut edges = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let call = if ["lift.oracle_call", "lift.oracle_uncall"].contains(&node.op.as_str()) {
            let (key, inputs, targets) = key(node, env, &ids, tokens)?;
            let next = keys.len();
            let key = *keys.entry(key).or_insert(next);
            Some(Call {
                key,
                inputs,
                targets,
            })
        } else {
            None
        };
        let mut changed = Vec::new();
        if let Some(lhs) = &node.lhs {
            if lhs.starts_with('(') {
                changed.extend(list_wires(lhs, env, &ids)?);
            } else {
                changed.extend(actual(lhs, env, &ids)?);
            }
        }
        if node.op == "lift.perm_call" {
            let mutable = node
                .operands
                .get(1)
                .ok_or_else(|| fail("invalid permutation call"))?;
            changed.extend(list_wires(mutable, env, &ids)?);
        }
        writes.push(changed);
        calls.push(call);
        edges.push(successors(node, index, nodes.len(), first, labels)?);
    }
    // Only joins (and the entry) need retained states. A cycle reachable
    // from entry must revisit one of these checkpoints; linear paths can be
    // consumed without retaining a separate copy for every source row.
    let mut incoming = vec![0usize; nodes.len()];
    for next in edges.iter().flatten() {
        incoming[*next] += 1;
    }
    let mut matches = BTreeMap::new();
    let initial = State {
        versions: (0..ids.len() as u32).collect(),
        stacks: vec![vec![]; keys.len()],
    };
    let mut visited = BTreeSet::from([(0, initial.clone())]);
    let mut pending = VecDeque::from([(0, initial)]);
    const HOST_PROOF_BYTES: usize = 32 * 1024 * 1024;
    const HOST_PROOF_STATES: usize = 8192;
    let mut retained = pending[0].1.bytes();
    let mut queued = retained;
    while let Some((index, mut state)) = pending.pop_front() {
        queued -= state.bytes();
        let node = &nodes[index];
        if let Some(call) = &calls[index] {
            if node.op == "lift.oracle_uncall" {
                let frame = state.stacks[call.key]
                    .pop()
                    .ok_or_else(|| fail("unmatched oracle uncall on a source path"))?;
                if let Some(previous) = matches.insert(index, frame.site) {
                    if previous != frame.site {
                        return Err(fail(
                            "uncall matches different computing calls on different paths",
                        ));
                    }
                }
                if calls[frame.site].as_ref().unwrap().targets != call.targets {
                    return Err(fail("uncall targets do not belong to the most recent call"));
                }
                if frame.dirty {
                    return Err(fail(
                        "an intervening source row writes an oracle input location",
                    ));
                }
                if call
                    .targets
                    .iter()
                    .map(|w| state.versions[*w])
                    .collect::<Vec<_>>()
                    != frame.produced
                {
                    return Err(fail(
                        "uncall produced target versions are no longer current",
                    ));
                }
                state.mark_inputs(&call.targets, &calls);
                for (w, v) in call.targets.iter().zip(frame.previous) {
                    state.versions[*w] = v;
                }
            } else {
                state.mark_inputs(&call.targets, &calls);
                let previous = call.targets.iter().map(|w| state.versions[*w]).collect();
                state.write(&call.targets);
                let produced = call.targets.iter().map(|w| state.versions[*w]).collect();
                state.stacks[call.key].push(Frame {
                    site: index,
                    previous,
                    produced,
                    dirty: false,
                });
            }
        } else {
            state.mark_inputs(&writes[index], &calls);
            state.write(&writes[index]);
        }
        state.canonicalize();
        for next in &edges[index] {
            let checkpoint = *next == 0 || incoming[*next] > 1;
            if checkpoint && visited.contains(&(*next, state.clone())) {
                continue;
            }
            let bytes = state.bytes();
            queued = queued.saturating_add(bytes);
            if checkpoint {
                retained = retained.saturating_add(bytes);
            }
            if retained.saturating_add(queued) > HOST_PROOF_BYTES
                || (checkpoint && visited.len() >= HOST_PROOF_STATES)
            {
                return Err(Error::new(
                    Status::NoMem,
                    "§5.5 source cleanup analysis exhausts host state workspace",
                ));
            }
            if checkpoint {
                visited.insert((*next, state.clone()));
            }
            pending.push_back((*next, state.clone()));
        }
    }
    Ok(matches)
}
