"""Exercise pin resolution with an empty or drifted live sibling."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
NAME = "bennett-isa-spec"
PIN = tomllib.loads((ROOT / "gate.toml").read_text())["siblings"][NAME]["sha"]


def materialised_spec():
    # Gate run worktrees have only their supplied layout, not a local cache.
    if "BENNETT_GATE_LAYOUT" in os.environ:
        return Path(os.environ["BENNETT_GATE_LAYOUT"]) / NAME
    return ROOT / ".tmp/gate/cache" / f"{NAME}@{PIN[:12]}" / NAME


class PinResolution(unittest.TestCase):
    def test_rust_readers_have_no_live_sibling_paths(self):
        readers = [p.name for p in (ROOT / "tests").glob("*.rs")
                   if p.name != "gate.rs" and "../bennett-isa-spec" in p.read_text()]
        self.assertEqual(readers, [], f"readers bypass the pin resolver: {readers}")

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.workspace = Path(self.tmp.name)
        self.root = self.workspace / "bennett-sched"
        self.root.mkdir()
        shutil.copyfile(ROOT / "gate.toml", self.root / "gate.toml")
        (self.workspace / NAME).mkdir()  # Never usable as a fixture source.
        self.cache = self.root / ".tmp/gate/cache" / f"{NAME}@{PIN[:12]}" / NAME
        self.env = {k: v for k, v in os.environ.items()
                    if k not in {"BENNETT_GATE_LAYOUT", "BENNETT_REF_BIN"}}
        self.env["PYTHONDONTWRITEBYTECODE"] = "1"

    def checkout(self, path, sha=PIN):
        (path / ".git/objects").mkdir(parents=True)
        (path / ".git/refs").mkdir()
        (path / ".git/HEAD").write_text(sha + "\n")
        return path

    def resolve(self, *extra):
        return subprocess.run(
            ["python3", "-I", str(ROOT / "scripts/pinned_sibling.py"),
             str(self.root), NAME, *extra], env=self.env,
            capture_output=True, text=True, timeout=30)

    def diagnostic(self, result, actual="unavailable"):
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertEqual(len(result.stderr.splitlines()), 1, result.stderr)
        for text in [NAME, PIN, f"live ../{NAME} HEAD={actual}",
                     "gate_run.py verify --repo bennett-sched --sha HEAD"]:
            self.assertIn(text, result.stderr)

    def test_cache_works_with_empty_live_sibling(self):
        self.checkout(self.cache)
        result = self.resolve()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {"path": str(self.cache), "sha": PIN})

    def test_layout_takes_precedence_over_mismatched_cache(self):
        self.checkout(self.cache, "a" * 40)
        layout = self.workspace / "layout"
        self.checkout(layout / NAME)
        self.env["BENNETT_GATE_LAYOUT"] = str(layout)
        result = self.resolve()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["path"], str(layout / NAME))

    def test_missing_pin_names_live_head_without_using_live_fixtures(self):
        self.checkout(self.workspace / NAME, "b" * 40)
        self.diagnostic(self.resolve(), "b" * 40)

    def test_cache_head_mismatch_is_loud(self):
        self.checkout(self.cache, "a" * 40)
        result = self.resolve()
        self.diagnostic(result)
        self.assertIn("a" * 40, result.stderr)

    def test_layout_mismatch_does_not_fall_back_to_valid_cache(self):
        self.checkout(self.cache)
        layout = self.workspace / "layout"
        self.checkout(layout / NAME, "c" * 40)
        self.env["BENNETT_GATE_LAYOUT"] = str(layout)
        result = self.resolve()
        self.diagnostic(result)
        self.assertIn("c" * 40, result.stderr)

    def test_missing_layout_does_not_fall_back_to_cache(self):
        self.checkout(self.cache)
        self.env["BENNETT_GATE_LAYOUT"] = str(self.workspace / "absent")
        self.diagnostic(self.resolve())

    def test_reference_binary_comes_from_pin(self):
        self.checkout(self.cache)
        binary = self.cache / "reference-interpreter/target/debug/bennett-isa-ref"
        binary.parent.mkdir(parents=True)
        binary.write_text("pinned binary")
        result = self.resolve("--reference")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["binary"], str(binary))
        self.env["BENNETT_REF_BIN"] = str(binary)
        self.assertEqual(self.resolve("--reference").returncode, 0)
        self.env["BENNETT_REF_BIN"] = str(self.workspace / NAME / "live-binary")
        self.diagnostic(self.resolve("--reference"))

    def test_missing_reference_binary_is_loud(self):
        self.checkout(self.cache)
        self.diagnostic(self.resolve("--reference"))

    def test_reference_symlink_cannot_escape_pin(self):
        self.checkout(self.cache)
        live = self.workspace / NAME / "live-binary"
        live.write_text("live binary must not run")
        binary = self.cache / "reference-interpreter/target/debug/bennett-isa-ref"
        binary.parent.mkdir(parents=True)
        binary.symlink_to(live)
        self.diagnostic(self.resolve("--reference"))

    def test_worktree_gitdir_file_and_branch_head_are_checked(self):
        self.checkout(self.cache)
        gitdir = self.cache / ".git"
        (gitdir / "HEAD").write_text("ref: refs/heads/pinned\n")
        (gitdir / "refs/heads").mkdir()
        (gitdir / "refs/heads/pinned").write_text(PIN + "\n")
        moved = self.workspace / "gitdir"
        gitdir.rename(moved)
        gitdir.write_text(f"gitdir: {moved}\n")
        self.env["GIT_DIR"] = str(self.workspace / "wrong-git-dir")
        result = self.resolve()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["sha"], PIN)

    def test_inventory_script_uses_pin_with_empty_live_sibling(self):
        source = materialised_spec()
        self.cache.parent.mkdir(parents=True)
        self.cache.symlink_to(source.resolve(), target_is_directory=True)
        shutil.copytree(ROOT / "scripts", self.root / "scripts")
        (self.root / "data").symlink_to(ROOT / "data", target_is_directory=True)
        (self.root / "docs").mkdir()
        result = subprocess.run(
            ["python3", str(self.root / "scripts/conformance.py"), "--write", "--probe-reference"],
            env=self.env, cwd=self.root, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads((self.root / "docs/conformance-report.json").read_text())
        self.assertEqual(report["isa_spec_sha"], PIN)
        self.assertEqual(report["case_count"], len(list((source / "conformance/cases").glob("*/*/case.json"))))
        self.assertEqual(report["reference_09_admission_probe"]["exit"], 0)

    def test_inventory_regression_runs_in_gate_layout_without_local_cache(self):
        source = materialised_spec()
        shutil.copytree(ROOT / "scripts", self.root / "scripts")
        (self.root / "tests").mkdir()
        shutil.copyfile(ROOT / "tests/pin_resolution.py", self.root / "tests/pin_resolution.py")
        (self.root / "data").symlink_to(ROOT / "data", target_is_directory=True)
        env = dict(self.env, BENNETT_GATE_LAYOUT=str(source.parent.resolve()))
        result = subprocess.run(
            ["python3", "-I", str(self.root / "tests/pin_resolution.py"),
             "PinResolution.test_inventory_script_uses_pin_with_empty_live_sibling"],
            env=env, capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
