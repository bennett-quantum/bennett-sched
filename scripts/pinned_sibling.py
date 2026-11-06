"""Resolve this repository's gate.toml pins for scripts and Rust tests."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib


def head(path):
    # Only inspect this directory's repository, never an enclosing repository.
    if not (path / ".git").exists():
        return "unavailable"
    result = subprocess.run(["git", "-C", str(path), "rev-parse", "HEAD"],
                            env={k: v for k, v in os.environ.items() if not k.startswith("GIT_")},
                            capture_output=True, text=True, timeout=10)
    return result.stdout.strip() if result.returncode == 0 else "unavailable"


def resolve(root, name, reference=False):
    config = tomllib.loads((root / "gate.toml").read_text())
    sha = config["siblings"][name]["sha"]
    if len(sha) != 40 or any(c not in "0123456789abcdef" for c in sha):
        raise RuntimeError(f"invalid gate.toml pin for {name}: {sha!r}")
    path = (Path(os.environ["BENNETT_GATE_LAYOUT"]) / name
            if "BENNETT_GATE_LAYOUT" in os.environ else
            root / ".tmp/gate/cache" / f"{name}@{sha[:12]}" / name)

    def fail(reason):
        raise RuntimeError(f"{name} pinned sha={sha}: {reason}; live ../{name} HEAD={head(root.parent / name)}; "
                           f"materialise with gate_run.py verify --repo {config['repo']} --sha HEAD")

    actual = head(path)
    if actual != sha:
        fail(f"{path} HEAD={actual}")
    result = {"path": str(path), "sha": sha}
    if reference:
        candidates = ([Path(os.environ["BENNETT_REF_BIN"])] if "BENNETT_REF_BIN" in os.environ else
                      [path / "reference-interpreter/target" / mode / "bennett-isa-ref" for mode in ("debug", "release")])
        binary = next((p for p in candidates if p.is_file() and p.resolve().is_relative_to(path.resolve())), None)
        if binary is None:
            fail("reference binary missing from pinned checkout or BENNETT_REF_BIN points outside it")
        result["binary"] = str(binary)
    return result


if __name__ == "__main__":
    try:
        print(json.dumps(resolve(Path(sys.argv[1]), sys.argv[2], "--reference" in sys.argv[3:])))
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        sys.exit(str(error))
