#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{Status, canonical, object::Object, schedule::Document};
use serde_json::{Value, json};

fn controlled() -> (Object, Value) {
    let root = pins::spec("conformance/fixtures/controlled-action-09");
    // Explicit K91 rebinding of the pre-K91 token-free schedule template.
    let mut schedule =
        canonical::parse(&std::fs::read(root.join("schedule.json")).unwrap()).unwrap();
    schedule["bindings"]["token_aliases"] = json!([]);
    (
        Object::parse(&std::fs::read(root.join("controlled.bisa")).unwrap()).unwrap(),
        schedule,
    )
}

#[test]
fn every_path_is_checked_against_the_source_call_chain_and_loop_depth() {
    let (object, schedule) = controlled();
    Document::parse(schedule.clone())
        .unwrap()
        .validate_paths(&object)
        .unwrap();
    for malformed in 0..5 {
        let mut v = schedule.clone();
        match malformed {
            0 => v["entries"][0]["path"][1]["point"] = json!("entry:100"),
            1 => {
                v["entries"][0]["path"][0]["symbol"] = v["entries"][0]["path"][1]["symbol"].clone()
            }
            2 => v["entries"][0]["path"][1]["iteration"] = json!([0]),
            3 => v["entries"][0]["path"][0]["call_index"] = json!(1),
            _ => v["entries"][0]["path"][1]["instance"] = json!([0]),
        };
        let e = Document::parse(v)
            .unwrap()
            .validate_paths(&object)
            .unwrap_err();
        assert_eq!(e.status, Status::ScheduleInfeasible, "{malformed}: {e}");
    }
}

#[test]
fn published_nonexistent_inc8_row_rejects_and_reserved_oracle_points_are_valid() {
    let spec = std::fs::read_to_string(pins::spec("spec/Bennett-ISA-Spec-0.9.md")).unwrap();
    let bytes = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("```")
        .nth(1)
        .unwrap()
        .trim_start_matches('\n');
    let object = Object::parse(bytes.as_bytes()).unwrap();
    let (_, mut schedule) = controlled();
    let hash = object.closure_hash("inc8").unwrap();
    schedule["bindings"]["closure_hash"] = json!(hash);
    schedule["bindings"]["region"] = json!("inc8");
    schedule["entries"] = json!([{"path":[{"symbol":hash,"point":"entry:3","iteration":[]}],"step":"*","phase":"*","generated":"*","kind":"start","start":{"time":0}}]);
    let case: Value = canonical::parse(
        &std::fs::read(pins::spec(
            "conformance/cases/H/H-path-not-in-object/case.json",
        ))
        .unwrap(),
    )
    .unwrap();
    schedule["entries"][0]["path"][0]["point"] = case["scenario"]["point"].clone();
    assert_eq!(
        Document::parse(schedule.clone())
            .unwrap()
            .validate_paths(&object)
            .unwrap_err()
            .status,
        Status::ScheduleInfeasible
    );
    for point in ["entry:0", "$construct", "$step", "$commit", "$release"] {
        schedule["entries"][0]["path"][0]["point"] = json!(point);
        Document::parse(schedule.clone())
            .unwrap()
            .validate_paths(&object)
            .unwrap();
    }
    let (action, mut a) = controlled();
    for point in ["$construct", "$release", "$step", "$commit"] {
        a["entries"][0]["path"] =
            json!([{"symbol":a["bindings"]["closure_hash"],"point":point,"iteration":[]}]);
        let result = Document::parse(a.clone()).unwrap().validate_paths(&action);
        // K83 admits action entry/exit support even when its domain is empty.
        assert_eq!(result.is_ok(), ["$construct", "$release"].contains(&point));
    }
}

#[test]
fn concrete_lproc_instance_selectors_use_the_finite_source_binding_closure() {
    let spec = std::fs::read_to_string(pins::spec("spec/Bennett-ISA-Spec-0.9.md")).unwrap();
    let text = spec
        .split("### 16.3 Example objects")
        .nth(1)
        .unwrap()
        .split("## 17.")
        .next()
        .unwrap();
    let source = text
        .split("```")
        .skip(1)
        .step_by(2)
        .map(|s| s.trim_start_matches('\n'))
        .filter(|s| s.starts_with("bennett-object "))
        .nth(9)
        .unwrap();
    let object = Object::parse(source.as_bytes()).unwrap();
    let (_, mut value) = controlled();
    value["bindings"]["closure_hash"] = json!(object.closure_hash("through").unwrap());
    value["bindings"]["region"] = json!("through");
    value["entries"] = json!([{"path":[{"symbol":object.symbols["worker"].interface_hash,"point":"entry:0","iteration":[],"instance":[0,1,0]}],"step":"*","phase":"*","generated":0,"kind":"gate","gate":{"realisation":"native"}}]);
    Document::parse(value.clone())
        .unwrap()
        .validate_paths(&object)
        .unwrap();
    for key in [json!([]), json!([0, 0, 0]), json!([0, 2, 0])] {
        let mut malformed = value.clone();
        malformed["entries"][0]["path"][0]["instance"] = key;
        let error = Document::parse(malformed)
            .unwrap()
            .validate_paths(&object)
            .unwrap_err();
        assert_eq!(error.status, Status::ScheduleInfeasible);
        assert!(error.reason.contains("instance"), "{error}");
    }
}
