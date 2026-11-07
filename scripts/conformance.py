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
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from pinned_sibling import resolve

ROOT = Path(__file__).resolve().parent.parent

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
    "E-topology-grid-09": "engine::tests::published_grid_placement_outcomes_match_scheduler_admission",
}
DIRECT.update({f"F-k83-{name}": "k83_vectors::every_published_k83_source_trace_and_external_basis_value_matches_including_raw_inverse" for name in ["zero-trip","qif-arms","image-01","image-81","controlled-image-81","repeated-image-81"]})
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
    "H-unsupported": "normative_objects::every_normative_quantum_region_has_an_audited_typed_scheduler_outcome",
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    pin = resolve(ROOT, "bennett-isa-spec")
    SPEC = Path(pin["path"])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="write docs/conformance-report.json")
    parser.add_argument("--probe-reference", action="store_true")
    parser.add_argument("--require-complete", action="store_true", help="exit 2 while vectors remain unexercised")
    args = parser.parse_args()
    for key in ["DISPLAY", "WAYLAND_DISPLAY", "DBUS_SESSION_BUS_ADDRESS", "BROWSER"]:
        os.environ.pop(key, None)
    os.environ.setdefault("TMPDIR", str(ROOT / ".tmp/conformance/tmp"))
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
        elif identifier.startswith("H-k83-"):
            record.update(coverage="rebound-schedule-fixture-test",test="k83_vectors::published_k83_schedule_polarities_hold_after_explicit_library_and_target_rebinding",remaining="Published schedule omits required expansion binding and has placeholder target hash. Original strict rejection and explicitly rebound admission are separate evidence.",beads=["bennett-sched-qnl"],target_adapter_sha256=sha((ROOT / "data/k83-target.json").read_bytes()))
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
            deferred={"through":"classical pseudo-location wire binding in nested/lproc callee", "plus_db":"classical pseudo-location wire binding", "g":"classical pseudo-location wire binding", "ginv":"classical pseudo-location wire binding", "addya":"classical pseudo-location wire binding in inverse pair"}
            for symbol in symbols.values():
                if symbol["kind"] in {"oracle","action","perm_action"}:
                    name=symbol["name"]
                    region={"name":name,"interface_hash":symbol["interface_hash"],"outcome":"UNSUPPORTED" if name in deferred else "OK","test":"normative_objects::every_normative_quantum_region_has_an_audited_typed_scheduler_outcome"}
                    if name in deferred: region.update(reason=deferred[name],beads=["bennett-sched-r28","bennett-sched-ark"] if name in {"g","ginv","plus_db","addya","through"} else ["bennett-sched-ark"])
                    if name=="sum_db":
                        region.update(outcome="ERROR",status="BENNETT_E_NOMEM",reason="Materialized reference constructor leaf workspace exhausted at B=3; scalable expansion remains required.",beads=["bennett-sched-2u4","bennett-sched-ark"],zero_budget_witness={"outcome":"OK","budget":[0,0,0,0]})
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
        elif path.name=="k83.json":
            record.update(coverage="source-traces-and-rebound-schedule-admission",test="k83_vectors",source_cases=6,schedule_cases=8,rebound_accepted=2,rebound_rejected=6,original_missing_expansion=8,target_adapter_sha256=sha((ROOT / "data/k83-target.json").read_bytes()),remaining="Original positive schedule schema/bindings need upstream fixture repair; exact request queued under COMMON14.")
        elif path.name=="k80.json":
            record.update(coverage="version-rebound-source-traces-and-complete-basis-tuples",test="k80_vectors::all_published_k80_trace_fields_and_complete_basis_tuples_match_after_version_rebinding",source_cases=5,basis_tuples=80,remaining="Published 0.8 source bodies are re-enveloped/rehashed for 0.9 without instruction changes; this is not verbatim 0.9 document admission.")
        elif path.name=="k81.json":
            record.update(coverage="partial-equivalent-witness",test="cleanup::intrinsic_boundaries_and_joint_call_inverses_have_exactly_one_owner",remaining="General mutable-interface/borrowed pre-call history replay and direct complete manifest replay; allocated copy history and fresh call spaces have focused coverage.")
        portable.append(record)
    report = {
        "schema": "bennett-sched.conformance-inventory.v1",
        "meaning": "Coverage inventory, not a full-conformance pass report. Equivalent witnesses are separate from direct fixture replay.",
        "isa_spec_sha": pin["sha"],
        "spec_sha256": sha(spec_bytes), "case_count": len(records), "suites": dict(sorted(suites.items())),
        "coverage_counts": dict(sorted(collections.Counter(r["coverage"] for r in records).items())),
        "normative_scheduler_audit":{"regions":sum(len(e["quantum_region_audit"]) for e in examples),"outcomes":dict(collections.Counter(r["outcome"] for e in examples for r in e["quantum_region_audit"])),"request":"B=3, heap=stack=depth=0; typed zero args, tokens=(3,0,0,0); universal exact target, unbounded capacity, no topology","meaning":"Producer schedule/document audit with tracked deferrals; no independent checker verdict."},
        "cases": records, "normative_objects": examples, "portable_manifests": portable,
        "inline_witnesses": [{"id":"K77-TWO-TOKEN", "source":"spec/Bennett-ISA-Spec-0.9.md", "section":"11.1 Two-token witness", "coverage":"published-source-expansion-witness", "test":"oracle_calls::published_k77_two_token_inc8_witness_covers_every_input_and_both_controls", "registry_adapter_test":"linked_source::source_registry_replays_the_actual_k77_two_token_witness_through_direct_and_captured_adapters", "capture_forms":["direct","manual-body","manual-template","typed-body","typed-template"], "input_values":256, "control_values":2, "initial_target":0, "initial_fault":0, "certificate_limit":"Combined differing-budget resource reports require bennett-sched-1na; no complete certificate claim."}],
        "reference_schedule_execution": "not claimed; published reference interpreter documents staged target/schedule execution",
        "reference_09_schedule_admission": {"test":"differential::emitted_09_schedule_and_certificate_pass_reference_metadata_admission","status":"blocked-at-pin","upstream":"b-isa-spec-3wl9","meaning":"The pinned reference rejects token_aliases; the default test remains active. This is not certificate-claim replay or scheduled execution."},
    }
    if args.probe_reference:
        binary = Path(resolve(ROOT, "bennett-isa-spec", reference=True)["binary"])
        command = [str(binary), "check", str(SPEC / "conformance/fixtures/controlled-action-09/controlled.bisa")]
        result = subprocess.run(command, capture_output=True, text=True, timeout=30, check=False)
        report["reference_09_admission_probe"] = {"command": command, "exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr}
    if args.write:
        # COMMON requires time to be read in the same command that records it.
        report["recorded_at_utc"] = subprocess.check_output(["date", "-u", "+%Y-%m-%dT%H:%M:%SZ"], text=True).strip()
        (ROOT / "docs/conformance-report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"case_count": len(records), "suites": report["suites"], "coverage_counts": report["coverage_counts"], "normative_objects": len(examples), "portable_manifests": len(portable)}, sort_keys=True))
    return 2 if args.require_complete and any(r["coverage"].startswith(("partial", "not-", "rebound")) for r in records) else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as error:
        raise SystemExit(str(error))
