#[path = "support/pins.rs"]
mod pins;
// The published bodies use 0.8 envelopes. Rehash them for 0.9 without
// changing instructions, then compare all declared trace fields and tuples.
use crate::common;
use bennett_sched::{
    expansion::{Operation, inverse, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn rebind(source: &str) -> Object {
    let symbols: Value = serde_json::from_str(
        source
            .split("section symbols\n")
            .nth(1)
            .unwrap()
            .split('\n')
            .next()
            .unwrap(),
    )
    .unwrap();
    let functions = source.split("section functions\n").nth(1).unwrap();
    let mut pieces = Vec::new();
    let mut current = String::new();
    for line in functions.lines() {
        if line.starts_with("perm_action @") && !current.is_empty() {
            pieces.push(current);
            current = String::new();
        }
        current.push_str(line);
        current.push('\n');
    }
    pieces.push(current);
    let mut bytes: Option<Vec<u8>> = None;
    for function in pieces.iter().rev() {
        let (header, rest) = function.split_once('\n').unwrap();
        let name = header.split('@').nth(1).unwrap().split(' ').next().unwrap();
        let mut sig = symbols
            .as_object()
            .unwrap()
            .values()
            .find(|s| s["name"] == name)
            .unwrap()["signature"]
            .clone();
        sig["abi"] = json!("0.9");
        let body = rest
            .split_once("^entry():\n")
            .unwrap()
            .1
            .trim_end_matches('\n');
        bytes = Some(match bytes {
            None => common::action("perm_action", name, sig, header, body),
            Some(base) => common::caller(&base, "perm_action", name, sig, header, body),
        });
    }
    Object::parse(&bytes.unwrap()).unwrap()
}

#[test]
fn all_published_k80_trace_fields_and_complete_basis_tuples_match_after_version_rebinding() {
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(pins::spec(
            "conformance/fixtures/accepted-k78-k80-09/k80.json",
        ))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["vectors"].as_array().unwrap().len(), 5);
    for vector in manifest["vectors"].as_array().unwrap() {
        let object = rebind(vector["source"].as_str().unwrap());
        let mut trace = Trace::build(&object, "multi", 1, &json!([]), [0; 4]).unwrap();
        assert_eq!(trace.cleanups.iter().filter(|p| p.owner == 0).count(), 1);
        if vector["realisation"] == "recompute" {
            let carrier = &trace
                .cleanups
                .iter()
                .find(|p| p.owner == 0)
                .unwrap()
                .carrier;
            let entry=bennett_sched::schedule::parse_entry(&json!({"path":carrier.path.iter().map(|p|p.json()).collect::<Vec<_>>(),"step":"*","phase":"*","generated":0,"kind":"cleanup","cleanup":{"realisation":"recompute"}})).unwrap();
            trace.realise_cleanup_entries(&[entry]).unwrap();
        }
        let expected = vector["leaves"].as_array().unwrap();
        assert_eq!(trace.events.len(), expected.len(), "{vector}");
        let mut names = BTreeMap::from([
            ("mut:0[0]".to_owned(), json!(["t", 0])),
            ("in:0[0]".to_owned(), json!(["x", 0])),
            ("in:1[0]".to_owned(), json!(["z", 0])),
            ("control:0".to_owned(), json!(["p", 0])),
        ]);
        let mut allocations = BTreeMap::new();
        for (event, row) in trace.events.iter().zip(expected) {
            let (gate, wires, controls) = match &event.leaf.op {
                Operation::Alloc {
                    identity,
                    wires,
                    class,
                    state,
                } => {
                    let tuple: Value = serde_json::from_str(&format!(
                        "[{}]",
                        row["allocation"]
                            .as_str()
                            .unwrap()
                            .strip_prefix("Some((")
                            .unwrap()
                            .strip_suffix("))")
                            .unwrap()
                    ))
                    .unwrap();
                    assert_eq!(tuple[1], wires.len());
                    assert_eq!(tuple[2], class.as_str());
                    assert_eq!(tuple[3], state.as_str());
                    allocations.insert(identity.clone(), tuple.clone());
                    for (i, w) in wires.iter().enumerate() {
                        names.insert(w.clone(), json!([tuple[0], i]));
                    }
                    ("alloc", vec![], vec![])
                }
                Operation::Release { identity, .. } => {
                    let tuple = &allocations[identity];
                    assert_eq!(
                        row["allocation"].as_str().unwrap(),
                        format!(
                            "Some(({}, {}, {}, {}))",
                            tuple[0], tuple[1], tuple[2], tuple[3]
                        )
                    );
                    ("release", vec![], vec![])
                }
                Operation::Gate(g) => {
                    assert_eq!(row["allocation"], "None");
                    (
                        g.op.as_str(),
                        g.wires.iter().map(|w| names[w].clone()).collect(),
                        g.controls.iter().map(|w| names[w].clone()).collect(),
                    )
                }
                Operation::Classical { .. } => panic!("classical K80 leaf"),
            };
            assert_eq!(row["gate"], gate);
            assert_eq!(row["wires"], json!(wires));
            assert_eq!(row["controls"], json!(controls));
            let caller = event.location.path.first().unwrap();
            let callee =
                (event.location.path.len() > 1).then(|| event.location.path.last().unwrap());
            assert_eq!(row["caller_point"], caller.point);
            assert_eq!(row["caller_iteration"], json!(caller.iteration));
            assert_eq!(
                row["callee_point"],
                callee.map_or(Value::Null, |p| json!(p.point))
            );
            assert_eq!(
                row["callee_iteration"],
                callee.map_or(json!([]), |p| json!(p.iteration))
            );
            assert_eq!(row["generated"], event.location.generated);
            assert_eq!(row["call_index"], json!(caller.call_index));
            assert_eq!(
                row["instance"],
                json!(event.location.path.last().unwrap().instance)
            );
            assert_eq!(row["step"], Value::Null);
            assert_eq!(row["phase"], Value::Null);
            assert_eq!(row["scratch"], false);
            assert_eq!(event.location.root, Default::default());
        }
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let order = ["in:0[0]", "in:1[0]", "control:0", "mut:0[0]"];
        assert_eq!(vector["tuple_order"], json!(["x", "z", "control", "t"]));
        for tuple in vector["tuples"].as_array().unwrap() {
            let before = tuple["before"].as_u64().unwrap();
            let mut state = order
                .iter()
                .enumerate()
                .map(|(i, w)| (w.to_string(), before & (1 << i) != 0))
                .collect::<BTreeMap<_, _>>();
            simulate_basis(&leaves, &mut state).unwrap();
            let packed = order
                .iter()
                .enumerate()
                .map(|(i, w)| u64::from(state[*w]) << i)
                .sum::<u64>();
            assert_eq!(tuple["after"], packed);
            assert_eq!(tuple["live_wires"], state.len());
            assert_eq!(tuple["phase"], json!([1, 0]));
            assert_eq!(tuple["scratch"], json!([]));
            simulate_basis(&inverse(&leaves), &mut state).unwrap();
            let packed = order
                .iter()
                .enumerate()
                .map(|(i, w)| u64::from(state[*w]) << i)
                .sum::<u64>();
            assert_eq!(tuple["inverse_after"], packed);
        }
    }
}
