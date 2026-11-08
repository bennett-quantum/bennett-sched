#[path = "support/pins.rs"]
mod pins;
use bennett_sched::canonical;
use serde_json::Value;
use std::process::Command;

#[test]
fn published_diagnostic_witnesses_preserve_verdicts_without_requiring_local_code_spelling() {
    let directory = pins::spec("conformance/fixtures/accepted-k84-09");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("k84.json")).unwrap()).unwrap();
    let cases = manifest["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let path = directory.join(case["object"].as_str().unwrap());
        assert_eq!(
            canonical::hash(&std::fs::read(&path).unwrap()),
            case["sha256"]
        );
        let result = Command::new(pins::reference())
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            case["accepted"].as_bool().unwrap(),
            "{}: {}",
            case["name"],
            String::from_utf8_lossy(&result.stderr)
        );
        if result.status.success() {
            assert_eq!(canonical::parse(&result.stdout).unwrap()["status"], "ok");
        } else {
            assert!(
                !result.stderr.is_empty(),
                "rejection needs an obligation diagnostic"
            );
        }
    }
}

#[test]
fn published_token_partition_objects_have_their_exact_metadata_admission_outcomes() {
    let directory = pins::spec("conformance/fixtures/accepted-k91-09");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("k91.json")).unwrap()).unwrap();
    let cases = manifest["fixtures"].as_array().unwrap();
    assert_eq!(cases.len(), 9);
    for case in cases {
        let path = directory.join(case["object"].as_str().unwrap());
        assert_eq!(
            canonical::hash(&std::fs::read(&path).unwrap()),
            case["sha256"]
        );
        let result = Command::new(pins::reference())
            .arg("check")
            .arg(path)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            case["accepted"].as_bool().unwrap(),
            "{}: {}",
            case["name"],
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn published_alias_source_binds_both_partitions_without_changing_source_or_parameter_identity() {
    use bennett_sched::{Outcome, Request, object::Object};
    use serde_json::json;
    let directory = pins::spec("conformance/fixtures/accepted-k91-09");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("k91.json")).unwrap()).unwrap();
    let bytes =
        std::fs::read(directory.join(manifest["source"]["object"].as_str().unwrap())).unwrap();
    let object = Object::parse(&bytes).unwrap();
    assert_eq!(object.object_hash, manifest["object_hash"]);
    let mut hashes = Vec::new();
    for partition in manifest["partitions"].as_array().unwrap() {
        let mut r = Request::default();
        r.set_object(&bytes).unwrap();
        r.set_target(&std::fs::read("data/k83-target.json").unwrap())
            .unwrap();
        r.set_region(object.closure_hash("pair").unwrap(), "forward", 0)
            .unwrap();
        r.set_params(br#"[["1","0","0","0"],["1","0","0","0"]]"#)
            .unwrap();
        r.set_token_aliases(&canonical::to_vec(&partition["token_aliases"]).unwrap())
            .unwrap();
        r.set_accuracy("0").unwrap();
        r.budget = Some([0; 4]);
        let response = r.schedule().unwrap();
        assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
        let schedule = canonical::require_jcs(&response.schedule).unwrap();
        let certificate = canonical::require_jcs(&response.certificate).unwrap();
        assert_eq!(
            schedule["bindings"]["token_aliases"],
            partition["token_aliases"]
        );
        assert_eq!(schedule["bindings"]["params_hash"], manifest["params_hash"]);
        assert_eq!(
            certificate["bindings"]["object_hash"],
            manifest["object_hash"]
        );
        assert_eq!(
            certificate["bindings"]["invocation"]["token_aliases"],
            partition["token_aliases"]
        );
        let cleanups =
            &certificate["regions"][object.closure_hash("pair").unwrap()]["cleanup"]["forward:0"];
        let expected = if partition["name"] == "same" { 1 } else { 0 };
        assert_eq!(cleanups.as_array().unwrap().len(), expected);
        hashes.push(canonical::hash(&response.schedule));
        assert_eq!(
            certificate["bindings"]["invocation"]["args"],
            json!([["1", "0", "0", "0"], ["1", "0", "0", "0"]])
        );
    }
    assert_ne!(hashes[0], hashes[1]);
}
