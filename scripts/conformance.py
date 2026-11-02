#!/usr/bin/env python3
"""Inventory every published vector. Coverage mappings are not pass verdicts."""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT.parent / "bennett-isa-spec"

DIRECT = {
    "E-imports": "published_vectors::published_import_quantity_is_mapped_exactly_and_bound_to_model_version",
    "E-aggregation": "published_vectors::published_aggregation_leaf_list_is_counted_once_with_classical_time",
    "E-ast-division-by-zero": "published_vectors::published_cost_ast_vectors_match_their_exact_outcomes",
    "E-ast-eps-out-of-range": "published_vectors::published_cost_ast_vectors_match_their_exact_outcomes",
    "E-ast-log2inv": "published_vectors::published_cost_ast_vectors_match_their_exact_outcomes",
    "E-ast-unbound-variable": "published_vectors::published_cost_ast_vectors_match_their_exact_outcomes",
    "E-table-index": "published_vectors::published_cost_ast_vectors_match_their_exact_outcomes",
    "E-epsilon-zero-entry": "published_vectors::published_target_hash_and_topology_vectors_are_checked_directly",
    "E-epsilon-zero-exact": "published_vectors::published_target_hash_and_topology_vectors_are_checked_directly",
    "E-jcs-target-hash": "published_vectors::published_target_hash_and_topology_vectors_are_checked_directly",
    "E-mbu-must-be-false": "published_vectors::published_target_hash_and_topology_vectors_are_checked_directly",
    "E-topology-grid-09": "published_vectors::published_target_hash_and_topology_vectors_are_checked_directly",
}
EQUIVALENT = {
    "E-matching-first-entry": "target::matching_continues_after_unachievable_accuracy_and_counts_effective_controls",
    "E-ref-09-qxor-witness": "expansion::published_bound_qxor_all_sixteen_tuples_and_inverse_ordinals",
    "E-scheme-expansion": "target::published_scheme_repeat_emits_three_leaves_without_charging_a_parent",
    "E-timing-overlap": "resources::scheduled_overlap_sums_demands_and_half_open_reuse_does_not",
    "H-path-not-in-object": "admission (linked source point, loops, instances and semantic-call path validation)",
    "F-borrow-positive-check": "borrow::normative_translation_frame_is_universal_and_emits_a_separate_borrow_record",
    "F-borrow-neg-swap": "borrow::translation_lemma_never_certifies_any_of_the_four_normative_negative_patterns",
    "F-borrow-neg-vwv": "borrow::translation_lemma_never_certifies_any_of_the_four_normative_negative_patterns",
    "F-borrow-neg-control-in-b": "borrow::translation_lemma_never_certifies_any_of_the_four_normative_negative_patterns",
    "F-borrow-neg-target-outside-b": "borrow::translation_lemma_never_certifies_any_of_the_four_normative_negative_patterns",
    "H-specificity-incomparable": "selectors (maximal ties, independent kinds, nested invocation coordinates)",
    "H-defaults-uncovered": "canonical_cost::decision_decimals_preserve_all_digits_and_shares_round_down; pipeline",
    "H-capacity-peak": "pipeline::infeasible_capacity_has_typed_outcome_reason_and_no_documents",
    "H-reproducibility": "pipeline::schedules_real_controlled_fixture_and_emits_bound_exact_certificate",
    "H-mbu-realisation-invalid": "schedule::parse_entry restricts unitary cleanup; selectors",
    "H-unknown-expansion-library": "selectors::a_schedule_cannot_bind_an_unavailable_expansion_library",
    "H-unsupported": "pipeline::postponed_lproc_oracle_has_a_specific_bead_reason_and_no_feasible_document",
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="write docs/conformance-report.json")
    parser.add_argument("--probe-reference", action="store_true")
    parser.add_argument("--require-complete", action="store_true", help="exit 2 while vectors remain unexercised")
    args = parser.parse_args()
    for key in ["DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "BROWSER"]:
        os.environ.pop(key, None)
    os.environ.setdefault("TMPDIR", str(ROOT / ".tmp/sol-sched/tmp"))
    Path(os.environ["TMPDIR"]).mkdir(parents=True, exist_ok=True)
    records = []
    suites = collections.Counter()
    for path in sorted((SPEC / "conformance/cases").glob("*/*/case.json")):
        raw = path.read_bytes()
        case = json.loads(raw)
        identifier = case["id"]
        suites[case["suite"]] += 1
        record = {"id": identifier, "suite": case["suite"], "source": str(path.relative_to(SPEC)), "sha256": sha(raw)}
        if identifier in DIRECT:
            record.update(coverage="direct-fixture-test", test=DIRECT[identifier])
        elif identifier in EQUIVALENT:
            record.update(coverage="equivalent-witness-test", test=EQUIVALENT[identifier])
        elif identifier.startswith("E-ref-09-"):
            record.update(coverage="partial-reference-expansion", beads=["bennett-sched-ark", "bennett-sched-qnl"])
        elif case["suite"] in {"E", "H"} or identifier == "G-estimate-resolved-class-decimal":
            record.update(coverage="not-exercised-scheduler-vector", beads=["bennett-sched-ark"])
        else:
            record.update(coverage="not-exercised-executor-or-checker-vector", reason="No executor/checker CLI is claimed by this scheduler. Full source expansion conformance remains open.", beads=["bennett-sched-ark"])
        records.append(record)
    if not records or set(suites) != set("ABCDEFGH"):
        raise RuntimeError("missing or incomplete published A–H case inventory")
    spec_bytes = (SPEC / "spec/Bennett-ISA-Spec-0.9.md").read_bytes()
    example_text = spec_bytes.decode().split("### 16.3 Example objects", 1)[1].split("## 17.", 1)[0]
    examples = []
    for index, block in enumerate(example_text.split("```")):
        block = block.lstrip("\n")
        if index % 2 and block.startswith("bennett-object "):
            record={"number": len(examples) + 1, "sha256": sha(block.encode()), "coverage": "all-interface-hashes-checked", "test": "normative_objects"}
            if record["number"]==1: record.update(schedule="pipeline::normative_inc8_schedules_through_the_abi_with_history_and_oracle_claim",semantics="oracle (equivalent two-bit increment full input/target/fault/control domains, budgets 0-4)")
            if record["number"]==3: record.update(semantics="borrow::normative_translation_frame_is_universal_and_emits_a_separate_borrow_record (all 8 basis tuples and 16 inherited-control tuples)")
            symbols=json.loads(block.split("section symbols\n",1)[1].split("\n",1)[0])
            regions=[]
            deferred={"sum_db":"layout/lproc construction", "through":"layout/lproc construction", "plus_db":"layout state construction", "g":"classical pseudo-location wire binding", "ginv":"classical pseudo-location wire binding", "addya":"qinplace inverse-pair construction"}
            for symbol in symbols.values():
                if symbol["kind"] in {"oracle","action","perm_action"}:
                    name=symbol["name"]
                    region={"name":name,"interface_hash":symbol["interface_hash"],"outcome":"UNSUPPORTED" if name in deferred else "OK","test":"normative_objects::every_normative_quantum_region_has_an_audited_typed_scheduler_outcome"}
                    if name in deferred: region.update(reason=deferred[name],beads=["bennett-sched-r28","bennett-sched-ark"] if name in {"g","ginv"} else ["bennett-sched-ark"])
                    regions.append(region)
            record["quantum_region_audit"]=regions
            examples.append(record)
    portable = []
    for path in sorted((SPEC / "conformance/fixtures").glob("accepted-*/*.json")):
        v = json.loads(path.read_bytes())
        record={"path": str(path.relative_to(SPEC)), "sha256": sha(path.read_bytes()), "vectors": len(v.get("vectors", v.get("cases", []))), "coverage": "not-exercised", "beads": ["bennett-sched-ark"]}
        if path.name=="k82.json":
            record.update(coverage="direct-fixture-test",test="named_interface",catalog_interfaces=54,edge_digests=86)
        elif path.name=="k79.json":
            record.update(coverage="source-admission-and-complete-basis-tuples",test="regions::every_k79_source_address_vector_is_admitted_or_rejected_before_ref_helper_expansion",admitted=4,rejected=8,basis_tuples=128)
        elif path.name=="k80.json":
            record.update(coverage="partial-equivalent-witness",test="cleanup (primitive/intrinsic recompute, eight-leaf carrier ordinals, loop coordinate provenance)",remaining="fresh semantic-call recompute contexts and direct complete manifest replay")
        elif path.name=="k81.json":
            record.update(coverage="partial-equivalent-witness",test="cleanup::intrinsic_boundaries_and_joint_call_inverses_have_exactly_one_owner",remaining="fresh call recompute, nonzero target history replay and direct complete manifest replay")
        portable.append(record)
    report = {
        "schema": "bennett-sched.conformance-inventory.v1",
        "meaning": "Coverage inventory, not a full-conformance pass report. Equivalent witnesses are separate from direct fixture replay.",
        "spec_sha256": sha(spec_bytes), "case_count": len(records), "suites": dict(sorted(suites.items())),
        "coverage_counts": dict(sorted(collections.Counter(r["coverage"] for r in records).items())),
        "normative_scheduler_audit":{"regions":sum(len(e["quantum_region_audit"]) for e in examples),"outcomes":dict(collections.Counter(r["outcome"] for e in examples for r in e["quantum_region_audit"])),"request":"B=3, heap=stack=depth=0; typed zero args, tokens=(3,0,0,0); universal exact target, unbounded capacity, no topology","meaning":"Producer schedule/document audit with tracked deferrals; no independent checker verdict."},
        "cases": records, "normative_objects": examples, "portable_manifests": portable,
        "reference_schedule_execution": "not claimed; installed reference interpreter documents staged target/schedule execution",
    }
    if args.probe_reference:
        candidates = [Path(os.environ["BENNETT_REF_BIN"])] if "BENNETT_REF_BIN" in os.environ else [SPEC / "reference-interpreter/target" / mode / "bennett-isa-ref" for mode in ("release", "debug")]
        binary = next((p for p in candidates if p.is_file()), None)
        if binary is None:
            raise RuntimeError("reference binary missing; set BENNETT_REF_BIN")
        command = [str(binary), "check", str(SPEC / "conformance/fixtures/controlled-action-09/controlled.bisa")]
        result = subprocess.run(command, capture_output=True, text=True, timeout=30, check=False)
        report["reference_09_admission_probe"] = {"command": command, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    if args.write:
        # COMMON requires time to be read in the same command that records it.
        report["recorded_at_utc"] = subprocess.check_output(["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], text=True).strip()
        (ROOT / "docs/conformance-report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"case_count": len(records), "suites": report["suites"], "coverage_counts": report["coverage_counts"], "normative_objects": len(examples), "portable_manifests": len(portable)}, sort_keys=True))
    return 2 if args.require_complete and any(r["coverage"].startswith(("partial", "not-")) for r in records) else 0


if __name__ == "__main__":
    raise SystemExit(main())
