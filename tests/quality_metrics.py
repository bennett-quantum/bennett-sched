"""Hand-derived metrics and deliberate regression mutations for the ratchet."""
import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("quality", Path(__file__).resolve().parents[1] / "scripts/quality.py")
quality = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quality)


class Metrics(unittest.TestCase):
    def test_rust_strings_comments_and_nested_decisions(self):
        source = '''// fn phantom() { if false {} }
fn actual() {
    let text = r#"fn phantom() { while true {} }"#;
    if ready { while pending { step(); } }
}
'''
        self.assertEqual(quality.rust_metrics(source), ([4], 3, 0))

    def test_wrapper_count_distinguishes_dispatch_and_straight_call(self):
        self.assertEqual(quality.rust_metrics("fn f() { self.run() }"), ([1], 0, 1))
        self.assertEqual(quality.rust_metrics("fn f() { if ready { run() } }"), ([1], 1, 0))

    def test_wrapper_count_is_independent_of_signature_formatting(self):
        source = "fn f(\n    a: u64,\n    b: u64,\n    c: u64,\n    d: u64,\n) {\n    run(a, b, c, d)\n}"
        self.assertEqual(quality.rust_metrics(source), ([8], 0, 1))

    def test_array_parameter_semicolons_do_not_hide_function_bodies(self):
        self.assertEqual(quality.rust_metrics("fn f(a: [u64; 4]) { if ready { run() } }"), ([1], 1, 0))

    def test_nested_block_comments_do_not_invent_functions(self):
        source = "/* outer /* inner */ fn phantom() { if false {} } */ fn real() { run() }"
        self.assertEqual(quality.rust_metrics(source), ([1], 0, 1))

    def test_python_nesting_and_wrappers(self):
        self.assertEqual(quality.python_metrics("def f():\n    if ready:\n        while pending:\n            step()\n"), ([4], 3, 0))
        self.assertEqual(quality.python_metrics("def f():\n    return run()\n"), ([2], 0, 1))

    def test_each_kind_of_increase_fails_independently(self):
        baseline = {"function_lengths": {"count": 1, "maximum": 10, "over_30": 0, "over_100": 0, "over_300": 0},
                    "complexity_mass": {"src/a.rs": 3}, "trivial_wrappers": 0,
                    "process_identifiers": 0, "litter": []}
        self.assertEqual(quality.regressions(baseline, baseline), [])
        changes = [
            ("function_lengths", "maximum", 11),
            ("function_lengths", "over_30", 1),
            ("complexity_mass", "src/a.rs", 4),
            ("complexity_mass", "src/new.rs", 1),
            ("trivial_wrappers", None, 1),
            ("process_identifiers", None, 1),
            ("litter", None, ["src/cache.pyc"]),
        ]
        for field, nested, value in changes:
            with self.subTest(field=field, nested=nested):
                current = copy.deepcopy(baseline)
                if nested is None:
                    current[field] = value
                else:
                    current[field][nested] = value
                self.assertTrue(quality.regressions(current, baseline))

    def test_litter_and_process_identifiers_are_detected(self):
        for path in ("src/__pycache__/a.pyc", "tool.log", "docs/PROGRESS.md", "data/x.darc", ".beads/dolt/LOCK"):
            self.assertIsNotNone(quality.LITTER.search(path), path)
        self.assertIsNone(quality.LITTER.search("src/oracle.rs"))
        self.assertIsNotNone(quality.PROCESS.search("postponed (bennett-sched-abc)"))
        self.assertIsNotNone(quality.PROCESS.search("pending b-isa-spec-ab12"))
        self.assertIsNotNone(quality.PROCESS.search("job/new-task"))
        self.assertIsNone(quality.PROCESS.search("bennett-isa-spec"))
        self.assertIsNone(quality.PROCESS.search("§11.1 K62"))


if __name__ == "__main__":
    unittest.main()
