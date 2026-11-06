use bennett_sched::{expansion::Operation, object::Object};
use serde_json::json;
#[path = "common/basis.rs"]
mod basis;
use crate::common;

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
#[ignore = "slow exhaustive simulation; run by the push gate with --ignored"]
fn lifted_oracle_calls_and_intrinsic_uncalls_keep_independent_budgets_and_clean_complete_small_domains()
 {
    use std::collections::BTreeMap;
    for paired in [false, true] {
        let object = program(paired);
        let required = if paired { 4 } else { 2 };
        for outer in 0..=required {
            for inner in 0..=2 {
                let trace = common::trace_distinct_tokens(
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
    let trace = common::trace_distinct_tokens(
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
    let response = common::schedule_distinct_tokens(&mut request).unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let document = Document::parse(canonical::require_jcs(&response.schedule).unwrap()).unwrap();
    document.validate_request(&request).unwrap();
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    let contexts = certificate["regions"][&nested]["frame_contexts"]
        .as_array()
        .unwrap();
    assert_eq!(contexts.len(), 4);
    let mut ids = std::collections::BTreeSet::new();
    for context in contexts {
        let history = context["frame"]["history"].as_array().unwrap();
        assert_eq!(history.len(), 1);
        assert!(ids.insert(history[0]["id"].as_str().unwrap()));
        assert!(context["context_key"][0]["step"].is_number());
        assert!(context["context_key"][0]["phase"].is_string());
    }
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
    let trace =
        common::trace_distinct_tokens(&object, "outer", 0, &json!([]), [2, 0, 0, 0]).unwrap();
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
    let mut request = bennett_sched::Request::default();
    request
        .set_region(object.closure_hash("outer").unwrap(), "forward", 0)
        .unwrap();
    let callee = object.closure_hash("empty").unwrap().to_owned();
    request.object = Some(std::sync::Arc::new(object));
    request
        .set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    request.set_params(b"[]").unwrap();
    request.budget = Some([2, 0, 0, 0]);
    request.set_accuracy("0").unwrap();
    let response = common::schedule_distinct_tokens(&mut request).unwrap();
    assert_eq!(
        response.outcome,
        bennett_sched::Outcome::Ok,
        "{:?}",
        response.reason
    );
    let certificate = bennett_sched::canonical::require_jcs(&response.certificate).unwrap();
    let count = certificate["composition"]["multiplicities"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(key, _)| key.starts_with(&callee))
        .map(|(_, n)| n.as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(count, 4);
}

fn forwarded_lproc_program(coherent: bool) -> Object {
    use bennett_sched::canonical;
    use serde_json::Value;
    let signatures = [
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        json!({"abi":"0.9","tier":"q","kind":"lproc","params":[1],"result":1,"tokens":["token"],"args":[],"mem":[]}),
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":if coherent{vec![1,1]}else{vec![1]},"tgt":1,"fault":4,"mem":[],"args":["token","token"]}),
    ];
    let functions = [
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()\n^entry():\nlift.retval x",
        "lproc @worker (p:reg[1]) (r:reg[1]) (b:token) () ()\nregs { f:reg[4] }\n^entry():\nlift.oracle_call @o b (p) r f () ()\nlift.ret r",
        if coherent {
            "oracle @outer (x:wire in, c:wire in) (t:wire) (fr:wire[4]) () (b0:token, b1:token)\nregs { y:reg[1] }\n^entry():\nqbranch c ^left ^right\n^left():\ny = lift.call @worker (x) (b0) () ()\nqjump ^done\n^right():\ny = lift.call @worker (x) (b1) () ()\nqjump ^done\n^done():\nlift.retval y"
        } else {
            "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b0:token, b1:token)\nregs { y:reg[1], z:reg[1], r:reg[1] }\n^entry():\ny = lift.call @worker (x) (b0) () ()\nz = lift.call @worker (x) (b1) () ()\nr = lift.xor y z\nlift.retval r"
        },
    ];
    let layout = json!({"ptr_width":3,"H":0,"A_tab":0,"S_cells":3,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}});
    let jcs = |v: &Value| String::from_utf8(canonical::to_vec(v).unwrap()).unwrap();
    let mut leaves = layout
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| !["globals", "interface_regions"].contains(&k.as_str()))
        .map(|(k, v)| (jcs(&json!([k])), jcs(v)))
        .collect::<Vec<_>>();
    leaves.sort();
    let layout_suffix = leaves
        .iter()
        .map(|(p, v)| format!("layout {p} {v}\n"))
        .collect::<String>();
    let names = ["o", "worker", "outer"];
    let kinds = ["oracle", "lproc", "oracle"];
    let mut hashes = Vec::<String>::new();
    let mut symbols = json!({});
    for index in 0..3 {
        let mut body = functions[index].to_owned();
        let mut refs = Vec::new();
        for earlier in 0..index {
            if body.contains(&format!("@{} ", names[earlier])) {
                body = body.replace(
                    &format!("@{} ", names[earlier]),
                    &format!("@{} ", hashes[earlier]),
                );
                refs.push(hashes[earlier].clone());
            }
        }
        refs.sort();
        refs.dedup();
        let suffix = refs.iter().map(|h| format!("{h}\n")).collect::<String>();
        let hash = canonical::hash(
            format!(
                "isa=0.9\nptr_width=3\n{}\n{body}\n{}{suffix}",
                jcs(&signatures[index]),
                if kinds[index] == "oracle" {
                    layout_suffix.as_str()
                } else {
                    ""
                }
            )
            .as_bytes(),
        );
        symbols[index.to_string()] = json!({"kind":kinds[index],"name":names[index],"signature":signatures[index],"interface_hash":hash});
        hashes.push(hash);
    }
    let source = format!(
        "bennett-object isa=0.9 object=0.9 launch=0.9 sched=0.9 cert=0.9 coverage=0.9 target=0.9 ptr_width=3\n\nsection symbols\n{}\n\nsection functions\n{}\n\nsection layout\n{}\n",
        jcs(&symbols),
        functions.join("\n"),
        jcs(&layout)
    );
    Object::parse(source.as_bytes()).unwrap()
}

#[test]
#[ignore = "slow exhaustive simulation; run by the push gate with --ignored"]
fn fixed_lproc_token_forwarding_keeps_two_positional_instances_without_lift_call_edges() {
    use std::collections::{BTreeMap, BTreeSet};
    let object = forwarded_lproc_program(false);
    let space = bennett_sched::lproc::IndexSpace::form(&object, "outer").unwrap();
    assert!(space.instances.iter().all(|i| i.activation_cells == 6));
    for (first, second) in [(1, 0), (1, 1), (0, 1)] {
        for outer in [0, 7, 8] {
            let params = json!([
                [first.to_string(), "0", "0", "0"],
                [second.to_string(), "0", "0", "0"]
            ]);
            let trace =
                common::trace_distinct_tokens(&object, "outer", 1, &params, [outer, 0, 8, 1])
                    .unwrap();
            let callees = trace
                .invocations
                .iter()
                .filter(|i| i.name == "o")
                .collect::<Vec<_>>();
            if outer > 0 {
                assert_eq!(callees.len(), outer as usize * 4);
                let keys = callees
                    .iter()
                    .map(|i| i.path[0].instance.clone())
                    .collect::<BTreeSet<_>>();
                assert_eq!(keys, BTreeSet::from([vec![0], vec![1]]));
                for inv in callees {
                    assert_eq!(
                        inv.path.len(),
                        1,
                        "lift.call is not a region invocation edge"
                    );
                    assert_eq!(inv.path[0].symbol, object.symbols["worker"].interface_hash);
                    assert_eq!(inv.path[0].point, "entry:0");
                    assert_eq!(inv.path[0].call_index, Some(0));
                    assert_eq!(
                        inv.budget[0],
                        if inv.path[0].instance == [0] {
                            first
                        } else {
                            second
                        }
                    );
                }
            }
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            let mut cases = Vec::new();
            for bits in 0..128 {
                let mut state = BTreeMap::new();
                for (name, n, offset) in [("in:0", 1, 0), ("tgt:0", 1, 1), ("fault:0", 4, 2)] {
                    for bit in 0..n {
                        state.insert(format!("{name}[{bit}]"), bits & (1 << (offset + bit)) != 0);
                    }
                }
                state.insert("control:0".into(), bits & 64 != 0);
                cases.push(state);
            }
            basis::Program::compile(&leaves)
                .unwrap()
                .run_many(&mut cases)
                .unwrap();
            for (bits, state) in cases.iter().enumerate() {
                // The second call needs another six monotonic stack cells.
                // With only eight available it faults before any outer retval.
                assert_eq!(
                    state["tgt:0[0]"],
                    bits & 2 != 0,
                    "outer={outer}, tokens={first},{second}"
                );
                let code = if outer < 4 { 1 } else { 3 };
                for bit in 0..4 {
                    assert_eq!(
                        state[&format!("fault:0[{bit}]")],
                        (bits & (1 << (bit + 2)) != 0) ^ (bits & 64 != 0 && code & (1 << bit) != 0)
                    );
                }
                assert_eq!(state["in:0[0]"], bits & 1 != 0);
                assert_eq!(state.len(), 7);
            }
        }
    }
}

#[test]
#[ignore = "slow exhaustive simulation; run by the push gate with --ignored"]
fn coherent_lproc_paths_execute_both_fixed_token_instances_over_the_complete_interface() {
    use std::collections::BTreeMap;
    let object = forwarded_lproc_program(true);
    for (first, second) in [(1, 0), (1, 1), (0, 1)] {
        for outer in [0, 5, 6] {
            let params = json!([
                [first.to_string(), "0", "0", "0"],
                [second.to_string(), "0", "0", "0"]
            ]);
            let trace =
                common::trace_distinct_tokens(&object, "outer", 1, &params, [outer, 0, 8, 1])
                    .unwrap();
            let leaves = trace
                .events
                .iter()
                .map(|e| e.leaf.clone())
                .collect::<Vec<_>>();
            let mut cases = Vec::new();
            for bits in 0..256 {
                let mut state = BTreeMap::from([
                    ("in:0[0]".into(), bits & 1 != 0),
                    ("in:1[0]".into(), bits & 2 != 0),
                    ("tgt:0[0]".into(), bits & 4 != 0),
                    ("control:0".into(), bits & 128 != 0),
                ]);
                for bit in 0..4 {
                    state.insert(format!("fault:0[{bit}]"), bits & (1 << (bit + 3)) != 0);
                }
                cases.push(state);
            }
            basis::Program::compile(&leaves)
                .unwrap()
                .run_many(&mut cases)
                .unwrap();
            for (bits, state) in cases.iter().enumerate() {
                let selected = if bits & 2 != 0 { first } else { second };
                assert_eq!(
                    state["tgt:0[0]"],
                    (bits & 4 != 0)
                        ^ (outer >= 6 && bits & 128 != 0 && selected > 0 && bits & 1 != 0),
                    "outer={outer}, tokens={first},{second}, input={bits}"
                );
                for bit in 0..4 {
                    assert_eq!(
                        state[&format!("fault:0[{bit}]")],
                        (bits & (1 << (bit + 3)) != 0) ^ (bit == 0 && outer < 6 && bits & 128 != 0)
                    );
                }
                assert_eq!(state["in:0[0]"], bits & 1 != 0);
                assert_eq!(state["in:1[0]"], bits & 2 != 0);
                assert_eq!(state.len(), 8);
            }
        }
    }
}
