#[path = "support/pins.rs"]
mod pins;
use std::{path::PathBuf, process::Command};
mod common;
#[test]
fn c11_consumer_links_the_cdylib_across_the_binary_edge() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(
        deps.join("libbennett_sched.so").is_file(),
        "cargo must build the cdylib"
    );
    let tmp = PathBuf::from(std::env::var_os("TMPDIR").expect("headless gate must set TMPDIR"))
        .join(format!("c-abi-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let binary = tmp.join("consumer");
    for include in [root.join("data"), pins::spec("include")] {
        let result = Command::new("cc")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .arg(root.join("tests/c_abi.c"))
            .arg("-I")
            .arg(&include)
            .arg("-L")
            .arg(&deps)
            .arg(format!("-Wl,-rpath,{}", deps.display()))
            .arg("-lbennett_sched")
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            include.display(),
            String::from_utf8_lossy(&result.stderr)
        );
        let fixtures = pins::spec("conformance/fixtures/controlled-action-09");
        let run = Command::new("timeout")
            .arg("30")
            .arg(&binary)
            .arg(fixtures.join("controlled.bisa"))
            .arg(fixtures.join("target.json"))
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(String::from_utf8_lossy(&run.stdout).contains("C ABI:"));
    }
    std::fs::remove_dir_all(tmp).unwrap();
}

#[test]
fn c11_alias_consumer_uses_the_additive_setter_and_keeps_partitions() {
    use bennett_sched::object::Object;
    use serde_json::json;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let tmp = std::env::temp_dir().join(format!("c-alias-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let bytes = common::action(
        "oracle",
        "identity",
        json!({"abi":"0.9","tier":"q","kind":"oracle","in":[1],"tgt":1,"fault":4,"mem":[],"args":[]}),
        "oracle @identity (x:wire in) (t:wire) (f:wire[4]) () ()",
        "lift.retval x",
    );
    let bytes = common::caller(
        &bytes,
        "perm_action",
        "pair",
        json!({"abi":"0.9","tier":"q","kind":"perm_action","mut":[1,4],"in":[1],"mem":[],"args":["token","token"],"bounds":[]}),
        "perm_action @pair (t:wire mut, f:wire[4] mut) (x:wire in) () (a:token, b:token) ()",
        "oracle_call @identity a (x) t f () ()\noracle_call @identity b (x) t f () ()",
    );
    let object = Object::parse(&bytes).unwrap();
    std::fs::write(tmp.join("pair.bisa"), bytes).unwrap();
    let binary = tmp.join("consumer");
    let result = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(root.join("tests/c_token_aliases.c"))
        .arg("-I")
        .arg(root.join("data"))
        .arg("-L")
        .arg(&deps)
        .arg(format!("-Wl,-rpath,{}", deps.display()))
        .arg("-lbennett_sched")
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let run = Command::new("timeout")
        .arg("30")
        .arg(binary)
        .arg(tmp.join("pair.bisa"))
        .arg(root.join("data/k83-target.json"))
        .arg(object.closure_hash("pair").unwrap())
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("both identities verified"));
    std::fs::remove_dir_all(tmp).unwrap();
}
