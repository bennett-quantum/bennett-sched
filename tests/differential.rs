#[path = "support/pins.rs"]
mod pins;
use bennett_sched::{canonical, expansion::simulate_basis, object::Object, source::Trace};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, process::Command};
fn oracle() -> PathBuf {
    pins::reference()
}

#[test]
fn emitted_09_schedule_and_certificate_pass_reference_metadata_admission() {
    use bennett_sched::{Outcome, Request};
    let fixtures = pins::spec("conformance/fixtures/controlled-action-09");
    let source = std::fs::read_to_string(fixtures.join("controlled.bisa")).unwrap();
    let object = Object::parse(source.as_bytes()).unwrap();
    let mut request = Request::default();
    request.set_object(source.as_bytes()).unwrap();
    request
        .set_target(&std::fs::read(fixtures.join("target.json")).unwrap())
        .unwrap();
    request
        .set_region(object.closure_hash("controlled").unwrap(), "forward", 0)
        .unwrap();
    request.budget = Some([0; 4]);
    request.set_params(b"[]").unwrap();
    request.set_accuracy("0").unwrap();
    let response = request.schedule().unwrap();
    assert_eq!(response.outcome, Outcome::Ok, "{:?}", response.reason);
    let prefix = source.split("\nsection schedule\n").next().unwrap();
    let path = std::env::temp_dir().join(format!("sched-reference-{}.bisa", std::process::id()));
    let check = |schedule: &serde_json::Value| {
        std::fs::write(
            &path,
            format!(
                "{prefix}\nsection schedule\n{}\n\nsection certificate\n{}\n",
                String::from_utf8(canonical::to_vec(schedule).unwrap()).unwrap(),
                std::str::from_utf8(&response.certificate).unwrap()
            ),
        )
        .unwrap();
        Command::new("timeout")
            .arg("30")
            .arg(oracle())
            .arg("check")
            .arg(&path)
            .output()
            .unwrap()
    };
    let mut schedule = canonical::require_jcs(&response.schedule).unwrap();
    let output = check(&schedule);
    assert!(
        output.status.success(),
        "reference admission: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = canonical::parse(&output.stdout).unwrap();
    assert_eq!(result["object_hash"], object.object_hash);
    assert_eq!(result["status"], "ok");
    schedule["entries"] = json!([{"path":[{"symbol":object.symbols["controlled"].interface_hash,"point":"entry:99","iteration":[],"instance":[]}],"step":"*","phase":"*","generated":0,"kind":"gate","gate":{"realisation":"native","epsilon_share":0}}]);
    let output = check(&schedule);
    assert!(
        !output.status.success(),
        "reference admitted an absent source point"
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn every_basis_tuple_of_published_controlled_action_agrees_with_reference_binary() {
    let fixtures = pins::spec("conformance/fixtures/controlled-action-09");
    let object = Object::parse(&std::fs::read(fixtures.join("controlled.bisa")).unwrap()).unwrap();
    let trace = Trace::build(&object, "controlled", 0, &json!([]), [0; 4]).unwrap();
    let leaves = trace
        .events
        .iter()
        .map(|e| e.leaf.clone())
        .collect::<Vec<_>>();
    for t in 0..2 {
        for c in 0..2 {
            let output = Command::new("timeout")
                .arg("20")
                .arg(oracle())
                .arg("run-action")
                .arg(fixtures.join("compat-0.8.bisa"))
                .args([
                    "controlled",
                    "--mut",
                    &t.to_string(),
                    "--in",
                    &c.to_string(),
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let expected = canonical::parse(&output.stdout).unwrap();
            let mut bits =
                BTreeMap::from([("mut:0[0]".into(), t != 0), ("in:0[0]".into(), c != 0)]);
            simulate_basis(&leaves, &mut bits).unwrap();
            assert_eq!(json!(u8::from(bits["mut:0[0]"])), expected["mut"][0]);
            assert_eq!(
                expected["state"][format!("{}{}", u8::from(bits["mut:0[0]"]), c)],
                json!([1, 0])
            );
        }
    }
}
