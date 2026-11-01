use bennett_sched::{canonical, expansion::simulate_basis, object::Object, source::Trace};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
fn oracle() -> PathBuf {
    if let Some(p) = std::env::var_os("BENNETT_REF_BIN") {
        return p.into();
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../bennett-isa-spec/reference-interpreter/target");
    for mode in ["release", "debug"] {
        let p = root.join(mode).join("bennett-isa-ref");
        if p.is_file() {
            return p;
        }
    }
    panic!("reference-interpreter binary missing; set BENNETT_REF_BIN");
}
#[test]
fn every_basis_tuple_of_published_controlled_action_agrees_with_reference_binary() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../bennett-isa-spec/conformance/fixtures/controlled-action-09");
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
