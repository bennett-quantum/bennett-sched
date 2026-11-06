// The shared gate tool owns worktree, pin isolation and ref protocol tests.
// This repository checks its own installed stub and declared check contract.
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

#[test]
fn sibling_resolution_uses_pins_even_when_live_checkout_is_empty_or_drifted() {
    let output = Command::new("python3")
        .arg("-I")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/pin_resolution.py"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn hex(value: &Value, width: usize) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() == width
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            && s.bytes().any(|c| c != b'0')
    })
}

#[test]
fn push_gate_declares_pinned_siblings_and_runs_only_the_installed_tool_stub() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = Command::new("python3")
        .args([
            "-I",
            "-c",
            "import json,sys,tomllib; print(json.dumps(tomllib.load(open(sys.argv[1],'rb'))))",
        ])
        .arg(root.join("gate.toml"))
        .output()
        .unwrap();
    assert!(
        config.status.success(),
        "{}",
        String::from_utf8_lossy(&config.stderr)
    );
    let config: Value = serde_json::from_slice(&config.stdout).unwrap();
    assert_eq!(config["schema"], 1);
    assert_eq!(config["repo"], "bennett-sched");
    assert!(hex(&config["tool_sha"], 40));
    let isa = &config["siblings"]["bennett-isa-spec"];
    assert!(hex(&isa["sha"], 40));
    assert!(hex(&isa["files"]["spec/Bennett-ISA-Spec-0.9.md"], 64));
    let builds = isa["build"].as_array().unwrap();
    assert!(builds.iter().any(|b| {
        b["cwd"] == "reference-interpreter"
            && b["argv"] == serde_json::json!(["cargo", "build", "--offline"])
            && b["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p == "reference-interpreter/target/debug/bennett-isa-ref")
    }));
    assert_eq!(
        config["env"]["BENNETT_REF_BIN"],
        "{sibling:bennett-isa-spec}/reference-interpreter/target/debug/bennett-isa-ref"
    );
    let checks = config["check"].as_array().unwrap();
    assert!(
        checks
            .iter()
            .any(|c| c["argv"].as_array().is_some_and(|a| a.len() > 2
                && a[0] == "cargo"
                && a[1] == "test"
                && a.iter().any(|s| s == "--offline")))
    );
    assert!(
        checks
            .iter()
            .any(|c| c["argv"] == serde_json::json!(["cargo", "fmt", "--", "--check"]))
    );
    let path = root.join("scripts/pre-push");
    let stub = fs::read_to_string(&path).unwrap();
    assert_ne!(fs::metadata(path).unwrap().permissions().mode() & 0o111, 0);
    assert!(
        stub.contains("show HEAD:gate.toml"),
        "hook must read a committed tool pin"
    );
    assert!(
        stub.contains("show \"$tool_sha:tools/gate_run.py\""),
        "hook must extract the pinned tool"
    );
    assert!(
        stub.contains("exec python3 -I \"$tool\" run --repo \"$root\" --tool-sha \"$tool_sha\"")
    );
    assert!(stub.contains("BENNETT_GATE_HOOK_PROBE"));
    assert!(
        !stub.contains("cargo test") && !stub.contains("worktree add"),
        "the stub must delegate the gate protocol"
    );
    assert!(
        !stub.contains("../bennett/tools/gate_run.py"),
        "the hook must not run a live sibling tool"
    );
    let checkpoint = fs::read_to_string(root.join("scripts/check-working-tree")).unwrap();
    assert!(checkpoint.contains("cargo test") && checkpoint.contains("cargo fmt"));
    assert!(
        !checkpoint.contains("git "),
        "the runner's optional local checkpoint must stay git-free"
    );
}
