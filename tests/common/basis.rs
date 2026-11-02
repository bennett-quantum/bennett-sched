//! Dense execution of an already emitted permutation trace, for exhaustive tests.
//! Wire interning happens once; the ordinary map interpreter remains an oracle.
use bennett_sched::{
    Error, Result,
    expansion::{Leaf, Operation},
};
use std::collections::BTreeMap;
enum Step {
    Acquire(Vec<usize>),
    Release(Vec<usize>),
    X(Vec<usize>, usize),
    Swap(Vec<usize>, usize, usize),
}
pub struct Program {
    ids: BTreeMap<String, usize>,
    steps: Vec<Step>,
}
impl Program {
    pub fn compile(leaves: &[Leaf]) -> Result<Self> {
        let mut ids = BTreeMap::new();
        let mut steps = Vec::new();
        for leaf in leaves {
            let mut id = |wire: &str| {
                let n = ids.len();
                *ids.entry(wire.to_owned()).or_insert(n)
            };
            let step = match &leaf.op {
                Operation::Alloc { wires, state, .. } => {
                    if state != "clean" {
                        return Err(Error::arg("dense witness requires clean allocations"));
                    }
                    Step::Acquire(wires.iter().map(|w| id(w)).collect())
                }
                Operation::Release { wires, state, .. } => {
                    if state != "clean" {
                        return Err(Error::arg("dense witness requires clean releases"));
                    }
                    Step::Release(wires.iter().map(|w| id(w)).collect())
                }
                Operation::Gate(g) => {
                    let controls = g.control_wires().iter().map(|w| id(w)).collect();
                    match g.op.as_str() {
                        "x" | "cx" | "ccx" | "mcx" => {
                            Step::X(controls, id(g.wires.last().unwrap()))
                        }
                        "swap" => Step::Swap(controls, id(&g.wires[0]), id(&g.wires[1])),
                        _ => return Err(Error::arg("dense witness requires permutation gates")),
                    }
                }
                _ => {
                    return Err(Error::arg(
                        "dense witness does not execute classical history",
                    ));
                }
            };
            steps.push(step);
        }
        Ok(Self { ids, steps })
    }
    pub fn run(&self, bits: &mut BTreeMap<String, bool>) -> Result<()> {
        let mut values = vec![false; self.ids.len()];
        let mut active = vec![false; self.ids.len()];
        for (name, value) in bits.iter() {
            if let Some(id) = self.ids.get(name) {
                values[*id] = *value;
                active[*id] = true;
            }
        }
        for step in &self.steps {
            match step {
                Step::Acquire(ids) => {
                    for id in ids {
                        if active[*id] {
                            return Err(Error::arg("duplicate dense wire"));
                        }
                        active[*id] = true;
                        values[*id] = false;
                    }
                }
                Step::Release(ids) => {
                    for id in ids {
                        if !active[*id] || values[*id] {
                            return Err(Error::arg("unclean/missing dense release"));
                        }
                        active[*id] = false;
                    }
                }
                Step::X(controls, _) | Step::Swap(controls, _, _) => {
                    let mut enabled = true;
                    for id in controls {
                        if !active[*id] {
                            return Err(Error::arg("missing dense control"));
                        }
                        enabled &= values[*id];
                    }
                    if !enabled {
                        continue;
                    }
                    match step {
                        Step::X(_, target) => {
                            if !active[*target] {
                                return Err(Error::arg("missing dense target"));
                            }
                            values[*target] ^= true;
                        }
                        Step::Swap(_, a, b) => {
                            if !active[*a] || !active[*b] {
                                return Err(Error::arg("missing dense swap wire"));
                            }
                            values.swap(*a, *b);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        for (name, id) in &self.ids {
            if active[*id] {
                bits.insert(name.clone(), values[*id]);
            } else {
                bits.remove(name);
            }
        }
        Ok(())
    }
}
#[test]
fn dense_witness_matches_map_interpreter_with_controls_clean_lifetimes_and_reacquisition() {
    use bennett_sched::expansion::{Binding, Env, Library, simulate_basis};
    use serde_json::json;
    let env = Env::from([
        ("a".into(), Binding::Wires(vec!["a".into()])),
        ("b".into(), Binding::Wires(vec!["b".into()])),
    ]);
    let body = json!([{"for":"i","from":0,"to":2,"body":[{"alloc":"q","width":1,"class":"any","state":"clean"},{"gate":"cx","wires":["a","q"]},{"gate":"cx","wires":["q","b"]},{"gate":"cx","wires":["a","q"]},{"release":"q"}]},{"gate":"swap","wires":["a","b"]}]);
    let leaves = Library::published()
        .unwrap()
        .body(body.as_array().unwrap(), &env, &["c".into()])
        .unwrap();
    let program = Program::compile(&leaves).unwrap();
    for tuple in 0..8 {
        let mut slow = BTreeMap::from([
            ("a".into(), tuple & 1 != 0),
            ("b".into(), tuple & 2 != 0),
            ("c".into(), tuple & 4 != 0),
            ("untouched".into(), true),
        ]);
        let mut fast = slow.clone();
        simulate_basis(&leaves, &mut slow).unwrap();
        program.run(&mut fast).unwrap();
        assert_eq!(fast, slow);
    }
}
