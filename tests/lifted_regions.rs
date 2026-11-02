use bennett_sched::{expansion::Operation, object::Object, source::Trace};
use serde_json::json;
#[path = "common/basis.rs"]
mod basis;
mod common;

fn program(paired: bool) -> Object {
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]});
    let body = if paired {
        "lift.oracle_call @o b (x) r f () ()\ns = lift.mov r\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval s"
    } else {
        "lift.oracle_call @o b (x) r f () ()\nlift.retval r"
    };
    Object::parse(&common::caller(&base,"oracle","outer",sig,
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1], f:reg[4], s:reg[1] }",body)).unwrap()
}

#[test]
fn lifted_oracle_calls_and_intrinsic_uncalls_keep_independent_budgets_and_clean_complete_small_domains()
 {
    use std::collections::BTreeMap;
    for paired in [false, true] {
        let object = program(paired);
        let required = if paired { 4 } else { 2 };
        for outer in 0..=required {
            for inner in 0..=2 {
                let trace = Trace::build(
                    &object,
                    "outer",
                    1,
                    &json!([[inner.to_string(), "0", "0", "0"]]),
                    [outer, 0, 0, 0],
                )
                .unwrap();
                let leaves = trace
                    .events
                    .iter()
                    .map(|e| e.leaf.clone())
                    .collect::<Vec<_>>();
                let native = basis::Program::compile(&leaves).unwrap();
                let mut cases = Vec::new();
                for bits in 0..128 {
                    let mut state = BTreeMap::new();
                    for (name, n, offset) in [("in:0", 1, 0), ("tgt:0", 1, 1), ("fault:0", 4, 2)] {
                        for bit in 0..n {
                            state.insert(
                                format!("{name}[{bit}]"),
                                bits & (1 << (bit + offset)) != 0,
                            );
                        }
                    }
                    state.insert("control:0".into(), bits & 64 != 0);
                    cases.push(state);
                }
                native.run_many(&mut cases).unwrap();
                for (bits, state) in cases.iter().enumerate() {
                    let c = bits & 64 != 0;
                    let success = outer >= required;
                    assert_eq!(state["in:0[0]"], bits & 1 != 0);
                    assert_eq!(
                        state["tgt:0[0]"],
                        (bits & 2 != 0) ^ (c && success && inner > 0 && bits & 1 != 0)
                    );
                    let code = if c && !success { 1 } else { 0 };
                    for bit in 0..4 {
                        assert_eq!(
                            state[&format!("fault:0[{bit}]")],
                            (bits & (1 << (bit + 2)) != 0) ^ (code & (1 << bit) != 0)
                        );
                    }
                    assert_eq!(state.len(), 7);
                }
                assert!(
                    trace
                        .events
                        .iter()
                        .all(|e| !matches!(e.leaf.op, Operation::Classical { .. }))
                );
            }
        }
    }
}

#[test]
fn lifted_call_paths_retain_callee_ordinals_and_nested_step_coordinates_under_outer_inverse() {
    use bennett_sched::schedule::Phase;
    let object = program(false);
    let trace = Trace::build(
        &object,
        "outer",
        0,
        &json!([["2", "0", "0", "0"]]),
        [2, 0, 0, 0],
    )
    .unwrap();
    assert_eq!(
        trace.invocations.iter().filter(|i| i.name == "o").count(),
        4
    );
    for (owner, inv) in trace.invocations.iter().enumerate().skip(1) {
        assert_eq!(inv.parent, Some(0));
        assert_eq!(inv.budget, [2, 0, 0, 0]);
        assert_eq!(inv.path.len(), 1);
        assert_eq!(inv.path[0].point, "entry:0");
        assert_eq!(inv.path[0].call_index, Some(0));
        let mut seen_fwd = false;
        let mut seen_rev = false;
        for event in &trace.events[inv.start..inv.end] {
            assert_eq!(event.owner, owner);
            assert_eq!(event.location.path.len(), 2);
            assert_eq!(event.leaf.ordinal, event.location.generated);
            let nested = event.location.path[0].entered_oracle.unwrap();
            if nested.phase == Some(Phase::Fwd) {
                seen_fwd = true;
            }
            if nested.phase == Some(Phase::Rev) {
                seen_rev = true;
            }
            if event.location.path[1].point.starts_with('$')
                && event.location.path[1].point != "$step"
            {
                assert_eq!(nested, Default::default());
            }
        }
        assert!(seen_fwd && seen_rev);
    }
}

#[test]
fn a_nested_oracle_request_emits_a_shared_selector_per_address_and_all_history_records() {
    use bennett_sched::{Outcome, Request, canonical, schedule::Document};
    use std::sync::Arc;
    let object = Arc::new(program(false));
    let mut request = Request::default();
    request
        .set_region(object.closure_hash("outer").unwrap(), "forward", 1)
        .unwrap();
    let nested = object.closure_hash("o").unwrap().to_owned();
    request.object = Some(object);
    request
        .set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    request.set_params(br#"[["1","0","0","0"]]"#).unwrap();
    request.budget = Some([2, 0, 0, 0]);
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let document = Document::parse(canonical::require_jcs(&response.schedule).unwrap()).unwrap();
    document.validate_request(&request).unwrap();
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        certificate["regions"][&nested]["frame"]["history"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let count = certificate["composition"]["multiplicities"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.starts_with(&nested))
        .map(|(_, n)| n.as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(count, 4);
}

#[test]
fn zero_leaf_permutation_calls_retain_all_forward_and_inverse_invocation_records() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":[],"bounds":[0]});
    let base = common::action(
        "perm_action",
        "empty",
        sig,
        "perm_action @empty (a:wire mut) (x:wire in) () () ()",
        "qloop 0 {\nx a\n}",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    let bytes = common::caller(
        &base,
        "oracle",
        "outer",
        sig,
        "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () ()\nregs { r:reg[1] }",
        "lift.perm_call @empty (r) (x) () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    let trace = Trace::build(&object, "outer", 0, &json!([]), [2, 0, 0, 0]).unwrap();
    let calls = trace
        .invocations
        .iter()
        .filter(|i| i.name == "empty")
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        4,
        "a zero-leaf call still has a bound invocation context"
    );
    for call in calls {
        assert_eq!(call.start, call.end);
        assert_eq!(call.parent, Some(0));
        assert_eq!(call.path[0].point, "entry:0");
    }
}
