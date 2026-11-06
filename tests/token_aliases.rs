use bennett_sched::{Outcome, Request, Status, canonical, object::Object};
use serde_json::json;
#[path = "common/basis.rs"]
mod basis;
use crate::common;

fn paired_tokens() -> (Request, Object) {
    let oracle = common::action(
        "oracle",
        "identity",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @identity (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let bytes = common::caller(
        &oracle,
        "perm_action",
        "pair",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,4],"in":[1],"mem":[],"args":["token","token"],"bounds":[]}),
        "perm_action @pair (t:wire mut, f:wire[4] mut) (x:wire in) () (a:token, b:token) ()",
        "oracle_call @identity a (x) t f () ()\noracle_call @identity b (x) t f () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("pair").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(&canonical::to_vec(&json!([["1", "0", "0", "0"], ["1", "0", "0", "0"]])).unwrap())
        .unwrap();
    r.budget = Some([0; 4]);
    r.set_accuracy("0").unwrap();
    (r, object)
}
#[test]
fn legacy_tuple_only_token_argument_requests_require_an_explicit_alias_binding() {
    let (r, _) = paired_tokens();
    let error = r
        .schedule()
        .expect_err("a budget tuple cannot choose a token identity partition");
    assert_eq!(error.status, Status::Arg);
    assert!(error.reason.contains("token_aliases"));
}

#[test]
fn alias_setters_copy_canonical_partitions_and_reject_malformed_input_atomically() {
    let mut r = Request::default();
    let mut bytes = b"[0,1,0]".to_vec();
    r.set_token_aliases(&bytes).unwrap();
    bytes.fill(b' ');
    assert_eq!(r.token_aliases, Some(vec![0, 1, 0]));
    for bytes in [
        b"[]\n".as_slice(),
        b"[1]",
        b"[0,2]",
        b"[0,-1]",
        b"[\"0\"]",
        b"[0.0]",
        b"{}",
    ] {
        let error = r.set_token_aliases(bytes).unwrap_err();
        assert_eq!(error.status, Status::Arg);
        assert_eq!(r.token_aliases, Some(vec![0, 1, 0]));
    }
}

#[test]
fn shared_and_distinct_equal_tuple_tokens_bind_different_schedules_and_cleanup_pairs() {
    let mut hashes = Vec::new();
    let mut params_hashes = Vec::new();
    for aliases in [vec![0, 0], vec![0, 1]] {
        let (mut r, object) = paired_tokens();
        r.set_token_aliases(&canonical::to_vec(&json!(aliases)).unwrap())
            .unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let schedule = canonical::require_jcs(&response.schedule).unwrap();
        let c = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(schedule["bindings"]["token_aliases"], json!(aliases));
        assert_eq!(c["bindings"]["invocation"]["token_aliases"], json!(aliases));
        let pairs = c["regions"][object.closure_hash("pair").unwrap()]["cleanup"]["forward:0"]
            .as_array()
            .unwrap();
        assert_eq!(pairs.len(), if aliases[0] == aliases[1] { 1 } else { 0 });
        hashes.push(canonical::hash(&response.schedule));
        params_hashes.push(schedule["bindings"]["params_hash"].clone());
    }
    assert_ne!(hashes[0], hashes[1]);
    assert_eq!(params_hashes[0], params_hashes[1]);
}

#[test]
fn alias_arity_and_tuple_consistency_are_checked_before_scheduling() {
    let (mut r, _) = paired_tokens();
    for aliases in [b"[]".as_slice(), b"[0]", b"[0,1,2]"] {
        r.set_token_aliases(aliases).unwrap();
        assert_eq!(r.schedule().unwrap_err().status, Status::Arg);
    }
    r.set_token_aliases(b"[0,0]").unwrap();
    r.set_params(br#"[["1","0","0","0"],["2","0","0","0"]]"#)
        .unwrap();
    assert_eq!(r.schedule().unwrap_err().status, Status::Arg);
    r.set_token_aliases(b"[0,1]").unwrap();
    assert_eq!(r.schedule().unwrap().outcome, Outcome::Ok);
}

#[test]
fn oracle_primary_identity_and_all_consistent_three_token_partitions_are_supported() {
    let bytes = common::action(
        "oracle",
        "probe",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token","token"]}),
        "oracle @probe (x:wire in) (t:wire) (f:wire[4]) () (a:token, b:token)",
        "lift.retval x",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("probe").unwrap(), "forward", 0)
        .unwrap();
    r.budget = Some([1, 0, 0, 0]);
    r.set_params(br#"[["1","0","0","0"],["1","0","0","0"]]"#)
        .unwrap();
    r.set_accuracy("0").unwrap();
    for aliases in [
        vec![0, 0, 0],
        vec![0, 0, 1],
        vec![0, 1, 0],
        vec![0, 1, 1],
        vec![0, 1, 2],
    ] {
        r.set_token_aliases(&canonical::to_vec(&json!(aliases)).unwrap())
            .unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(c["bindings"]["invocation"]["token_aliases"], json!(aliases));
        assert_eq!(
            c["regions"][object.closure_hash("probe").unwrap()]["claims"][0]["invocation"]["token_aliases"],
            json!(aliases)
        );
    }
    r.set_token_aliases(b"[0,0,1]").unwrap();
    r.budget = Some([2, 0, 0, 0]);
    assert_eq!(r.schedule().unwrap_err().status, Status::Arg);
}

#[test]
fn schedules_cannot_be_reused_under_another_partition_with_the_same_params_hash() {
    use bennett_sched::schedule::Document;
    let (mut r, _) = paired_tokens();
    r.set_token_aliases(b"[0,0]").unwrap();
    let response = r.schedule().unwrap();
    let document = Document::parse(canonical::require_jcs(&response.schedule).unwrap()).unwrap();
    document.validate_request(&r).unwrap();
    r.set_token_aliases(b"[0,1]").unwrap();
    let error = document.validate_request(&r).unwrap_err();
    assert_eq!(error.status, Status::ScheduleInfeasible);
    assert!(error.reason.contains("token_aliases"));
}

#[test]
fn both_action_partitions_preserve_every_input_target_and_fault_tuple() {
    use std::collections::BTreeMap;
    for aliases in [vec![0, 0], vec![0, 1]] {
        let (r, object) = paired_tokens();
        let trace = bennett_sched::source::Trace::build_with_token_aliases(
            &object,
            "pair",
            0,
            r.params.as_ref().unwrap(),
            [0; 4],
            &aliases,
        )
        .unwrap();
        let leaves = trace
            .events
            .iter()
            .map(|e| e.leaf.clone())
            .collect::<Vec<_>>();
        let native = basis::Program::compile(&leaves).unwrap();
        let mut cases = (0..64)
            .map(|tuple| {
                let mut state = BTreeMap::from([
                    ("in:0[0]".into(), tuple & 1 != 0),
                    ("mut:0[0]".into(), tuple & 2 != 0),
                ]);
                for bit in 0..4 {
                    state.insert(format!("mut:1[{bit}]"), tuple & (1 << (2 + bit)) != 0);
                }
                state
            })
            .collect::<Vec<_>>();
        let before = cases.clone();
        native.run_many(&mut cases).unwrap();
        assert_eq!(cases, before);
    }
}

#[test]
fn captured_source_arguments_keep_global_ids_instead_of_reusing_local_partition_labels() {
    use bennett_sched::expansion::{Binding, Env, Library};
    use std::sync::Arc;
    for shared in [true, false] {
        let (_, object) = paired_tokens();
        let mut library = Library::published().unwrap();
        let hash = library.link_source(Arc::new(object), "pair").unwrap();
        let mut env = Env::new();
        env.insert("c".into(), Binding::Wires(vec!["c".into()]));
        env.insert("S".into(), Binding::Symbol(hash.clone()));
        let ids = vec![7, if shared { 7 } else { 11 }];
        env.insert(
            "A".into(),
            Binding::Arguments {
                symbol: hash,
                values: vec![
                    Binding::Wires(vec!["t".into()]),
                    Binding::Wires((0..4).map(|i| format!("f[{i}]")).collect()),
                    Binding::Wires(vec!["x".into()]),
                    Binding::Value {
                        isa_type: json!("token"),
                        value: json!(["1", "0", "0", "0"]),
                        token_identity: Some(ids[0]),
                    },
                    Binding::Value {
                        isa_type: json!("token"),
                        value: json!(["1", "0", "0", "0"]),
                        token_identity: Some(ids[1]),
                    },
                ],
                oracle_invocation: None,
                oracle_identity: None,
            },
        );
        let expanded = library
            .body_with_sources(
                &[json!({"invoke":"ctl","args":["c",[{"invoke":"action_call","args":["S","A"]}]]})],
                &env,
                &[],
            )
            .unwrap();
        assert_eq!(expanded.source_calls.len(), 1);
        let trace = &expanded.source_calls[0].trace;
        assert_eq!(trace.invocations[0].token_ids, ids);
        assert_eq!(trace.cleanups.len(), usize::from(shared));
        for child in trace.invocations.iter().filter(|i| i.name == "identity") {
            assert_eq!(
                bennett_sched::token_aliases::partition(&child.token_ids),
                vec![0]
            );
        }
    }
}

#[test]
fn aliased_root_positions_preserve_one_identity_through_fixed_lproc_formals() {
    let functions = [
        (
            "o",
            "oracle",
            json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
            "oracle @o (x:wire in) (t:wire) (f:wire[4]) () ()\n^entry():\nlift.retval x",
        ),
        (
            "worker",
            "lproc",
            json!({"abi":"0.9","tier":"q","kind":"lproc","params":[1],"result":1,"tokens":["token","token"],"args":[],"mem":[]}),
            "lproc @worker (p:reg[1]) (r:reg[1]) (a:token, b:token) () ()\nregs { f:reg[4] }\n^entry():\nlift.oracle_call @o a (p) r f () ()\nlift.oracle_uncall @o b (p) r f () ()\nlift.ret r",
        ),
        (
            "outer",
            "oracle",
            json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token","token"]}),
            "oracle @outer (x:wire in) (t:wire) (fr:wire[4]) () (a:token, b:token)\nregs { y:reg[1] }\n^entry():\ny = lift.call @worker (x) (a, b) () ()\nlift.retval y",
        ),
    ];
    let bytes = common::layout_sources(
        &functions,
        json!({"ptr_width":3,"H":0,"A_tab":0,"S_cells":3,"null_size":1,"heap_base":1,"heap_limit_max":0,"globals":{},"interface_regions":{}}),
    );
    let object = Object::parse(&bytes).unwrap();
    let params = json!([["1", "0", "0", "0"], ["1", "0", "0", "0"]]);
    bennett_sched::source::Trace::build_with_token_aliases(
        &object,
        "outer",
        0,
        &params,
        [0; 4],
        &[0, 1, 1],
    )
    .unwrap();
    let error = bennett_sched::source::Trace::build_with_token_aliases(
        &object,
        "outer",
        0,
        &params,
        [0; 4],
        &[0, 1, 2],
    )
    .unwrap_err();
    assert_eq!(error.status, Status::Arg);
    assert!(error.reason.contains("§5.5"));
}

#[test]
fn legacy_token_free_requests_still_emit_the_inferred_partition() {
    for kind in ["perm_action", "oracle"] {
        let (signature, header, body, aliases) = if kind == "oracle" {
            (
                json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
                "oracle @plain (x:wire in) (t:wire) (f:wire[4]) () ()",
                "lift.retval x",
                json!([0]),
            )
        } else {
            (
                json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]}),
                "perm_action @plain (x:wire mut) () () () ()",
                "x x",
                json!([]),
            )
        };
        let bytes = common::action(kind, "plain", signature, header, body);
        let object = Object::parse(&bytes).unwrap();
        let mut request = Request::default();
        request.set_object(&bytes).unwrap();
        request
            .set_target(include_bytes!("../data/k83-target.json"))
            .unwrap();
        request
            .set_region(object.closure_hash("plain").unwrap(), "forward", 0)
            .unwrap();
        request.set_params(b"[]").unwrap();
        request.budget = Some([1, 0, 0, 0]);
        request.set_accuracy("0").unwrap();
        let response = request.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let schedule = canonical::require_jcs(&response.schedule).unwrap();
        let certificate = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(schedule["bindings"]["token_aliases"], aliases);
        assert_eq!(
            certificate["bindings"]["invocation"]["token_aliases"],
            aliases
        );
        let mut missing = schedule;
        missing["bindings"]
            .as_object_mut()
            .unwrap()
            .remove("token_aliases");
        assert_eq!(
            bennett_sched::schedule::Document::parse(missing)
                .unwrap_err()
                .status,
            Status::Arg
        );
    }
}
