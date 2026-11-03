# bennett-sched batch review: 2026-11-03 (round 1)

Reviewer: Claude Opus 5.5 (opus-rev-sched; four Opus readers split by module, every P0/P1 re-run by the lead). This is the policy rule 2 opposite-model seat.
Range: `0930959^..26c2138` (43 commits). Reviewed sha: `26c21383c7fa1dff71419ac9140fefb9877fdebb` (detached worktree snapshot).
Contract: ISA 0.9 (`bennett-isa-spec` 961dc81, spec file differs from the 97e7b9f pin only by the K91 editor's note) and `data/ref-0.9.json` (byte-identical to the published file).

**Verdict: BLOCK.** 1 P0 and 2 P1s are newly filed. 6 P1s were already tracked by open beads and are re-confirmed below. The snapshot also fails its own test suite (`token_aliases`).

## New findings

| Sev | Where | Failure scenario | Ran | Bead |
|---|---|---|---|---|
| P0 | `src/source.rs:664-712` (`events.reverse()` :699) | An `adj action_call` is formed by reversing the already-expanded leaves. K62 (§5.3/§11.1) requires source replacement in reverse source order, then forward expansion. For inner `qxor s t` (2-bit), this gives ord0 = bit1 and ord1 = bit0, where the spec gives bit0 then bit1. Ordinal-bound selectors and certificates therefore bind different leaves from any conforming implementation. Affects qxor, qcopy/quncopy, qload/qunload, qstore and qexchm. | `repro-B tests/r.rs::adjoint_qxor_order` | bennett-sched-fdu |
| P1 | `src/engine.rs:55-69`, `:290-322` | Class-choice search is chronological and exhaustive: a capacity failure that does not depend on earlier choices still enumerates every earlier `qalloc any` choice. With a 2-class target, n free allocations, then `qalloc A 2` over capacity: n=10 takes 0.09 s, n=14 0.51 s, n=16 2.9 s (release). About n=30 would take hours. | `repro-A --bin search <n>` | bennett-sched-jez |
| P1 | `scripts/conformance.py:21-23` vs `tests/published_vectors.rs:62-90` | Four E vectors are labelled direct-tested, but the test only parses them (epsilon-zero-exact), checks adjacency (topology-grid-09), or accepts any error (epsilon-zero-entry, mbu-must-be-false). The test still passes with the expectations inverted or with the fixtures replaced by `{}` or `not json`. | reader D, scratch fixture copy; code read by lead | bennett-sched-b08 |

## Re-confirmed P1s already tracked (notes added, no duplicate beads)

- Red snapshot and K91: `cargo test --test token_aliases` fails at `tests/token_aliases.rs:78` because `bindings.token_aliases` is never emitted. `bennett_sched_request_set_token_aliases` is not exported (`nm`) and is missing from `data/bennett_sched.h`. Tracked in **xzv**.
- North-star narrowings, each producing NoMem by run:
  - `src/oracle.rs:640-666`: a 64 MiB cap rejects a one-row oracle at T=100000. Tracked in **2u4**.
  - `src/oracle_pairs.rs:383`: the 8192-state cap is effectively a row cap, so an oracle with N=9000 rows plus an uncall is rejected. Tracked in **kbc**.
  - qinplace has a 20-bit cap and no `inverse_pair_library` discharge path. Tracked in **a0v** and **vt8** (K92).
- Refusals:
  - `src/source.rs:2019-2023` rejects valid other-workspace cleanup with GateUnsupported (H S⁴ H = I). Tracked in **ark**.
  - `src/admission.rs:248` rejects recompute + `call_index`. Tracked in **ark**.
  - `src/target.rs:385-389` rejects the K86 `realisation_model` field. Tracked in **vt8**.
- Evidence pins disagree: `gate.toml` names ecaaac0 (k83 fixtures are absent there), `SPEC_VERSIONS.toml` names 97e7b9f, and `docs/conformance-report.json` passes only against an unrecorded 4cc6ac5. Tracked in **nsy**.

Nits (P2+): one bead, **bennett-sched-52u**.

## Coverage and provenance

- **Read closely:** engine, error_bounds, decimal, canonical, resources, ffi, request, target, schedule, expansion, ref_contract, source (invoke/adjoint/lifetimes), cleanup and replay, borrow, qinplace, object, oracle_pairs/memory/calls, gate scripts, published_vectors, conformance.py.
- **Skimmed:** admission, layout, lproc, source_ir, token_aliases, oracle_state/activation, cleanup/raw, about 35 test files.
- **Not read:** source/oracle_call.rs, source/call_graph.rs.
- **Checked sound:** upward 34-digit rounding, JCS key order and escaping, peak sweep, FFI catch_unwind, borrow translation lemma, recompute XOR algebra, §9.2 hashing (all 10 normative objects).
- **Mixed fat object:** nothing here locks it out.
- **Provenance:** 30 of the 43 commits are Codex (`wip(codex job sol-sched)`, gpt sol runner). The rest are orchestrator, beads, gate and worklog commits. No Claude implementation commits are in range.
- **R14 (scope):** only the reviewer's own paths were written. No fixes were made. Scheduler core is semantics-bearing, so on a later CLEAR round the rule 5 Muse seat is pending advisor confirmation.
