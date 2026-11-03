use bennett_sched::{Status, object::Object, source::Trace};
use serde_json::json;
mod common;

fn object(body: &str) -> Object {
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let signature = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]});
    Object::parse(&common::caller(&base,"oracle","outer",signature,"oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1], s:reg[1], f:reg[4], g:reg[4], a:reg[1], z:reg[1] }",body)).unwrap()
}
fn check(body: &str) -> bennett_sched::Result<Trace> {
    common::trace_distinct_tokens(
        &object(body),
        "outer",
        0,
        &json!([["1", "0", "0", "0"]]),
        [0; 4],
    )
}

#[test]
fn unmatched_ambiguous_non_lifo_and_stale_input_or_output_uncalls_reject_even_at_zero_budget() {
    let cases = [
        (
            "lift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            "unmatched",
        ),
        (
            "lift.oracle_call @o b (x) r f () ()\nlift.oracle_call @o b (x) s g () ()\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            "target",
        ),
        (
            "a = lift.mov x\nlift.oracle_call @o b (a) r f () ()\na = lift.mov x\nlift.oracle_uncall @o b (a) r f () ()\nlift.retval r",
            "input",
        ),
        (
            "lift.oracle_call @o b (x) r f () ()\nr = lift.mov x\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            "version",
        ),
        (
            "qbranch x ^left ^right\n^left():\nlift.oracle_call @o b (x) r f () ()\nqjump ^join\n^right():\nlift.oracle_call @o b (x) r f () ()\nqjump ^join\n^join():\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            "different",
        ),
        (
            "qbranch x ^left ^join\n^left():\nlift.oracle_call @o b (x) r f () ()\nqjump ^join\n^join():\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            "unmatched",
        ),
    ];
    for (body, reason) in cases {
        let error = match check(body) {
            Err(error) => error,
            Ok(_) => panic!("ill-formed uncall accepted ({reason})"),
        };
        assert_eq!(error.status, Status::Arg, "{error}");
        assert!(error.reason.contains(reason), "{error}");
    }
}

#[test]
fn nested_pairs_independent_keys_branches_and_balanced_loops_admit() {
    let cases = [
        "lift.oracle_call @o b (x) r f () ()\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
        "lift.oracle_call @o b (x) r f () ()\nlift.oracle_call @o b (x) r f () ()\nlift.oracle_uncall @o b (x) r f () ()\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
        "a = lift.mov x\nlift.oracle_call @o b (x) r f () ()\nlift.oracle_call @o b (a) s g () ()\nlift.oracle_uncall @o b (x) r f () ()\nlift.oracle_uncall @o b (a) s g () ()\nlift.retval r",
        "lift.oracle_call @o b (x) r f () ()\nqbranch x ^left ^right\n^left():\nz = lift.mov x\nqjump ^join\n^right():\na = lift.mov x\nqjump ^join\n^join():\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
        "qbranch x ^body ^exit\n^body():\nlift.oracle_call @o b (x) r f () ()\nlift.oracle_uncall @o b (x) r f () ()\nqjump ^entry\n^exit():\nlift.retval r",
    ];
    for body in cases {
        assert!(check(body).is_ok(), "{body}");
    }
}

#[test]
fn classical_argument_values_and_token_origins_are_resolved_before_pairing() {
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["i2"]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () (a:i2)",
        "lift.retval x",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token","token","i2","i2"]});
    for distinct in [false, true] {
        let body = format!(
            "lift.oracle_call @o b0 (x) r f () (a0)\nlift.oracle_uncall @o {} (x) r f () (a1)\nlift.retval r",
            if distinct { "b1" } else { "b0" }
        );
        let bytes = common::caller(
            &base,
            "oracle",
            "outer",
            sig.clone(),
            "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b0:token, b1:token, a0:i2, a1:i2)\nregs { r:reg[1], f:reg[4] }",
            &body,
        );
        let object = Object::parse(&bytes).unwrap();
        for equal in [false, true] {
            let params = json!([
                ["1", "0", "0", "0"],
                ["1", "0", "0", "0"],
                "0",
                if equal { "0" } else { "1" }
            ]);
            let result = common::trace_distinct_tokens(&object, "outer", 0, &params, [0; 4]);
            if equal && !distinct {
                assert!(result.is_ok());
            } else {
                assert_eq!(result.err().unwrap().status, Status::Arg);
            }
        }
    }
}

#[path = "common/basis.rs"]
mod basis;
#[test]
fn balanced_loop_pairing_preserves_total_oracle_behavior_for_all_small_inputs_targets_faults_and_controls()
 {
    use std::collections::BTreeMap;
    let object = object(
        "qbranch x ^body ^exit\n^body():\nlift.oracle_call @o b (x) r f () ()\nlift.oracle_uncall @o b (x) r f () ()\nqjump ^entry\n^exit():\nlift.retval r",
    );
    for budget in 0..=5 {
        let trace = common::trace_distinct_tokens(
            &object,
            "outer",
            1,
            &json!([["1", "0", "0", "0"]]),
            [budget, 0, 0, 0],
        )
        .unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let mut cases = Vec::new();
        for bits in 0..128 {
            let mut state = BTreeMap::from([
                ("in:0[0]".into(), bits & 1 != 0),
                ("tgt:0[0]".into(), bits & 2 != 0),
                ("control:0".into(), bits & 64 != 0),
            ]);
            for bit in 0..4 {
                state.insert(format!("fault:0[{bit}]"), bits & (1 << (bit + 2)) != 0);
            }
            cases.push(state);
        }
        basis::Program::compile(&leaves)
            .unwrap()
            .run_many(&mut cases)
            .unwrap();
        for (bits, state) in cases.iter().enumerate() {
            assert_eq!(state["in:0[0]"], bits & 1 != 0);
            assert_eq!(state["tgt:0[0]"], bits & 2 != 0);
            let fault = bits & 64 != 0 && !(bits & 1 == 0 && budget >= 2);
            for bit in 0..4 {
                assert_eq!(
                    state[&format!("fault:0[{bit}]")],
                    (bits & (1 << (bit + 2)) != 0) ^ (bit == 0 && fault)
                );
            }
            assert_eq!(state.len(), 7);
        }
    }
}

#[test]
fn pairing_retains_the_classical_pseudo_location_contract_diagnostic() {
    let base = common::action(
        "oracle",
        "o",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let sig = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[],"tgt":1,"fault":4,"mem":[],"args":["token","i1"]});
    let bytes = common::caller(
        &base,
        "oracle",
        "outer",
        sig,
        "oracle @outer () (t:wire) (fr:wire[4]) () (b:token, a:i1)\nregs { r:reg[1], f:reg[4] }",
        "lift.oracle_call @o b (a) r f () ()\nlift.oracle_uncall @o b (a) r f () ()\nlift.retval r",
    );
    let object = Object::parse(&bytes).unwrap();
    let result = common::trace_distinct_tokens(
        &object,
        "outer",
        0,
        &json!([["1", "0", "0", "0"], "0"]),
        [0; 4],
    );
    let error = result
        .err()
        .expect("pseudo-location storage is not silently guessed");
    assert_eq!(error.status, Status::GateUnsupported);
    assert!(error.reason.contains("bennett-sched-r28"), "{error}");
}

#[test]
fn stores_between_call_and_uncall_do_not_count_as_input_location_writes() {
    let layout = json!({"ptr_width":3,"H":1,"A_tab":0,"S_cells":0,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}});
    let oracle = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    let root = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]});
    let bytes = common::layout_sources(
        &[
            (
                "o",
                "oracle",
                oracle,
                "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()\n^entry():\nlift.retval x",
            ),
            (
                "outer",
                "oracle",
                root,
                "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1], f:reg[4], p:reg[3], v:reg[8] }\n^entry():\np = lift.const.i3 1\nv = lift.const.i8 7\nlift.oracle_call @o b (x) r f () ()\nlift.store.i8 p v\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
            ),
        ],
        layout,
    );
    let object = Object::parse(&bytes).unwrap();
    common::trace_distinct_tokens(&object, "outer", 0, &json!([["1", "0", "0", "0"]]), [0; 4])
        .unwrap();
}

#[test]
fn two_lproc_token_formals_forwarding_one_root_token_retain_its_identity() {
    let layout = json!({"ptr_width":3,"H":0,"A_tab":0,"S_cells":3,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}});
    let oracle = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]});
    let worker = json!({"abi":"0.9","tier":"q","kind":"lproc","params":[1],"result":1,"tokens":["token","token"],"args":[],"mem":[]});
    let root = json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]});
    let bytes = common::layout_sources(
        &[
            (
                "o",
                "oracle",
                oracle,
                "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()\n^entry():\nlift.retval x",
            ),
            (
                "worker",
                "lproc",
                worker,
                "lproc @worker (p:reg[1]) (r:reg[1]) (a:token, b:token) () ()\nregs { f:reg[4] }\n^entry():\nlift.oracle_call @o a (p) r f () ()\nlift.oracle_uncall @o b (p) r f () ()\nlift.ret r",
            ),
            (
                "outer",
                "oracle",
                root,
                "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (b:token)\nregs { r:reg[1] }\n^entry():\nr = lift.call @worker (x) (b, b) () ()\nlift.retval r",
            ),
        ],
        layout,
    );
    let object = Object::parse(&bytes).unwrap();
    common::trace_distinct_tokens(
        &object,
        "outer",
        0,
        &json!([["1", "0", "0", "0"]]),
        [0, 0, 8, 1],
    )
    .unwrap();
}

#[test]
fn unbounded_pending_call_stacks_report_host_exhaustion_without_a_smaller_path_claim() {
    let result = check(
        "lift.oracle_call @o b (x) r f () ()\nqbranch x ^entry ^exit\n^exit():\nlift.oracle_uncall @o b (x) r f () ()\nlift.retval r",
    );
    let error = result
        .err()
        .expect("this analysis has a bounded host-state workspace");
    assert_eq!(error.status, Status::NoMem, "{error}");
    assert!(error.reason.contains("source cleanup analysis"), "{error}");
}
