use crate::common;
use bennett_sched::{Outcome, Request, canonical, object::Object};
use serde_json::{Value, json};

#[test]
fn published_resource_context_witness_preserves_exact_rationals_and_canonical_projection() {
    let sig = |mutables: Vec<u64>, inputs: Vec<u64>| json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":mutables,"in":inputs,"mem":[],"args":[],"bounds":[]});
    let flip = common::action(
        "perm_action",
        "flip",
        sig(vec![1], vec![]),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    let bytes = common::caller(
        &flip,
        "perm_action",
        "root",
        sig(vec![1], vec![1]),
        "perm_action @root (t:wire mut) (c:wire in) () () ()",
        "action_call @flip (t) () () ()\nqpred c\naction_call @flip (t) () () ()\nqunpred c",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut target: Value =
        serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
    target["gates"] = json!([{"op":"x","classes":["*"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":{"op":"/","args":[1,3]}}}]);
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    r.set_region(object.closure_hash("root").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(b"[]").unwrap();
    r.budget = Some([0; 4]);
    r.set_accuracy("0").unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    let root = &certificate["regions"][object.closure_hash("root").unwrap()];
    assert_eq!(
        root["resources"]["time"],
        "0.6666666666666666666666666666666667"
    );
    assert_eq!(
        root["resources"]["qubit_second[A]"],
        "1.333333333333333333333333333333334"
    );
    assert_eq!(
        root["estimate"]["exact_resources"]["time"],
        json!({"numerator":"2","denominator":"3"})
    );
    assert_eq!(root["gates_used"], json!({"x":{"count":2}}));
    let child = &certificate["regions"][object.closure_hash("flip").unwrap()];
    let contexts = child["resource_contexts"].as_array().unwrap();
    let frames = child["frame_contexts"].as_array().unwrap();
    assert_eq!(contexts.len(), 2);
    assert_eq!(frames.len(), 2);
    assert!(child.get("frame").is_none());
    for (index, context) in contexts.iter().enumerate() {
        let key = json!([{"symbol":object.symbols["root"].interface_hash,"point":format!("entry:{}",index*2),"iteration":[],"instance":[],"call_index":0}]);
        assert_eq!(context["context_key"], key);
        assert_eq!(frames[index]["context_key"], key);
        assert_eq!(
            context["resources"]["time"],
            "0.3333333333333333333333333333333334"
        );
        assert_eq!(
            context["estimate"]["exact_resources"]["time"],
            json!({"numerator":"1","denominator":"3"})
        );
        assert_eq!(
            context["resources"]["peak_qubits[A]"],
            (index + 1).to_string()
        );
        assert_eq!(context["gates_used"], json!({"x":{"count":1}}));
        assert_eq!(frames[index]["frame"]["slots"].as_array().unwrap().len(), 1);
    }
    assert_eq!(child["resources"], contexts[0]["resources"]);
    assert_eq!(child["gates_used"], contexts[0]["gates_used"]);
    assert_eq!(child["estimate"], contexts[0]["estimate"]);
}

#[test]
fn common_error_bounds_cover_every_context_independently_of_call_order() {
    let sig = |mutables: Vec<u64>| json!({"abi":"0.9","tier":"q","kind":"action","mut":mutables,"in":[],"mem":[],"args":[],"bounds":[]});
    let flip = common::action(
        "action",
        "flip",
        sig(vec![1]),
        "action @flip (t:wire mut) () () () ()",
        "x t",
    );
    for body in [
        "action_call @flip (t) () () ()\naction_call @flip (u) () () ()",
        "action_call @flip (u) () () ()\naction_call @flip (t) () () ()",
    ] {
        let bytes = common::caller(
            &flip,
            "action",
            "root",
            sig(vec![1, 1]),
            "action @root (t:wire mut, u:wire mut) () () () ()",
            body,
        );
        let object = Object::parse(&bytes).unwrap();
        let mut target: Value =
            serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
        target["classes"] = json!({"A":{"capacity":1,"qubit_second":1,"dirty_rate":1},"B":{"capacity":1,"qubit_second":1,"dirty_rate":1}});
        target["gates"] = json!([{"op":"x","classes":["A"],"controls":{"min":0,"max":0},"epsilon":0.01,"cost":{"time":1}},{"op":"x","classes":["B"],"controls":{"min":0,"max":0},"epsilon":0.02,"cost":{"time":1}}]);
        let mut r = Request::default();
        r.set_object(&bytes).unwrap();
        r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
        r.set_region(object.closure_hash("root").unwrap(), "forward", 0)
            .unwrap();
        r.set_params(b"[]").unwrap();
        r.budget = Some([0; 4]);
        r.set_accuracy("1").unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c = canonical::require_jcs(&response.certificate).unwrap();
        let child = &c["regions"][object.closure_hash("flip").unwrap()];
        assert_eq!(
            child["error"]["forward:0"],
            json!({"local":{"operator_norm":"0.02"},"inclusive":{"operator_norm":"0.02"}})
        );
        for claim in child["claims"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["kind"] == "numerical")
        {
            assert_eq!(claim["bound"], "0.02");
        }
        assert_eq!(c["composition"]["epsilon_total"]["diamond"], "0.08");
        assert_eq!(
            c["regions"][object.closure_hash("root").unwrap()]["error"]["forward:0"]["inclusive"]["operator_norm"],
            "0.04"
        );
        r.set_accuracy("0.03").unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Infeasible);
        assert!(response.schedule.is_empty() && response.certificate.is_empty());
    }
}

#[test]
fn requested_accuracy_is_compared_exactly_and_error_displays_keep_sufficient_precision() {
    let bytes = common::action(
        "action",
        "a",
        json!({"abi":"0.9","tier":"q","kind":"action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]}),
        "action @a (t:wire mut) () () () ()",
        "x t",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut target: Value =
        serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
    target["gates"] = json!([{"op":"x","classes":["*"],"controls":{"min":0,"max":0},"epsilon":{"op":"/","args":[1,3]},"cost":{"time":1}}]);
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    r.set_region(object.closure_hash("a").unwrap(), "forward", 0)
        .unwrap();
    r.set_params(b"[]").unwrap();
    r.budget = Some([0; 4]);
    let epsilon = "0.33333333333333333333333333333333333333333333333333333333333333334";
    r.set_accuracy(epsilon).unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let c = canonical::require_jcs(&response.certificate).unwrap();
    let local = &c["regions"][object.closure_hash("a").unwrap()]["error"]["forward:0"]["local"]["operator_norm"];
    let bound = bennett_sched::decimal::value(local).unwrap();
    assert!(bound >= bennett_sched::decimal::integer(1) / bennett_sched::decimal::integer(3));
    assert!(bound <= bennett_sched::decimal::parse(epsilon).unwrap());
    let total =
        bennett_sched::decimal::value(&c["composition"]["epsilon_total"]["diamond"]).unwrap();
    assert_eq!(total, bennett_sched::decimal::integer(2) * bound);
}

#[test]
fn selected_roots_keep_shared_callee_frames_and_inventories_bound_to_their_own_target() {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    let leaf = common::action(
        "perm_action",
        "flip",
        sig.clone(),
        "perm_action @flip (t:wire mut) () () () ()",
        "x t",
    );
    let a = common::caller(
        &leaf,
        "perm_action",
        "root_a",
        sig.clone(),
        "perm_action @root_a (t:wire mut) () () () ()",
        "action_call @flip (t) () () ()",
    );
    let bytes = common::caller(
        &a,
        "perm_action",
        "root_b",
        sig,
        "perm_action @root_b (t:wire mut) () () () ()",
        "action_call @flip (t) () () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let callee = object.closure_hash("flip").unwrap();
    for (root, other, class) in [("root_a", "root_b", "A"), ("root_b", "root_a", "B")] {
        let mut target: Value =
            serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
        target["classes"] = json!({"A":{"capacity":1,"qubit_second":1,"dirty_rate":1},"B":{"capacity":1,"qubit_second":1,"dirty_rate":1}});
        target["topology"] = json!({"vertices":[{"class":"A"},{"class":"B"}],"edges":[]});
        target["gates"] = json!([{"op":"x","classes":[class],"controls":{"min":0,"max":0},"epsilon":0,"cost":{"time":1}}]);
        let mut r = Request::default();
        r.set_object(&bytes).unwrap();
        r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
        r.set_region(object.closure_hash(root).unwrap(), "forward", 0)
            .unwrap();
        r.set_params(b"[]").unwrap();
        r.budget = Some([0; 4]);
        r.set_accuracy("0").unwrap();
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let c = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(
            c["bindings"]["closure_hash"],
            object.closure_hash(root).unwrap()
        );
        assert_eq!(c["bindings"]["object_hash"], object.object_hash);
        assert_eq!(c["regions"].as_object().unwrap().len(), 2);
        assert!(
            c["regions"]
                .get(object.closure_hash(other).unwrap())
                .is_none()
        );
        assert_eq!(
            c["regions"][callee]["frame"]["slots"],
            json!([{"id":"mut:0","role":"mut","class":class,"width":1}])
        );
        let root_record = &c["regions"][object.closure_hash(root).unwrap()];
        assert!(root_record["frame"].is_object());
        if let Some(contexts) = root_record.get("resource_contexts") {
            assert_eq!(contexts.as_array().unwrap().len(), 1);
            assert_eq!(contexts[0]["context_key"], json!([]));
        }
    }
}
