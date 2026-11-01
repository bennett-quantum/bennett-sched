use bennett_sched::{
    expansion::{Operation, simulate_basis},
    object::Object,
    source::Trace,
};
use serde_json::json;
use std::collections::BTreeMap;
mod common;

#[test]
fn published_nested_controlled_action_has_real_invocation_chain_and_one_leaf() {
    let bytes = std::fs::read(
        "../bennett-isa-spec/conformance/fixtures/controlled-action-09/controlled.bisa",
    )
    .unwrap();
    let o = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&o, "controlled", 0, &json!([]), [0; 4]).unwrap();
    assert_eq!(trace.events.len(), 1);
    assert_eq!(trace.events[0].location.path.len(), 2);
    assert_eq!(trace.events[0].location.path[0].call_index, Some(0));
    let Operation::Gate(g) = &trace.events[0].leaf.op else {
        panic!()
    };
    assert_eq!(g.controls.len(), 1);
    assert_eq!(trace.invocations.len(), 2);
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for t in [false, true] {
        for c in [false, true] {
            let mut bits = BTreeMap::from([(g.wires[0].clone(), t), (g.controls[0].clone(), c)]);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(bits[&g.wires[0]], t ^ c);
        }
    }
}

#[test]
fn readonly_roles_predicate_matching_and_permutation_gate_rules_are_enforced() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]});
    for body in ["cx t c", "h t", "qpred c\nx t\nqunpred t"] {
        let bytes = common::action(
            "perm_action",
            "a",
            signature.clone(),
            "perm_action @a (t:wire mut) (c:wire in) () () ()",
            body,
        );
        let object = Object::parse(&bytes).unwrap();
        assert!(
            Trace::build(&object, "a", 0, &json!([]), [0; 4]).is_err(),
            "{body}"
        );
    }
}

#[test]
fn copy_cleanup_restores_workspace_on_the_full_input_domain() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]});
    let bytes = common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut) (c:wire in) () () ()",
        "d = qcopy c\ncx d t\nquncopy c d",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for c in [false, true] {
        for t in [false, true] {
            let mut bits = BTreeMap::from([("mut:0[0]".into(), t), ("in:0[0]".into(), c)]);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(bits.len(), 2);
            assert_eq!(bits["mut:0[0]"], t ^ c);
        }
    }
}

#[test]
fn mutable_cleanup_dependencies_require_the_deferred_window_proof() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,1],"in":[],"mem":[],"args":[],"bounds":[]});
    let bytes = common::action(
        "perm_action",
        "a",
        signature,
        "perm_action @a (t:wire mut, c:wire mut) () () () ()",
        "d = qcopy c\nx c\ncx t d\ncx t d\nx c\nquncopy c d",
    );
    let object = Object::parse(&bytes).unwrap();
    let error = Trace::build(&object, "a", 0, &json!([]), [0; 4]).unwrap_err();
    assert_eq!(error.status, bennett_sched::Status::GateUnsupported);
    assert!(error.reason.contains("window"));
}
