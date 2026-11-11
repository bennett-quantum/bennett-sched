use crate::common;
use bennett_sched::{Outcome, Request, canonical, object::Object};
use serde_json::{Value, json};

fn request(classes: Value, topology: Value) -> (Request, String, String) {
    let sig = json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[3],"in":[],"mem":[],"args":[],"bounds":[]});
    let leaf = common::action(
        "perm_action",
        "flip",
        sig.clone(),
        "perm_action @flip (t:wire[3] mut) () () () ()",
        "x t[0]\nx t[1]\nx t[2]",
    );
    let bytes = common::caller(
        &leaf,
        "perm_action",
        "root",
        sig,
        "perm_action @root (t:wire[3] mut) () () () ()",
        "action_call @flip (t) () () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    let root = object.closure_hash("root").unwrap().to_owned();
    let child = object.closure_hash("flip").unwrap().to_owned();
    let mut target: Value =
        serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
    target["classes"] = classes;
    target["topology"] = topology;
    target["gates"] = json!([{"op":"x","classes":["*"],"controls":{"min":0,"max":0},"epsilon":0,"cost":{"time":1}}]);
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    r.set_region(&root, "forward", 0).unwrap();
    r.set_params(b"[]").unwrap();
    r.budget = Some([0; 4]);
    r.set_accuracy("0").unwrap();
    (r, root, child)
}

#[test]
fn heterogeneous_slot_keeps_one_source_record_and_merges_equal_adjacent_classes() {
    let (r, root, child) = request(
        json!({"A":{"capacity":1,"qubit_second":1,"dirty_rate":1},"B":{"capacity":2,"qubit_second":1,"dirty_rate":1}}),
        json!({"vertices":[{"class":"A"},{"class":"B"},{"class":"B"}],"edges":[]}),
    );
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(schedule["interface_vertices"], json!([[0, 1, 2]]));
    let c = canonical::require_jcs(&response.certificate).unwrap();
    for hash in [&root, &child] {
        assert_eq!(
            c["regions"][hash]["frame"]["slots"],
            json!([{"id":"mut:0","role":"mut","width":3,"fragments":[{"offset":0,"width":1,"class":"A"},{"offset":1,"width":2,"class":"B"}]}])
        );
        assert_eq!(c["regions"][hash]["frame"]["scratch"], json!([]));
        assert_eq!(c["regions"][hash]["resources"]["time"], "3");
        for (class, width) in [("A", 1), ("B", 2)] {
            assert_eq!(
                c["regions"][hash]["resources"][format!("peak_qubits[{class}]")],
                width.to_string()
            );
            assert_eq!(
                c["regions"][hash]["resources"][format!("qubit_second[{class}]")],
                (3 * width).to_string()
            );
        }
    }
}

#[test]
fn homogeneous_slots_use_class_and_insufficient_total_capacity_emits_no_documents() {
    for capacity in [2, 3] {
        let (r, root, _) = request(
            json!({"A":{"capacity":capacity,"qubit_second":1,"dirty_rate":1}}),
            json!({"vertices":[{"class":"A"},{"class":"A"},{"class":"A"}],"edges":[]}),
        );
        let response = r.schedule().unwrap();
        if capacity == 2 {
            assert_eq!(response.outcome, Outcome::Infeasible);
            assert!(response.schedule.is_empty() && response.certificate.is_empty());
        } else {
            assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
            let c = canonical::require_jcs(&response.certificate).unwrap();
            assert_eq!(
                c["regions"][root]["frame"]["slots"],
                json!([{"id":"mut:0","role":"mut","width":3,"class":"A"} ])
            );
        }
    }
}

#[test]
fn heterogeneous_gate_binding_is_tried_after_feasible_homogeneous_allocations_fail() {
    let bytes = common::action(
        "perm_action",
        "cross_class",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[2],"in":[],"mem":[],"args":[],"bounds":[]}),
        "perm_action @cross_class (t:wire[2] mut) () () () ()",
        "cx t[0] t[1]",
    );
    let object = Object::parse(&bytes).unwrap();
    let mut target: Value =
        serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
    target["classes"] = json!({"A":{"capacity":2,"qubit_second":1,"dirty_rate":1},"B":{"capacity":2,"qubit_second":1,"dirty_rate":1}});
    target["topology"] = json!({"vertices":[{"class":"A"},{"class":"A"},{"class":"B"},{"class":"B"}],"edges":[[0,2]]});
    target["gates"] = json!([{"op":"cx","classes":["A","B"],"controls":{"min":1,"max":1},"epsilon":0,"cost":{"time":1}}]);
    let mut r = Request::default();
    r.set_object(&bytes).unwrap();
    r.set_target(&canonical::to_vec(&target).unwrap()).unwrap();
    let root = object.closure_hash("cross_class").unwrap();
    r.set_region(root, "forward", 0).unwrap();
    r.set_params(b"[]").unwrap();
    r.budget = Some([0; 4]);
    r.set_accuracy("0").unwrap();
    let response = r.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let schedule = canonical::require_jcs(&response.schedule).unwrap();
    assert_eq!(schedule["interface_vertices"], json!([[0, 2]]));
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_eq!(
        certificate["regions"][root]["frame"]["slots"][0]["fragments"],
        json!([{"offset":0,"width":1,"class":"A"},{"offset":1,"width":1,"class":"B"}])
    );
}
