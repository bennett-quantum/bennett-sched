// The shared gate tool owns worktree, pin isolation and ref protocol tests.
// This repository checks its own installed stub and declared check contract.
#[path = "support/pins.rs"]
mod pins;
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

#[test]
fn installed_hook_obeys_committed_pin_and_propagates_tool_failure() {
    let output = Command::new("python3")
        .args(["-B", "tests/gate_stub.py"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn quality_measurement_rejects_each_independent_regression() {
    let output = Command::new("python3")
        .args(["-B", "tests/quality_metrics.py"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

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

#[test]
fn relative_reference_override_cannot_execute_path_decoy() {
    const PROBE: &str = "BENNETT_REFERENCE_PROBE_EXPECTED";
    if let Some(expected) = std::env::var_os(PROBE) {
        let chosen = pins::reference();
        let output = Command::new(&chosen).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"PINNED-REFERENCE\n");
        assert!(chosen.is_absolute(), "{}", chosen.display());
        assert_eq!(chosen, PathBuf::from(expected).canonicalize().unwrap());
        return;
    }

    // Run the resolver in a child to isolate cwd, environment and its OnceLock.
    let tmp = std::env::temp_dir().join(format!("pinned-reference-{}", std::process::id()));
    let layout = tmp.join("layout");
    let checkout = layout.join("bennett-isa-spec");
    fs::create_dir_all(checkout.join(".git/objects")).unwrap();
    fs::create_dir(checkout.join(".git/refs")).unwrap();
    fs::write(checkout.join(".git/HEAD"), format!("{}\n", pins::sha())).unwrap();
    let binary = checkout.join("reference-interpreter/target/debug/bennett-isa-ref");
    fs::create_dir_all(binary.parent().unwrap()).unwrap();
    fs::write(&binary, "#!/bin/sh\nprintf 'PINNED-REFERENCE\\n'\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let decoy = tmp.join("bin/bennett-isa-ref");
    fs::create_dir(decoy.parent().unwrap()).unwrap();
    fs::write(
        &decoy,
        "#!/bin/sh\ntouch \"$0.executed\"\nprintf 'UNPINNED-REFERENCE\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&decoy, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(decoy.parent().unwrap().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "gate::relative_reference_override_cannot_execute_path_decoy",
            "--nocapture",
        ])
        .env(PROBE, &binary)
        .env("BENNETT_GATE_LAYOUT", &layout)
        .env("BENNETT_REF_BIN", "./bennett-isa-ref")
        .env("PATH", path)
        .current_dir(binary.parent().unwrap())
        .output()
        .unwrap();
    let decoy_executed = decoy.with_extension("executed").exists();
    fs::remove_dir_all(tmp).unwrap();
    assert!(!decoy_executed, "PATH decoy was executed");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
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
    let quality = checks
        .iter()
        .filter(|c| c["name"] == "quality baseline")
        .collect::<Vec<_>>();
    assert_eq!(quality.len(), 1);
    assert_eq!(
        quality[0]["argv"],
        serde_json::json!(["python3", "-B", "scripts/quality.py", "--check"])
    );
    assert!(
        checks.iter().any(|c| c["argv"]
            .as_array()
            .is_some_and(|a| a.iter().any(|arg| arg == "--ignored"))),
        "the gate must execute the retained exhaustive tests"
    );
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
