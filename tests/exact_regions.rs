use crate::common;
use bennett_sched::{Outcome, Request, canonical, object::Object};
use serde_json::{Value, json};

fn source(kind: &str, nested: bool) -> Vec<u8> {
    if kind == "oracle" {
        return common::action(
            "oracle",
            "r",
            json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
            "oracle @r (x:wire in) (t:wire) (f:wire[4]) () ()",
            "lift.retval x",
        );
    }
    let sig = |kind: &str| json!({"abi":"0.9","tier":"q","kind":kind,"mut":[1],"in":[],"mem":[],"args":[],"bounds":[]});
    if nested {
        let base = common::action(
            "action",
            "child",
            sig("action"),
            "action @child (t:wire mut) () () () ()",
            "x t",
        );
        common::caller(
            &base,
            "perm_action",
            "r",
            sig("perm_action"),
            "perm_action @r (t:wire mut) () () () ()",
            "action_call @child (t) () () ()",
        )
    } else {
        common::action(
            kind,
            "r",
            sig(kind),
            &format!("{kind} @r (t:wire mut) () () () ()"),
            "x t",
        )
    }
}
fn run(kind: &str, nested: bool, exact_available: bool) -> (bennett_sched::Response, Object) {
    let bytes = source(kind, nested);
    let object = Object::parse(&bytes).unwrap();
    let mut target: Value =
        serde_json::from_slice(&std::fs::read("data/k83-target.json").unwrap()).unwrap();
    let entries = target["gates"].as_array_mut().unwrap();
    let index = entries.iter().position(|e| e["op"] == "x").unwrap();
    let mut approximate = entries[index].clone();
    approximate["epsilon"] = json!(0.001);
    if exact_available {
        entries[index]["cost"]["time"] = json!(2);
        entries.insert(index, approximate);
    } else {
        entries[index] = approximate;
    }
    let mut request = Request::default();
    request.set_object(&bytes).unwrap();
    request
        .set_target(&canonical::to_vec(&target).unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("r").unwrap(), "forward", 0)
        .unwrap();
    request.set_params(b"[]").unwrap();
    request.budget = Some([0; 4]);
    request.set_accuracy("1").unwrap();
    (request.schedule().unwrap(), object)
}
#[test]
fn exact_regions_match_exact_entries_even_after_approximate_entries_and_propagate_to_descendants() {
    for (kind, nested) in [
        ("oracle", false),
        ("perm_action", false),
        ("perm_action", true),
    ] {
        let (response, object) = run(kind, nested, true);
        assert_eq!(
            response.outcome,
            Outcome::Ok,
            "{kind} nested={nested}: {:?}",
            response.reason
        );
        let certificate = canonical::require_jcs(&response.certificate).unwrap();
        for name in if nested {
            vec!["r", "child"]
        } else {
            vec!["r"]
        } {
            let region = &certificate["regions"][object.closure_hash(name).unwrap()];
            for error in region["error"].as_object().unwrap().values() {
                assert_eq!(
                    error["local"]["operator_norm"], "0",
                    "{kind} nested={nested}"
                );
                assert_eq!(
                    error["inclusive"]["operator_norm"], "0",
                    "{kind} nested={nested}"
                );
            }
        }
    }
}
#[test]
fn approximate_only_entries_cannot_realise_exact_regions_but_remain_valid_for_actions() {
    for (kind, nested) in [
        ("oracle", false),
        ("perm_action", false),
        ("perm_action", true),
    ] {
        let (response, _) = run(kind, nested, false);
        assert_eq!(
            response.outcome,
            Outcome::Infeasible,
            "{kind} nested={nested}: {:?}",
            response.reason
        );
        assert!(response.schedule.is_empty() && response.certificate.is_empty());
    }
    let (response, object) = run("action", false, false);
    assert_eq!(response.outcome, Outcome::Ok);
    let certificate = canonical::require_jcs(&response.certificate).unwrap();
    assert_ne!(
        certificate["regions"][object.closure_hash("r").unwrap()]["error"]["forward:0"]["inclusive"]
            ["operator_norm"],
        "0"
    );
}
