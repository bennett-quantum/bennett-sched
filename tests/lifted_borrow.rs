use bennett_sched::{
    Outcome, Request, canonical, expansion::Operation, object::Object, source::Trace,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
#[path = "common/basis.rs"]
mod basis;
#[allow(dead_code)]
mod common;

fn borrowed_source() -> Vec<u8> {
    let spec = std::fs::read_to_string("../bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md").unwrap();
    spec.split("**Example 3**")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n')
        .as_bytes()
        .to_vec()
}
#[test]
fn source_adjoint_borrow_frames_use_the_adjoint_table_over_the_forward_translation_proof() {
    let bytes = common::caller(
        &borrowed_source(),
        "perm_action",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]}),
        "perm_action @outer (t:wire mut) (x:wire in) () () ()",
        "adj action_call @borrow_demo (t) (x) () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "outer", 1, &json!([]), [0; 4]).unwrap();
    let borrow = trace
        .events
        .iter()
        .find_map(|e| match &e.leaf.op {
            Operation::Alloc { wires, state, .. } if state == "borrowed" => Some(wires[0].clone()),
            _ => None,
        })
        .unwrap();
    let gates = trace
        .events
        .iter()
        .filter(|e| matches!(e.leaf.op, Operation::Gate(_)))
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for tuple in 0..16 {
        let mut state = BTreeMap::from([
            ("mut:0[0]".into(), tuple & 1 != 0),
            ("in:0[0]".into(), tuple & 2 != 0),
            (borrow.clone(), tuple & 4 != 0),
            ("control:0".into(), tuple & 8 != 0),
        ]);
        let before = state.clone();
        bennett_sched::expansion::simulate_basis(&gates, &mut state).unwrap();
        assert_eq!(state[&borrow], before[&borrow]);
        assert_eq!(
            state["mut:0[0]"],
            before["mut:0[0]"] ^ (before["in:0[0]"] && before["control:0"])
        );
    }
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("outer").unwrap(), "forward", 1)
        .unwrap();
    request.set_params(b"[]").unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    let region = &certificate["regions"][object.closure_hash("borrow_demo").unwrap()];
    let index = region["verification"]["borrow_frames"][0].as_u64().unwrap() as usize;
    let proof = &region["claims"][index];
    assert_eq!(proof["rule"], "adjoint_table");
    let base = proof["subclaims"][0].as_u64().unwrap() as usize;
    assert_eq!(region["claims"][base]["rule"], "translation_lemma");
}
#[test]
fn lifted_borrow_calls_keep_forward_and_inverse_proofs_and_restore_all_dirty_inputs() {
    let bytes = common::caller(
        &borrowed_source(),
        "oracle",
        "outer",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @borrow_demo (r) (x) () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    for budget in 0..=2 {
        let trace = Trace::build(&object, "outer", 1, &json!([]), [budget, 0, 0, 0]).unwrap();
        assert_eq!(trace.borrow_proofs.len(), 2 * budget as usize);
        let borrowed = trace
            .events
            .iter()
            .filter_map(|e| match &e.leaf.op {
                Operation::Alloc { wires, state, .. } if state == "borrowed" => Some(wires.clone()),
                _ => None,
            })
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let leaves = trace.events.iter().filter(|e|!matches!(&e.leaf.op,Operation::Alloc{state,..}|Operation::Release{state,..} if state=="borrowed")).map(|e|e.leaf.clone()).collect::<Vec<_>>();
        let native = basis::Program::compile(&leaves).unwrap();
        let mut cases = Vec::new();
        for dirty in 0..1usize << borrowed.len() {
            for tuple in 0..128 {
                let mut state = BTreeMap::from([
                    ("in:0[0]".into(), tuple & 1 != 0),
                    ("tgt:0[0]".into(), tuple & 2 != 0),
                    ("control:0".into(), tuple & 64 != 0),
                ]);
                for bit in 0..4 {
                    state.insert(format!("fault:0[{bit}]"), tuple & (1 << (bit + 2)) != 0);
                }
                for (bit, name) in borrowed.iter().enumerate() {
                    state.insert(name.clone(), dirty & (1 << bit) != 0);
                }
                cases.push(state);
            }
        }
        let before = cases.clone();
        native.run_many(&mut cases).unwrap();
        for (state, initial) in cases.iter().zip(&before) {
            for name in borrowed.iter().chain(
                ["in:0[0]", "control:0"]
                    .iter()
                    .map(|n| *n)
                    .map(String::from)
                    .collect::<Vec<_>>()
                    .iter(),
            ) {
                assert_eq!(state[name], initial[name]);
            }
            assert_eq!(
                state["tgt:0[0]"],
                initial["tgt:0[0]"] ^ (budget == 2 && initial["in:0[0]"] && initial["control:0"])
            );
            for bit in 0..4 {
                let name = format!("fault:0[{bit}]");
                assert_eq!(
                    state[&name],
                    initial[&name] ^ (budget < 2 && bit == 0 && initial["control:0"])
                );
            }
            assert_eq!(state.len(), 7 + borrowed.len());
        }
        if budget == 2 {
            let mut r = Request::default();
            r.set_object(&bytes).unwrap();
            r.set_target(&std::fs::read("data/k83-target.json").unwrap())
                .unwrap();
            r.set_region(object.closure_hash("outer").unwrap(), "forward", 1)
                .unwrap();
            r.set_params(b"[]").unwrap();
            r.budget = Some([budget, 0, 0, 0]);
            r.set_accuracy("0").unwrap();
            let response = r.schedule().unwrap();
            assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
            let c = canonical::require_jcs(&response.certificate).unwrap();
            let region = &c["regions"][object.closure_hash("borrow_demo").unwrap()];
            let frames = region["frame_contexts"].as_array().unwrap();
            assert_eq!(frames.len(), 4);
            assert!(
                frames
                    .iter()
                    .all(|f| f["frame"]["borrow"].as_array().unwrap().len() == 1)
            );
            let proofs = region["verification"]["borrow_frames"].as_array().unwrap();
            assert_eq!(proofs.len(), 4);
            let rules = proofs
                .iter()
                .map(|p| {
                    region["claims"][p.as_u64().unwrap() as usize]["rule"]
                        .as_str()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            assert_eq!(
                rules.iter().filter(|r| **r == "translation_lemma").count(),
                2
            );
            assert_eq!(rules.iter().filter(|r| **r == "adjoint_table").count(), 2);
        }
    }
}
