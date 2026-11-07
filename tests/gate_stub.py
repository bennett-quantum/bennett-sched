"""Execute the installed stub against a disposable committed tool contract."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class Hook(unittest.TestCase):
    def test_committed_tool_receives_arguments_and_its_exit_status_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            hook = root / "scripts/pre-push"
            shutil.copyfile(ROOT / "scripts/pre-push", hook)
            hook.chmod(0o755)
            env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_") and k != "BENNETT_GATE_HOOK_PROBE"}

            def git(*args):
                subprocess.run(["git", "-C", str(root), *args], env=env, check=True,
                               capture_output=True, text=True, timeout=10)

            git("init", "-q")
            sha = "1" * 40
            (root / "gate.toml").write_text(f'tool_sha = "{sha}"\n')
            git("add", "--", "gate.toml")
            git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "fixture")
            # A worktree edit must not change the executable tool pin.
            (root / "gate.toml").write_text('tool_sha = "' + "2" * 40 + '"\n')
            tool = root / f".tmp/gate/tool/gate_run-{sha}.py"
            tool.parent.mkdir(parents=True)
            for status in (0, 7):
                tool.write_text("import json,sys\nprint(json.dumps(sys.argv[1:]))\nraise SystemExit(" + str(status) + ")\n")
                result = subprocess.run([str(hook), "origin", "destination"], env=env,
                                        capture_output=True, text=True, timeout=15)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertEqual(json.loads(result.stdout), ["run", "--repo", str(root), "--tool-sha", sha, "origin", "destination"])


if __name__ == "__main__":
    unittest.main()
