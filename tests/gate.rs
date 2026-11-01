use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
};
#[test]
fn push_gate_uses_the_supplied_sha_and_deduplicates_refs_in_isolated_worktree() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tmp = PathBuf::from(std::env::var_os("TMPDIR").expect("TMPDIR"))
        .join(format!("gate-protocol-{}", std::process::id()));
    let checkout = tmp.join("checkout");
    let bin = tmp.join("bin");
    fs::create_dir_all(checkout.join("scripts")).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::copy(
        root.join("scripts/pre-push"),
        checkout.join("scripts/pre-push"),
    )
    .unwrap();
    let git = bin.join("git");
    fs::write(
        &git,
        r#"#!/usr/bin/env python3
import os,sys
a=sys.argv[1:]
with open(os.environ['GATE_AUDIT'],'a') as f:f.write('git '+repr(a)+'\n')
if a[2:4]==['rev-parse','--verify']:
 print(a[4].removesuffix('^{commit}'))
elif a[2:4]==['worktree','add']:
 assert a[4]=='--detach'
 assert a[6]=='a'*40
elif a[2:4]==['worktree','remove']:pass
else:raise SystemExit('unexpected git protocol '+repr(a))
"#,
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    let cargo = bin.join("cargo");
    fs::write(
        &cargo,
        r#"#!/usr/bin/env python3
import os,sys
assert '/checkout-gate.' in sys.argv[sys.argv.index('--manifest-path')+1]
assert os.environ.get('DISPLAY') is None
assert os.environ['TMPDIR'].endswith('/.tmp/sol-sched/tmp')
with open(os.environ['GATE_AUDIT'],'a') as f:f.write('cargo '+repr(sys.argv[1:])+'\n')
"#,
    )
    .unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    let log = tmp.join("audit");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let mut child = Command::new("bash")
        .arg(checkout.join("scripts/pre-push"))
        .env("PATH", path)
        .env("GATE_AUDIT", &log)
        .env("DISPLAY", ":99")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let sha = "a".repeat(40);
    let zero = "0".repeat(40);
    write!(child.stdin.take().unwrap(),"refs/heads/main {sha} refs/heads/main {zero}\nrefs/heads/other {sha} refs/heads/other {zero}\nrefs/heads/deleted {zero} refs/heads/deleted {sha}\n").unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let log = fs::read_to_string(log).unwrap();
    assert_eq!(log.matches("'worktree', 'add'").count(), 1);
    assert_eq!(log.matches("cargo [").count(), 2);
    assert!(!log.contains("'HEAD'"));
    fs::remove_dir_all(tmp).unwrap();
}
