#[path = "support/pins.rs"]
mod pins;
use bennett_sched::canonical;
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};
#[test]
fn conformance_inventory_records_every_case_and_portable_manifest_without_stale_hashes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = pins::spec("");
    let report: Value =
        serde_json::from_slice(&std::fs::read(root.join("docs/conformance-report.json")).unwrap())
            .unwrap();
    assert_eq!(
        report["isa_spec_sha"],
        pins::sha(),
        "inventory must record the gate.toml ISA pin"
    );
    assert_eq!(
        report["spec_sha256"],
        canonical::hash(&std::fs::read(spec.join("spec/Bennett-ISA-Spec-0.9.md")).unwrap())
    );
    let mut actual = BTreeSet::new();
    for suite in std::fs::read_dir(spec.join("conformance/cases")).unwrap() {
        let suite = suite.unwrap().path();
        if !suite.is_dir() {
            continue;
        }
        for case in std::fs::read_dir(suite).unwrap() {
            let path = case.unwrap().path().join("case.json");
            if path.is_file() {
                actual.insert(
                    path.strip_prefix(&spec)
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    let mut recorded = BTreeSet::new();
    for case in report["cases"].as_array().unwrap() {
        let path = case["source"].as_str().unwrap();
        assert!(recorded.insert(path.to_owned()));
        assert_eq!(
            case["sha256"],
            canonical::hash(&std::fs::read(spec.join(path)).unwrap()),
            "{path}"
        );
        assert!(case["coverage"].is_string());
    }
    assert_eq!(actual, recorded);
    assert_eq!(
        report["case_count"].as_u64().unwrap() as usize,
        actual.len()
    );
    assert_eq!(report["normative_objects"].as_array().unwrap().len(), 10);
    assert_eq!(report["normative_scheduler_audit"]["regions"], 13);
    assert_eq!(report["normative_scheduler_audit"]["outcomes"]["OK"], 7);
    assert_eq!(
        report["normative_scheduler_audit"]["outcomes"]["UNSUPPORTED"],
        5
    );
    assert_eq!(report["normative_scheduler_audit"]["outcomes"]["ERROR"], 1);
    let mut actual_manifests = BTreeSet::new();
    for directory in std::fs::read_dir(spec.join("conformance/fixtures")).unwrap() {
        let directory = directory.unwrap();
        if !directory
            .file_name()
            .to_string_lossy()
            .starts_with("accepted-")
            || !directory.path().is_dir()
        {
            continue;
        }
        for file in std::fs::read_dir(directory.path()).unwrap() {
            let path = file.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                actual_manifests.insert(
                    path.strip_prefix(&spec)
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }
    let mut recorded_manifests = BTreeSet::new();
    for manifest in report["portable_manifests"].as_array().unwrap() {
        let path = manifest["path"].as_str().unwrap();
        assert!(recorded_manifests.insert(path.to_owned()));
        assert_eq!(
            manifest["sha256"],
            canonical::hash(&std::fs::read(spec.join(path)).unwrap())
        );
        match path.rsplit('/').next().unwrap() {
            "k82.json" => {
                assert_eq!(manifest["coverage"], "direct-fixture-test");
                assert_eq!(manifest["vectors"], 16);
                assert_eq!(manifest["edge_digests"], 86);
            }
            "k79.json" => {
                assert_eq!(
                    manifest["coverage"],
                    "source-admission-and-complete-basis-tuples"
                );
                assert_eq!(manifest["admitted"], 4);
                assert_eq!(manifest["rejected"], 8);
            }
            "k83.json" => {
                assert_eq!(
                    manifest["coverage"],
                    "source-traces-and-rebound-schedule-admission"
                );
                assert_eq!(manifest["source_cases"], 6);
                assert_eq!(manifest["rebound_accepted"], 2);
                assert_eq!(manifest["rebound_rejected"], 6);
                assert_eq!(manifest["original_missing_expansion"], 8);
            }
            "k80.json" => {
                assert_eq!(
                    manifest["coverage"],
                    "version-rebound-source-traces-and-complete-basis-tuples"
                );
                assert_eq!(manifest["source_cases"], 5);
                assert_eq!(manifest["basis_tuples"], 80);
            }
            "k84.json" => {
                assert_eq!(manifest["coverage"], "reference-metadata-admission");
                assert_eq!(manifest["admitted"], 1);
                assert_eq!(manifest["rejected"], 2);
            }
            "k91.json" => {
                assert_eq!(
                    manifest["coverage"],
                    "reference-metadata-and-source-scheduling"
                );
                assert_eq!(manifest["metadata_cases"], 9);
                assert_eq!(manifest["source_partitions"], 2);
            }
            "k81.json" => {
                assert_eq!(manifest["coverage"], "partial-equivalent-witness")
            }
            _ => assert_eq!(manifest["coverage"], "not-exercised"),
        }
    }
    assert_eq!(
        actual_manifests, recorded_manifests,
        "every published portable manifest must be inventoried"
    );
}
