use bennett_sched::{Outcome, Request, canonical};
use serde_json::{Value, json};
use std::collections::BTreeSet;
mod common;

fn signature(mut_widths: &[u64], input_widths: &[u64], bounds: &[u64]) -> Value {
    json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":mut_widths,"in":input_widths,"mem":[],"args":[],"bounds":bounds})
}
fn target(classes: Value, gates: Value) -> Value {
    json!({"ptr_widths_supported":[32,64],"classes":classes,"resources":{"time":{"kind":"time","unit":"second"},"qubit_second":{"kind":"occupancy","unit":"qubit_second"},"peak_qubits":{"kind":"peak","unit":"qubit"}},"gates":gates,"schemes":[],"qram":null,"tables":{},"imports":{},"capabilities":{"measurement":{"mid_circuit":false,"feedforward_latency":0},"mbu":false,"max_parallel":"unbounded","max_predicate_depth":"unbounded","classical_step_time":1},"objective":{"weighted":{"time":1}}})
}
fn gates() -> Value {
    json!(["x","cx","ccx"].map(|op|json!({"op":op,"classes":["*"],"controls":{"min":0,"max":"unbounded"},"epsilon":0,"cost":{"time":1}})))
}
fn request(bytes: &[u8], region: &str, target: &Value) -> Request {
    let mut r = Request::default();
    r.set_object(bytes).unwrap();
    r.set_target(&canonical::to_vec(target).unwrap()).unwrap();
    let hash = r
        .object
        .as_ref()
        .unwrap()
        .closure_hash(region)
        .unwrap()
        .to_owned();
    r.set_region(&hash, "forward", 0).unwrap();
    r.budget = Some([0; 4]);
    r.set_params(b"[]").unwrap();
    r.set_accuracy("0").unwrap();
    r
}

#[test]
fn repeated_identical_scratch_calls_have_complete_distinct_dynamic_frames() {
    let callee = common::action(
        "perm_action",
        "sc",
        signature(&[1], &[], &[]),
        "perm_action @sc (t:wire mut) () () () ()",
        "d = qalloc any 1\ncx t d\ncx t d\nqfree d",
    );
    for repeats in [1, 2, 3] {
        let body = (0..repeats)
            .map(|_| "action_call @sc (t) () () ()")
            .collect::<Vec<_>>()
            .join("\n");
        let bytes = common::caller(
            &callee,
            "perm_action",
            "outer",
            signature(&[1], &[], &[]),
            "perm_action @outer (t:wire mut) () () () ()",
            &body,
        );
        let r = request(
            &bytes,
            "outer",
            &target(
                json!({"fast":{"capacity":2,"qubit_second":1,"dirty_rate":1}}),
                gates(),
            ),
        );
        let response = r.schedule().unwrap();
        assert_eq!(
            response.outcome,
            Outcome::Ok,
            "{repeats} calls: {:?}",
            response.reason
        );
        let c = canonical::require_jcs(&response.certificate).unwrap();
        let hash = r.object.as_ref().unwrap().closure_hash("sc").unwrap();
        let scratch = c["regions"][hash]["frame"]["scratch"].as_array().unwrap();
        assert_eq!(scratch.len(), repeats);
        assert_eq!(
            scratch
                .iter()
                .map(|s| s["id"].as_str().unwrap())
                .collect::<BTreeSet<_>>()
                .len(),
            repeats
        );
        assert!(
            scratch
                .iter()
                .all(|s| s["width"] == 1 && s["class"] == "fast" && s["state"] == "clean")
        );
        let resources = &c["regions"][r.region.as_ref().unwrap()]["resources"];
        assert_eq!(resources["peak_qubits[fast]"], "2");
        assert_eq!(resources["qubit_second[fast]"], (4 * repeats).to_string());
    }
}
