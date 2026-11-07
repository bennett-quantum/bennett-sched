// Python owns resolution so scripts and tests obey exactly the same policy.
#![allow(dead_code)]
use serde_json::Value;
use std::{path::PathBuf, process::Command, sync::OnceLock};

fn resolve(reference: bool) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new("python3");
    command
        .arg("-I")
        .arg(root.join("scripts/pinned_sibling.py"))
        .arg(&root)
        .arg("bennett-isa-spec");
    if reference {
        command.arg("--reference");
    }
    let output = command.output().expect("run the pinned sibling resolver");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("pin resolver JSON")
}

fn pin() -> &'static Value {
    static PIN: OnceLock<Value> = OnceLock::new();
    PIN.get_or_init(|| resolve(false))
}

pub fn spec(path: &str) -> PathBuf {
    PathBuf::from(pin()["path"].as_str().unwrap()).join(path)
}

pub fn sha() -> &'static str {
    pin()["sha"].as_str().unwrap()
}

pub fn reference() -> PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let binary = PathBuf::from(resolve(true)["binary"].as_str().unwrap());
            assert!(
                binary.is_absolute(),
                "pin resolver returned a relative reference binary: {}",
                binary.display()
            );
            binary
        })
        .clone()
}
