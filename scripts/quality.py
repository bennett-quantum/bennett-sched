#!/usr/bin/env python3
"""Record/check this repository's lexical quality baseline; no external packages."""
import argparse
import ast
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
MARKER = "<!-- quality-baseline -->"
TOKEN = re.compile(r'//[^\n]*|/\*.*?\*/|r(\#*)".*?"\1|b?"(?:\\.|[^"\\])*"|b?\'(?:\\.|[^\'\\])\'', re.S)
BRANCH = re.compile(r"\b(?:if|else|for|while|loop|match)\b|&&|\|\||=>|[{}]")
PROCESS = re.compile(
    r"(?:bennett-(?:sched|vm|driver|plugin|conformance|estimate|targets|circuits|arith|runtimes|dialect-spec)|b-isa-spec)-[a-z0-9]+"
    r"|\b(?:sol-sched|quality-sched)\b|\b(?:job|wip)/[a-z0-9_-]+|\bshift \d+", re.I)
LITTER = re.compile(r"(?:^|/)(?:__pycache__|PROGRESS[^/]*|LAST-RUN[^/]*|LOCK)(?:/|$)|\.(?:pyc|log|darc|lock|backup)$")


def mask_rust(source):
    masked, cursor = list(source), 0
    while token := TOKEN.search(source, cursor):
        start, end = token.span()
        if source.startswith("/*", start):
            depth, end = 1, start + 2
            while depth:
                opening, closing = source.find("/*", end), source.find("*/", end)
                if closing < 0:
                    raise ValueError("unterminated Rust block comment")
                if 0 <= opening < closing:
                    depth += 1
                    end = opening + 2
                else:
                    depth -= 1
                    end = closing + 2
        masked[start:end] = ["\n" if c == "\n" else " " for c in source[start:end]]
        cursor = end
    return "".join(masked)


def rust_functions(source):
    """Mask comments/strings, then balance function braces (line count unchanged)."""
    masked = mask_rust(source)
    for match in re.finditer(r"\bfn\s+\w+\s*(?:<[^;{]*>)?\s*\(", masked):
        opening, parentheses, brackets = match.end(), 1, 0
        while opening < len(masked):
            char = masked[opening]
            parentheses += (char == "(") - (char == ")")
            brackets += (char == "[") - (char == "]")
            if not parentheses and not brackets and char in "{;":
                break
            opening += 1
        if opening == len(masked) or masked[opening] == ";":
            continue
        depth = 1
        end = opening + 1
        while depth:
            if end >= len(masked):
                raise ValueError("unbalanced Rust function")
            depth += (masked[end] == "{") - (masked[end] == "}")
            end += 1
        yield source[match.start():end].count("\n") + 1, masked[opening + 1:end - 1]


def rust_metrics(source):
    lengths, mass, wrappers = [], 0, 0
    for length, body in rust_functions(source):
        lengths.append(length)
        depth = 0
        for token in BRANCH.findall(body):
            if token == "{":
                depth += 1
            elif token == "}":
                depth -= 1
            else:
                mass += 1 if token in ("&&", "||", "=>") else 1 + depth
        if re.fullmatch(r"\s*(?:return\s+)?[\w:.]+\([^;{}]*\)\??;?\s*", body):
            wrappers += 1
    return lengths, mass, wrappers


def python_metrics(source):
    tree = ast.parse(source)
    functions = [n for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
    lengths = [n.end_lineno - n.lineno + 1 for n in functions]
    branches = (ast.If, ast.For, ast.AsyncFor, ast.While, ast.Try, ast.IfExp, ast.Match)

    def complexity(node, depth=0):
        branch = isinstance(node, branches)
        own = (1 + depth) if branch else 0
        if isinstance(node, ast.BoolOp):
            own += 1
        return own + sum(complexity(child, depth + branch) for child in ast.iter_child_nodes(node))

    wrappers = sum(len(n.body) == 1 and isinstance(n.body[0], ast.Return)
                   and isinstance(n.body[0].value, ast.Call) for n in functions)
    return lengths, complexity(tree), wrappers


def measure(root):
    files, lengths, wrappers = {}, [], 0
    for directory in ("src", "scripts"):
        for path in sorted((root / directory).rglob("*")):
            if path.suffix not in (".rs", ".py"):
                continue
            source = path.read_text()
            sizes, mass, count = (rust_metrics if path.suffix == ".rs" else python_metrics)(source)
            files[str(path.relative_to(root))] = mass
            lengths.extend(sizes)
            wrappers += count
    tracked = subprocess.check_output(["git", "-C", str(root), "ls-files", "-z"]).decode().split("\0")
    identifiers = sum(len(PROCESS.findall(p.read_text())) for p in (root / "src").rglob("*.rs"))
    return {
        "function_lengths": {"count": len(lengths), "maximum": max(lengths, default=0),
                             **{f"over_{n}": sum(size > n for size in lengths) for n in (30, 100, 300)}},
        "complexity_mass": files,
        "trivial_wrappers": wrappers,
        "process_identifiers": identifiers,
        "litter": sorted(p for p in tracked if LITTER.search(p)),
    }


def regressions(current, baseline):
    errors = []
    for name in ("trivial_wrappers", "process_identifiers"):
        if current[name] > baseline[name]:
            errors.append(f"{name}: {baseline[name]} -> {current[name]}")
    for name, value in current["function_lengths"].items():
        # More small functions alone is not a regression; wrappers and mass
        # constrain gratuitous splitting separately.
        if name != "count" and value > baseline["function_lengths"][name]:
            errors.append(f"function_lengths.{name}: {baseline['function_lengths'][name]} -> {value}")
    for path, mass in current["complexity_mass"].items():
        previous = baseline["complexity_mass"].get(path, 0)
        if mass > previous:
            errors.append(f"complexity_mass.{path}: {previous} -> {mass}")
    if extra := sorted(set(current["litter"]) - set(baseline["litter"])):
        errors.append(f"new tracked litter: {extra}")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    current = measure(ROOT)
    document = ROOT / "docs/quality.md"
    if args.record:
        prefix = document.read_text().split(MARKER)[0].rstrip()
        document.write_text(prefix + "\n\n" + MARKER + "\n```json\n" + json.dumps(current, indent=2, sort_keys=True) + "\n```\n")
    elif args.check:
        baseline = json.loads(document.read_text().split(MARKER, 1)[1].split("```json\n", 1)[1].split("```", 1)[0])
        errors = regressions(current, baseline)
        if errors:
            raise SystemExit("quality baseline regressed:\n" + "\n".join(errors))
        print("quality baseline: no regression")
    else:
        print(json.dumps(current, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
