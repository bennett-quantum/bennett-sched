use crate::common;
use bennett_sched::{Outcome, Request, canonical, object::Object};
use serde_json::{Value, json};

fn request(second: u64, adjoint: bool) -> (Request, Object) {
    let oracle = common::action(
        "oracle",
        "identity",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @identity (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let inner = common::caller(
        &oracle,
        "perm_action",
        "inner",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":["token"],"bounds":[]}),
        "perm_action @inner (t:wire mut) () () (k:token) ()",
        "qinplace @identity @identity k t",
    );
    let orientation = if adjoint { "adj " } else { "" };
    let bytes = common::caller(
        &inner,
        "perm_action",
        "root",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[1],"mem":[],"args":["token","token"],"bounds":[]}),
        "perm_action @root (t:wire mut) (c:wire in) () (a:token, b:token) ()",
        &format!(
            "{orientation}action_call @inner (t) () () (a)\nqpred c\n{orientation}action_call @inner (t) () () (b)\nqunpred c"
        ),
    );
    let object = Object::parse(&bytes).unwrap();
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("root").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(
        &canonical::to_vec(&json!([
            ["1", "0", "0", "0"],
            [second.to_string(), "0", "0", "0"]
        ]))
        .unwrap(),
    )
    .unwrap();
    r.set_token_aliases(b"[0,1]").unwrap();
    r.budget = Some([9, 0, 0, 0]);
    r.set_accuracy("0").unwrap();
    (r, object)
}

#[test]
fn transitive_inverse_pairs_stay_owner_local_and_use_each_forwarded_token() {
    for adjoint in [false, true] {
        let (r, object) = request(1, adjoint);
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c: Value = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(
            c["regions"][object.closure_hash("root").unwrap()]["verification"]["inverse_pairs"],
            json!([])
        );
        let inner = &c["regions"][object.closure_hash("inner").unwrap()];
        let pairs = inner["verification"]["inverse_pairs"].as_array().unwrap();
        assert_eq!(pairs.len(), 2);
        for pair in pairs {
            assert_eq!(pair["g"], object.closure_hash("identity").unwrap());
            assert_eq!(pair["ginv"], pair["g"]);
            assert_eq!(pair["budget"], json!(["1", "0", "0", "0"]));
            assert_eq!(pair["token_aliases"], json!([0]));
            let claim = &inner["claims"][pair["claim"].as_u64().unwrap() as usize];
            assert_eq!(claim["kind"], "exhaustive");
            assert_eq!(claim["expected"], "inverse_pair");
            assert_eq!(
                claim["domain"]["bundles"],
                json!([{"name":"x","width":1,"lo":0,"hi":1}])
            );
        }
        let (r, _) = request(0, adjoint);
        let response = r.schedule().unwrap();
        assert_eq!(
            response.outcome,
            Outcome::Infeasible,
            "{:?}",
            response.reason
        );
        assert!(response.schedule.is_empty() && response.certificate.is_empty());
    }
}

#[test]
fn zero_trip_body_retains_static_pair_inventory_without_invocation_visits() {
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
        "root",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":["token"],"bounds":[0]}),
        "perm_action @root (t:wire mut) () () (k:token) ()",
        "qloop 0 {\nqinplace @identity @identity k t\n}",
    );
    let object = Object::parse(&bytes).unwrap();
    for token_budget in [0, 1] {
        let mut r = Request::default();
        r.set_object(&bytes).unwrap();
        r.set_target(&std::fs::read("data/k83-target.json").unwrap())
            .unwrap();
        r.set_region(object.closure_hash("root").unwrap(), "forward", 0)
            .unwrap();
        r.set_params(
            &canonical::to_vec(&json!([[token_budget.to_string(), "0", "0", "0"]])).unwrap(),
        )
        .unwrap();
        r.set_token_aliases(b"[0]").unwrap();
        r.budget = Some([0; 4]);
        r.set_accuracy("0").unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c = canonical::require_jcs(&response.certificate).unwrap();
        let root = &c["regions"][object.closure_hash("root").unwrap()];
        let pairs = root["verification"]["inverse_pairs"].as_array().unwrap();
        assert_eq!(pairs.len(), 1, "K92 requires static pair inventory");
        assert_eq!(pairs[0]["g"], object.closure_hash("identity").unwrap());
        assert_eq!(pairs[0]["ginv"], pairs[0]["g"]);
        assert_eq!(pairs[0]["budget"], json!(["1", "0", "0", "0"]));
        assert_eq!(
            c["bindings"]["invocation"]["args"],
            json!([[token_budget.to_string(), "0", "0", "0"]])
        );
        assert_eq!(root["frame"]["scratch"], json!([]));
        assert_eq!(root["gates_used"], json!({}));
        assert_eq!(c["regions"].as_object().unwrap().len(), 1);
        assert_eq!(
            c["composition"]["multiplicities"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn unentered_primary_identity_class_is_consistent_in_its_standalone_theorem() {
    let oracle = common::action(
        "oracle",
        "identity",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":["token"]}),
        "oracle @identity (x:wire in) (t:wire) (f:wire[4]) () (other:token)",
        "lift.retval x",
    );
    let bytes = common::caller(
        &oracle,
        "perm_action",
        "root",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":["token"],"bounds":[0]}),
        "perm_action @root (t:wire mut) () () (k:token) ()",
        "qloop 0 {\nqinplace @identity @identity k t () () (k)\n}",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&std::fs::read("data/k83-target.json").unwrap())
        .unwrap();
    r.set_region(object.closure_hash("root").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(br#"[["0","0","0","0"]]"#).unwrap();
    r.set_token_aliases(b"[0]").unwrap();
    r.budget = Some([0; 4]);
    r.set_accuracy("0").unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let c = canonical::require_jcs(&response.certificate).unwrap();
    let entry =
        &c["regions"][object.closure_hash("root").unwrap()]["verification"]["inverse_pairs"][0];
    assert_eq!(entry["budget"], json!(["1", "0", "0", "0"]));
    assert_eq!(entry["arg_tokens"], json!({"other":["1","0","0","0"]}));
    assert_eq!(entry["token_aliases"], json!([0, 0]));
    assert_eq!(
        c["bindings"]["invocation"]["args"],
        json!([["0", "0", "0", "0"]])
    );
}
