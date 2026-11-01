use std::{path::PathBuf, process::Command};
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
    for include in [root.join("data"), root.join("../bennett-isa-spec/include")] {
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
        let fixtures = root.join("../bennett-isa-spec/conformance/fixtures/controlled-action-09");
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
