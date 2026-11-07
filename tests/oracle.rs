use crate::common;
use bennett_sched::{
    expansion::{Operation, simulate_basis},
    object::Object,
    schedule::Phase,
    source::Trace,
};
use serde_json::json;
use std::collections::BTreeMap;
fn increment(width: u64) -> Object {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[width],"tgt":width,"fault":4,"mem":[],"args":[]});
    let function = format!(
        "oracle @inc (x:wire[{width}] in) (t:wire[{width}]) (f:wire[4]) () ()\nregs {{ one:reg[{width}], r:reg[{width}] }}"
    );
    Object::parse(&common::action(
        "oracle",
        "inc",
        sig,
        &function,
        &format!("one = lift.const.i{width} 1\nr = lift.add x one\nlift.retval r"),
    ))
    .unwrap()
}

#[test]
fn padded_oracle_commits_the_reference_result_or_budget_fault_and_cleans_all_state() {
    let object = increment(2);
    for budget in 0..=4 {
        let trace = Trace::build(&object, "inc", 1, &json!([]), [budget, 0, 0, 0]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for input in 0..4 {
            for target in 0..4 {
                for fault in 0..16 {
                    for control in [false, true] {
                        let mut bits = BTreeMap::new();
                        for bit in 0..2 {
                            bits.insert(format!("in:0[{bit}]"), input & (1 << bit) != 0);
                            bits.insert(format!("tgt:0[{bit}]"), target & (1 << bit) != 0);
                        }
                        for bit in 0..4 {
                            bits.insert(format!("fault:0[{bit}]"), fault & (1 << bit) != 0);
                        }
                        bits.insert("control:0".into(), control);
                        simulate_basis(&leaves, &mut bits).unwrap();
                        assert_eq!(bits.len(), 9);
                        let get = |name: &str, n: u64| {
                            (0..n)
                                .map(|i| u64::from(bits[&format!("{name}[{i}]")]) << i)
                                .sum::<u64>()
                        };
                        assert_eq!(get("in:0", 2), input);
                        assert_eq!(
                            get("tgt:0", 2),
                            target
                                ^ if control && budget >= 3 {
                                    (input + 1) % 4
                                } else {
                                    0
                                }
                        );
                        assert_eq!(get("fault:0", 4), fault ^ u64::from(control && budget < 3));
                    }
                }
            }
        }
        for point in ["$construct", "$commit", "$release"] {
            let events = trace
                .events
                .iter()
                .filter(|e| e.location.path.last().unwrap().point == point)
                .collect::<Vec<_>>();
            assert!(!events.is_empty());
            assert!(
                events
                    .iter()
                    .all(|e| e.location.root.step.is_none() && e.location.root.phase.is_none())
            );
            assert_eq!(
                events
                    .iter()
                    .map(|e| e.location.generated)
                    .collect::<Vec<_>>(),
                (0..events.len() as u64).collect::<Vec<_>>()
            );
        }
        for step in 1..=budget {
            let forward = trace
                .events
                .iter()
                .filter(|e| {
                    e.location.root.step == Some(step) && e.location.root.phase == Some(Phase::Fwd)
                })
                .collect::<Vec<_>>();
            let reverse = trace
                .events
                .iter()
                .filter(|e| {
                    e.location.root.step == Some(step) && e.location.root.phase == Some(Phase::Rev)
                })
                .collect::<Vec<_>>();
            assert_eq!(forward.len(), reverse.len());
            for (a, b) in forward.iter().zip(reverse.iter().rev()) {
                assert_eq!(a.location.path, b.location.path);
                assert_eq!(a.location.generated, b.location.generated);
            }
        }
        assert!(
            trace
                .events
                .iter()
                .all(|e| !matches!(e.leaf.op, Operation::Classical { .. }))
        );
    }
}

#[test]
fn oracle_source_names_do_not_collide_with_constructor_state_or_template_formals() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    let bytes = common::action(
        "oracle",
        "names",
        sig,
        "oracle @names (next:wire in) (result:wire) (code:wire[4]) () ()\nregs { pc:reg[1] }",
        "pc = lift.const.i1 1\nlift.retval pc",
    );
    Trace::build(
        &Object::parse(&bytes).unwrap(),
        "names",
        0,
        &json!([]),
        [2, 0, 0, 0],
    )
    .unwrap();
}

#[test]
fn branch_successors_capture_the_old_condition_and_fault_zero_commits_effective_code_four() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    let bytes = common::action(
        "oracle",
        "a",
        sig,
        "oracle @a (x:wire in) (t:wire) (f:wire[4]) () ()\nregs { r:reg[1] }",
        "r = lift.const.i1 0\nqbranch x ^bad ^ok\n^bad():\nfault 0\n^ok():\nr = lift.const.i1 1\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    for budget in 0..=5 {
        let trace = Trace::build(&object, "a", 0, &json!([]), [budget, 0, 0, 0]).unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        for input in 0..2 {
            for target in 0..2 {
                for fault in 0..16 {
                    let mut state = BTreeMap::from([
                        ("in:0[0]".into(), input != 0),
                        ("tgt:0[0]".into(), target != 0),
                    ]);
                    for i in 0..4 {
                        state.insert(format!("fault:0[{i}]"), fault & (1 << i) != 0);
                    }
                    simulate_basis(&leaves, &mut state).unwrap();
                    assert_eq!(state.len(), 6);
                    assert_eq!(state["in:0[0]"], input != 0);
                    assert_eq!(
                        state["tgt:0[0]"],
                        (target ^ u64::from(input == 0 && budget >= 4)) != 0
                    );
                    let code = if input != 0 && budget >= 3 {
                        4
                    } else if input == 0 && budget >= 4 {
                        0
                    } else {
                        1
                    };
                    assert_eq!(
                        (0..4)
                            .map(|i| u64::from(state[&format!("fault:0[{i}]")]) << i)
                            .sum::<u64>(),
                        fault ^ code
                    );
                }
            }
        }
    }
}

#[test]
fn source_literals_and_fault_causes_are_validated_without_truncating_illegal_values() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    for body in [
        "r = lift.const.i1 2\nlift.retval r",
        "r = lift.const.i1 -2\nlift.retval r",
        "r = lift.const.i1 00\nlift.retval r",
        "fault 16",
        "lift.faultif x 16\nlift.retval r",
    ] {
        let bytes = common::action(
            "oracle",
            "a",
            sig.clone(),
            "oracle @a (x:wire in) (t:wire) (f:wire[4]) () ()\nregs { r:reg[1] }",
            body,
        );
        let error = Object::parse(&bytes)
            .and_then(|o| Trace::build(&o, "a", 0, &json!([]), [2, 0, 0, 0]))
            .err()
            .expect(body);
        assert_eq!(error.status, bennett_sched::Status::Arg, "{body}: {error}");
    }
}

#[test]
fn oracle_row_reads_cannot_resolve_targets_faults_or_private_constructor_names() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    for body in [
        "r = lift.mov t\nlift.retval r",
        "r = lift.icmp.eq f f\nlift.retval r",
        "r = lift.icmp.eq pc pc\nlift.retval r",
    ] {
        let bytes = common::action(
            "oracle",
            "a",
            sig.clone(),
            "oracle @a (x:wire in) (t:wire) (f:wire[4]) () ()\nregs { r:reg[1] }",
            body,
        );
        let error = Object::parse(&bytes)
            .and_then(|o| Trace::build(&o, "a", 0, &json!([]), [2, 0, 0, 0]))
            .err()
            .expect(body);
        assert_eq!(error.status, bennett_sched::Status::Arg, "{body}: {error}");
    }
}
