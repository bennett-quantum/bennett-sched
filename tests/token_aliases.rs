use bennett_sched::{Outcome, Request, Status, canonical, object::Object};
use serde_json::json;
mod common;

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
