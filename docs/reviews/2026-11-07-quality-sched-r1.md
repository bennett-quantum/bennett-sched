# quality-sched review, round 1

Reviewed `main..job/quality-sched` / `main...job/quality-sched`: base
`576dbf160746f15b006fbab571ba0d8050629bc2`, head
`ad59bb344dbc2a232ba867a4b72edcd2f101d41c`. Includes pin-sched and the
3b10907 ISA re-pin; also inspected the inherited unreviewed semantic changes
since the previous batch head `26c21383c7fa1dff71419ac9140fefb9877fdebb`.

**Verdict: CLEAR. No confirmed P0/P1 findings; no P0/P1 beads.**

The phase extraction preserves the original operation bodies and ordering;
published trace/ordinal and basis tests cover the moved paths. Capacity
conflicts retain all live class-choice dependencies; topology conflicts now
reject missing dependency metadata. CFG proof retention keeps entry/join
checkpoints, without dropping the checks along straight paths. The revised
normative audit checks the contract diagnostic and empty unsupported outputs.
No newly introduced narrowing or silent fallback was found.

Red-first assessment used `git show main:<path>` without changing branches:
the original capacity failure leaves released choices enumerable, missing
placement metadata defaults to zero, and the old visited-state limit rejects
the 9,000-row proof. The repaired memory-adjoint fixtures previously failed
object validation before reaching the existing adjoint implementation.

Focused verification (headless, job-local TMPDIR/CARGO_TARGET_DIR):

- `timeout 600 cargo test --offline -j3 --lib -- --test-threads=4`: **4 passed**.
- `timeout 300 cargo test --offline -j3 --test integration -- source_adjoint:: structure:: oracle_pairs:: resources:: published_vectors:: gate:: conformance_inventory:: differential:: token_aliases:: ffi:: c_link:: pipeline:: --test-threads=4`: **63 passed**.
- `timeout 300 cargo test --offline -j3 --test integration -- expansion:: source:: linked_source:: named_interface:: oracle:: oracle_memory:: oracle_allocator:: oracle_lproc:: qinplace:: cleanup:: raw_inverse_cleanup:: admission:: layout:: k80_vectors:: k83_vectors:: --test-threads=4`: **89 passed, 6 slow tests ignored as declared**.
- `timeout 180 cargo test --offline -j3 --test integration -- normative_objects::every_normative_quantum_region_has_an_audited_typed_scheduler_outcome --ignored --exact --test-threads=4`: **1 passed**.
- `timeout 60 python3 -B tests/quality_metrics.py`: **8 passed**;
  `timeout 60 python3 -B scripts/quality.py --check`: **no regression**.
  Committed ISA spec bytes, gate/SPEC_VERSIONS/inventory SHAs and digests agree;
  `git diff --check main...job/quality-sched` passes. The single quality gate
  check, exhaustive-test gate entry, HANDOFF deletion and README handoff link
  are present. No full gate or complete slow-suite rerun was performed.

P2+ nits: **bennett-sched-7jo** (priority 3, review): `README.md:39-41` still claims the resolved pinned reference failure; the reference-admission test passes at this head.

Coverage: **read closely** scheduler conflict search, source adjoints and extracted phases, oracle construction/proof retention, inherited token-identity propagation, substantive test changes, pin resolver, gate and quality ratchet; **skimmed** remaining fixture-path/harness migrations, inventory records and historical documentation. Checked every COMMON anti-slop rule; no additional confirmed bug-hiding violation. Existing materialization/proof/cleanup limitations remain explicit tracked work, not completed capabilities.

Provenance: implementer **gpt-6.1-sol**; reviewer **gpt-6.1-sol, fresh session (D9)**. Review only; no implementation changes.
