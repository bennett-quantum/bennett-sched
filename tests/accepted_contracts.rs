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
