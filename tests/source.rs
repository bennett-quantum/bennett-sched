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
fn mutable_cleanup_dependencies_reject_an_intervening_workspace_change() {
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
    assert_eq!(error.status, bennett_sched::Status::Arg);
    assert!(error.reason.contains("window"));
}

#[test]
fn matched_dependency_changes_are_allowed_when_the_window_state_is_preserved() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"action","mut":[1,1],"in":[],"mem":[],"args":[],"bounds":[]});
    let bytes = common::action(
        "action",
        "a",
        sig,
        "action @a (t:wire mut, c:wire mut) () () () ()",
        "d = qcopy c\nx c\ns t\nx c\nquncopy c d",
    );
    Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).unwrap();
}

#[test]
fn other_workspace_cleans_after_composed_uses_cancel_and_rejects_transferred_dependence() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    for body in [
        "d = qalloc any 1\nh d\nh d\nqfree d",
        "d = qalloc any 1\nh d\ncx d t\ncx d t\nh d\nqfree d",
    ] {
        let bytes = common::action(
            "action",
            "a",
            sig.clone(),
            "action @a (t:wire mut) () () () ()",
            body,
        );
        Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).unwrap();
    }
    let bytes = common::action(
        "action",
        "a",
        sig,
        "action @a (t:wire mut) () () () ()",
        "d = qalloc any 1\nh d\ncx d t\nh d\nqfree d",
    );
    assert!(Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).is_err());
}

#[test]
fn interface_gate_pairs_are_not_mistaken_for_workspace_cleanup_windows() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    let bytes = common::action(
        "action",
        "a",
        sig,
        "action @a (t:wire mut) () () () ()",
        "x t\nh t\nh t\nx t",
    );
    Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4]).unwrap();
}

#[test]
fn semantic_call_inverse_restores_every_output_before_joint_releases() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,1],"in":[],"mem":[],"args":[],"bounds":[]});
    let base = common::action(
        "perm_action",
        "swap2",
        sig,
        "perm_action @swap2 (a:wire mut, b:wire mut) () () () ()",
        "swap a b",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[],"in":[],"mem":[],"args":[],"bounds":[]});
    let bytes = common::caller(
        &base,
        "perm_action",
        "outer",
        sig,
        "perm_action @outer () () () () ()",
        "a = qalloc any 1\nb = qalloc any 1\naction_call @swap2 (a, b) () () ()\nadj action_call @swap2 (a, b) () () ()\nqfree a\nqfree b",
    );
    Trace::build(
        &Object::parse(&bytes).unwrap(),
        "outer",
        0,
        &json!([]),
        [0; 4],
    )
    .unwrap();
}

#[test]
fn release_opcode_matches_allocation_kind_and_intrinsic_cleanup_keeps_its_pair() {
    let signature = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[]});
    for body in [
        "d = qalloc any 1\nqreturn d",
        "d = qalloc any 1\ncx c d\nquncopy c d",
    ] {
        let bytes = common::action(
            "perm_action",
            "a",
            signature.clone(),
            "perm_action @a (t:wire mut) (c:wire in) () () ()",
            body,
        );
        assert_eq!(
            Trace::build(&Object::parse(&bytes).unwrap(), "a", 0, &json!([]), [0; 4])
                .unwrap_err()
                .status,
            bennett_sched::Status::Arg
        );
    }
}

#[test]
fn malformed_loop_without_body_returns_an_argument_error() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[1]});
    let bytes = common::action(
        "perm_action",
        "a",
        sig,
        "perm_action @a (t:wire mut) () () () ()",
        "qloop 1",
    );
    let object = Object::parse(&bytes).unwrap();
    assert_eq!(
        Trace::build(&object, "a", 0, &json!([]), [0; 4])
            .unwrap_err()
            .status,
        bennett_sched::Status::Arg
    );
}
