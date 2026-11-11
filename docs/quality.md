# Scheduler quality evidence

Evidence for the current additions uses the ISA revision in `gate.toml`
(`52836976829e05770090dd49c50909541c1207ba`). Older measurements below retain
their explicitly named prior revisions. This is a remediation record,
not a claim that all published capabilities or quality targets are complete.

The reference metadata-admission differential passes unchanged at this pin:
the emitted schedule and certificate are admitted, and an absent source point
is rejected. The spec digest was checked against the committed bytes, and the
reference catalog is byte-identical to the previous pin.

The timing and coverage measurements below were recorded at the former
`97e7b9f` pin. Its `token_aliases` rejection is historical; the new-pin
verification is recorded separately below. Coverage runs that continued after
that historical failure remain measurements, not successful test runs.

Inherited review work:
- **bennett-sched-fdu**: valid layout fixtures; three adjoint tests pass and
  disabling replacement expansion fails all three on primitive ordering.
- **bennett-sched-jez**: capacity conflicts retain live class dependencies;
  a deterministic thirty-released-choice regression fails before the fix.
- **bennett-sched-b08**: exact published shares, costs, error categories/reasons
  and fixed grid placement verdicts are checked. A changed native duration
  fails the published literal-time assertion.
- **bennett-sched-nsy**: inventory regenerated from the gate pin; its complete
  case and manifest hash test covers 557 cases and eight manifests.
- **bennett-sched-xzv**: setter/identity tests and actual C linkage pass.
  The accepted per-role K91 query is now implemented; Rust FFI and linked C11
  consumers verify the exact manifest, lifetime and atomic NULL-output behavior.
- **bennett-sched-r28**: section 3.7 permits classical pseudo-location reads,
  but K25 omits their physical allocation, initialization and ordinal ordering.
- **bennett-sched-kbc**: straight paths no longer retain one proof state per
  row; the unchanged 9,000-row regression passes. General cyclic pending-stack
  summaries remain required.
- **bennett-sched-2u4**, **bennett-sched-a0v**, **bennett-sched-ark** and
  **bennett-sched-vt8** remain open. Materialized construction, exhaustive pair
  proofs, general cleanup and native realisation models are algorithmic work,
  not missing contracts or reduced supported scope.

Focused validation: `timeout 180 cargo test --offline -j3 --lib --test
published_vectors --test oracle_pairs --test token_aliases --test ffi --test
c_link -- --test-threads=4` passed 32 tests. `timeout 180 cargo test --offline
-j3 --test conformance_inventory -- --test-threads=4` passed one test.


Default test timing at 804eb15: 155.15 seconds including 59.73 seconds of build,
95.19 seconds for the integration suite. Library tests: 2 passed. Integration:
220 passed, 1 known upstream failure, 10 slow tests retained under --ignored.
Command: timeout 600 cargo test --offline -j3 -- --test-threads=4.

Coverage before structural refactoring (804eb15): **87.86% lines**.
Command: timeout 600 cargo llvm-cov --offline -j3 --ignore-run-fail --json
--summary-only --ignore-filename-regex "/tests/" --output-path
.tmp/quality-sched/coverage.json -- --test-threads=4. The one known reference
failure is present in the test log; --ignore-run-fail permits report generation
and does not make that test pass. Ignored exhaustive cases are outside this
default-suite coverage number.

Five least-covered modules in this measurement:
- src/lib.rs: 75.0% (18/24 lines).
- src/ffi.rs: 75.41% (138/183 lines).
- src/target.rs: 76.83% (567/738 lines).
- src/cleanup/replay.rs: 81.86% (564/689 lines).
- src/oracle.rs: 82.58% (1010/1223 lines).
The FFI error/panic guard gets direct tests in the test quality pass. Other
low coverage comes from validation/error paths and rare layout/capture cases;
coverage includes actual token-alias C consumers and typed source binding tests.
Slow full-domain tests remain gate obligations, not deleted coverage.

Structural pass: source traversal now has separate loop, conditional, call,
bundle, memory and gate phases. Expansion separates semantic calls, templates,
record visits, declarations and inversion. Oracle construction separates
interface binding, byte layout, fixed procedure rows and event projection;
value rows and memory rows share operand validation. Target native-entry
validation and allocator realisation are separate phases.

The source scanner reports no function over 300 lines (maximum 298).
The unchanged published trace/ordinal, basis, source adjoint, source registry,
layout and target tests exercise these phases. The extracted value-row phase
also has a literal frame-slice and invalid-width unit test. After splitting:
four library tests pass; the default integration suite has 222 passes, the
same one upstream reference failure, and ten ignored cases (64.42 seconds of
test execution; 189.78 seconds including the full multi-file rebuild).

Fail-fast and litter pass: a missing live-wire placement dependency now returns
InvalidState; the corruption regression fails before the fix and passes after.
Zero defaults retained elsewhere represent unselected search choices, absent
optional topology, initial source versions or empty register sets; they are not
used to conceal this metadata failure. Product source contains zero process
identifiers. Generated Python bytecode was removed and ignore rules cover
bytecode, logs, locks and backups. One previously tracked backup,
.beads/hooks/pre-push.backup, is outside this job owned paths and remains an
orchestrator cleanup under bennett-sched-ogc.

The gate and SPEC_VERSIONS spec commit/file digest were checked against git show
of the pinned commit. At the former pin, README and the conformance inventory
attributed the reference failure instead of repeating an unsupported admission
claim. Ten oracle-pair tests passed after diagnostic cleanup.

The quality ratchet is one gate check: python3 -B scripts/quality.py --check.
It compares this repo with the JSON baseline below, not a global threshold.
Measurements cover Rust under src/ and Python under scripts/. Rust function
lengths include signatures and comments; comments/strings are masked when
finding braces. The cognitive-complexity mass is a documented lexical proxy:
decisions cost one plus brace nesting, logical operators and match arms cost
one; Python uses AST decision nesting. A wrapper is a function whose body is
one forwarded call (independent of signature formatting). Increasing any
length-tail count, maximum, per-file mass, wrapper count, process-identifier
count or tracked litter fails. Function count alone is reported, not a limit.
Litter is inspected across all tracked paths; process identifiers are searched
in product Rust source. Eight hand-derived/mutation unit tests check measurement
and regression directions, including raw strings, nested comments and array
parameter semicolons. The installed gate launcher runs those tests.

Reasons/tests for the five initially least-covered modules:
- lib.rs: most uncovered lines are the compatibility probe and generic error
  conversion/formatting, rather than an unexecuted semantic subsystem. Existing
  C consumers exercise the binary interface; no coverage threshold is imposed.
- ffi.rs: the new direct guard test covers panic containment, successful status
  and preserved argument errors. Existing C consumers cover ownership and alias
  copies; invalid non-null pointers remain the documented caller obligation.
- target.rs: published exact-share and specific rejection tests were strengthened.
  Native realisation-model support remains explicit unfinished work under vt8;
  parsing a field without validating its model would provide misleading coverage.
- cleanup/replay.rs: primitive/intrinsic and allocated pre-call history have
  semantic tests. General arbitrary-interface/borrowed history is still ark;
  synthetic success paths would not establish the missing restoration proof.
- oracle.rs: the extracted value phase has direct literal-offset/error tests.
  Existing full-domain oracle tests run in the slow gate; classical argument
  storage remains the r28 contract gap. Host allocation/exponent error paths
  are kept fail-loud rather than allocating unbounded fixtures for coverage.

Inherited verification record at the former ISA pin (not rerun for this change):

- timeout 180 cargo test --offline -j3 -- --test-threads=4: four library tests
  pass; integration has 223 passes, one pinned-reference failure and ten ignored.
  Measured command wall time is 53.01 seconds, integration test time 52.74 seconds.
  The failure is still exactly unknown field token_aliases at the original ISA
  pin; no test was skipped or re-pinned to conceal it.
- timeout 600 cargo test --offline -j3 -- --ignored --test-threads=4: all ten
  retained exhaustive simulations pass, in 181.62 seconds after phase splitting.
- Final line coverage: **88.06%**, 13,218 / 15,011 measured source lines.
  Command: timeout 600 cargo llvm-cov --offline -j3 --ignore-run-fail --json
  --summary-only --ignore-filename-regex "/tests/" --output-path
  .tmp/quality-sched/coverage-final.json -- --test-threads=4.
  Its test run retains the same one known failure; report generation succeeds.
- cargo fmt -- --check, python3 -B scripts/quality.py --check and all eight
  metrics tests pass. An actual temporary new source file makes the CLI ratchet
  fail for added mass and a process identifier; removing it restores the pass.
- Five structure tests now use literal nested-coordinate tables where applicable;
  three admission tests pin distinct rejection reasons, and the catalog digest
  test is explicitly packaging integrity, with semantic catalog coverage kept
  in named_interface and expansion.

Final least-covered modules are ffi.rs (69.79%), lib.rs (75.00%), target.rs
(77.08%), cleanup/replay.rs (81.86%) and source/oracle_call.rs (82.59%). The first
four retain the tests/reasons above. Source oracle-call binding is exercised by
the full small-domain/token-identity call tests; its remaining uncovered lines
are alternative invalid binding/overlap checks and uncommon invocation contexts,
rather than a wholly unexecuted source adapter.

The residual work is linked under bennett-sched-ogc to r28, xzv, 2u4, kbc, a0v,
ark and vt8. The reference metadata-admission failure no longer reproduces at
the current pin. Critical scheduler/gate review and the council round remain
the orchestrator responsibility; this record does not claim CLEAR or a green
push gate.

Historical pin-only verification (ISA `3b10907`; before the current feature changes):

- `timeout 120 cargo test --offline -j3 --test integration differential::emitted_09_schedule_and_certificate_pass_reference_metadata_admission -- --exact --test-threads=4`:
  one pass. The identical command at scheduler `c6879dd` and ISA `97e7b9f`
  failed with `unknown field token_aliases`.
- `timeout 600 cargo test --offline -j3 -- --test-threads=4`: four library
  tests and 224 integration tests pass; ten slow tests remain in the ignored
  suite. Integration execution took 50.03 seconds. Comparing all 238 named
  test outcomes against the previous pin shows only the intended admission
  failure becoming a pass.
- `timeout 600 cargo test --offline -j3 -- --ignored --test-threads=4`:
  nine pass, one fails in 142.13 seconds. The normative-region audit fails at
  `tests/normative_objects.rs:122` because its diagnostic assertion still
  requires `bennett-sched-ark`. This contradicts the inherited slow-green
  record above; no current green gate is claimed. The audit does not call the
  reference interpreter, and its §16.3 source text is byte-identical at both
  pins. The gate failure is tracked by **bennett-sched-el7**; this pin-only
  change leaves product code and tests untouched.
- `cargo fmt -- --check`, `python3 -B scripts/quality.py --check` and
  `timeout 120 python3 -I tests/pin_resolution.py -v` pass (14 pin tests).
- `timeout 120 python3 -B scripts/conformance.py --write` regenerated the
  inventory from the new gate pin: 557 cases and eight portable manifests.
  New K91 cases remain explicitly unexercised by direct fixture replay.


Current K84–K93 producer checkpoint, recorded 2026-11-08T12:11:08Z:

- Pin verification materialised published ISA 62a0cf0 through the main-checkout
  gate tool `verify`; it did not run a full gate. Spec digest and immutable
  catalog identity are checked at the new pin.
- `python3 -B .tmp/sched-vt8/guard.py moderate cargo test --offline -j8
  --test integration <filter> -- --test-threads=8` ran the recorded focused
  filters: borrow, cleanup::, source::, oracle_calls::, token_aliases::, ffi::,
  c_link::, pipeline::, context_reports::, native_models::, slot_fragments::,
  mcx_bundles::, canonical_cost::, conformance_inventory::, target:: and
  differential::. Results: 112 passes and one retained K89 reader failure.
  The cleanup filter leaves one existing slow simulation ignored; no new
  ignore/skip was added. These were focused checkpoints, not a full gate.
- The final K69/K70 lifetime refinement adds two red-first domain regressions:
  complete sequential widths and preceding callee lifetimes. The final `borrow`
  filter passes 12 tests, with the same one pinned K89 reader failure.
- Subsequent `pair_coverage::` passes three tests, including standalone
  zero-token inventory with primary/argument alias consistency.
  `qinplace::` passed all ten regressions after separating static inventory
  from executed workspace. `accepted_contracts::published_` passes three tests
  covering K84's three objects, K91's nine metadata objects and both source
  partitions. None establishes independent schedule or certificate replay.
- `scripts/conformance.py --write` inventories 559 cases and eight manifests.
  `--require-complete` still exits 2. K84 and K91 portable evidence is labelled
  metadata admission/source scheduling, not full checker conformance.
- The new native reader, entry analysis, exhaustive frame replay and static
  pair inventory add required behavior. The quality baseline is explicitly
  refreshed for that feature code and its adapter branches; the measurement
  script and regression rules are unchanged. Maximum function length remains
  298, over-300 functions remain 0, wrappers remain
  58, product process identifiers remain 0, and the tracked
  litter set is unchanged. No unrelated existing-file complexity allowance is
  increased. Changed per-file masses (before, after):
  `{"scripts/conformance.py": [91, 106], "src/borrow.rs": [54, 59], "src/borrow/entry.rs": [0, 75], "src/borrow/exhaustive.rs": [0, 58], "src/canonical.rs": [205, 210], "src/cleanup/raw.rs": [49, 51], "src/cleanup/replay.rs": [268, 274], "src/engine.rs": [316, 329], "src/native_model.rs": [0, 158], "src/oracle_calls.rs": [124, 126], "src/qinplace.rs": [35, 44], "src/qinplace/inventory.rs": [0, 82], "src/source.rs": [834, 839], "src/source/oracle_call.rs": [83, 91], "src/source/qinplace.rs": [108, 101], "src/source_ir.rs": [188, 187], "src/source_registry.rs": [97, 103], "src/target.rs": [399, 403]}`.

The current branch is **not green**: the unchanged independent metadata
admission test for selected K89 claims rejects `workspace` in the pinned
reference reader. This blocker is bennett-sched-vt8.4; the upstream request is
queued under COMMON14. Fixed-image production (vt8.3), general native-model
verification (vt8.1), remaining borrow mechanics (vt8.2),
large pair proofs (a0v), root class bindings (4tj), owner borrowing (qm2) and
classical pseudo-locations (r28) remain required. No complete conformance or independent K89 checker success is claimed.
The resumed fast-gate evidence is recorded below.

## Resumed published-contract adoption

The branch merged current origin/main and repinned gate.toml and
SPEC_VERSIONS.toml to published ISA dbc8845. Pin verification built its
reference binary and verified the committed spec digest. The vendored catalog
and the published catalog both have raw-byte SHA-256
`52dbc3f8f0c91d587fd933a43080385fd5284abcfec0ce84788229fcb15c53d8`.
The inventory now records 878 cases and eleven portable manifests; it remains
incomplete, including the newly published K85/K86/K93 manifests.

K91's previously missing query contract is now published and implemented.
The added Rust query regression failed with the missing export before the
implementation; it passes for exact JCS bytes, stable immutable storage,
concurrent queries, no request dependency and atomic NULL-output errors.
The linked C alias consumer queries before scheduling both partitions and
checks manifest lifetime after request destruction. Focused ffi (4), c_link
(2) and token_aliases (11) tests pass at the new pin.

The merged-base fast tier ran: 246 integration tests passed, two failed and
ten pre-existing slow tests remained ignored in the default tier. K89 failed
on the published reader's unknown `workspace` field. K88's unchanged class
search regression exposed an unintended change from homogeneous to mixed
placement. The scheduler now searches homogeneous assignments first and
falls back to complete per-bit assignments; all eight review regressions and
three slot-fragment tests pass, including a mixed-class native gate with
adequate capacity for either homogeneous allocation. That added fixture first
used an incorrect zero-control range for CX; it was corrected to one control
before interpreting its result. No existing test was changed or weakened.

The unchanged K89 regression also fails at the new published pin on the same
unknown-field diagnostic. This is a reader interoperability blocker under
vt8.4, not permission to omit the accepted field or claim independent replay.
vt8.1–vt8.3 and the other recorded producer/scaling obligations remain required.

The quality baseline is explicitly remeasured for the new query and complete
placement fallback: engine complexity mass 329 to 336; ffi mass 35 to 37.
Measurement and regression rules are unchanged. No test or quality check is
removed; all other per-file complexity allowances remain unchanged.

## Closed K89 occurrence adoption

The 5283697 pin publishes ni9's closed source/allocation occurrences. The
producer now uses those objects, complete invocation contexts and acquisition
ordinals independent of generated gate ordinals. Generated local-region
workspace and paired forward acquisition names through raw reversal are
covered. Oracle source-call attachment reconstructs borrow proofs after
binding their invocation paths; the raw-inverse test exposed an otherwise
stale callee-root context before this repair.

The root occurrence test first failed on the earlier path array; the generated
allocation test first failed on the explicit unsupported selector path. A new
nested-call fixture initially omitted the required space in its comma-separated
operand list; it was corrected before interpreting its result. Ten focused
borrow-frame semantic tests pass, including generated allocations, source call
contexts and raw inverse lifetimes. The original reference-admission assertion
is retained unchanged. Its current-pin result and the fast tier are recorded
separately after the pinned reader rebuild.

This new encoding and acquisition counting increases exhaustive.rs complexity
mass from 58 to 73 and the number of functions over 30 lines from 145 to 147.
The measurement/check implementation is unchanged; maximum function length,
over-300 count, wrapper count, identifiers and other file masses are unchanged.
These focused results do not attest to independent frame proof replay or full
conformance. The producer explicitly selects the original catalog hash; the
new published r1 default cannot rewrite that explicit selection.

<!-- quality-baseline -->
```json
{
  "complexity_mass": {
    "scripts/conformance.py": 106,
    "scripts/pinned_sibling.py": 15,
    "scripts/quality.py": 78,
    "src/admission.rs": 218,
    "src/borrow.rs": 59,
    "src/borrow/entry.rs": 75,
    "src/borrow/exhaustive.rs": 73,
    "src/canonical.rs": 210,
    "src/cleanup.rs": 184,
    "src/cleanup/raw.rs": 51,
    "src/cleanup/replay.rs": 274,
    "src/decimal.rs": 57,
    "src/engine.rs": 336,
    "src/engine/error_bounds.rs": 21,
    "src/expansion.rs": 930,
    "src/expansion_env.rs": 0,
    "src/ffi.rs": 37,
    "src/layout.rs": 77,
    "src/lib.rs": 0,
    "src/lproc.rs": 219,
    "src/native_model.rs": 158,
    "src/object.rs": 576,
    "src/oracle.rs": 270,
    "src/oracle_activation.rs": 15,
    "src/oracle_calls.rs": 126,
    "src/oracle_memory.rs": 42,
    "src/oracle_pairs.rs": 120,
    "src/oracle_state.rs": 113,
    "src/qinplace.rs": 44,
    "src/qinplace/inventory.rs": 82,
    "src/ref_contract.rs": 75,
    "src/request.rs": 16,
    "src/resources.rs": 112,
    "src/schedule.rs": 126,
    "src/source.rs": 839,
    "src/source/call_graph.rs": 102,
    "src/source/oracle_call.rs": 91,
    "src/source/qinplace.rs": 101,
    "src/source_ir.rs": 187,
    "src/source_registry.rs": 103,
    "src/target.rs": 403,
    "src/token_aliases.rs": 28
  },
  "function_lengths": {
    "count": 447,
    "maximum": 298,
    "over_100": 43,
    "over_30": 147,
    "over_300": 0
  },
  "litter": [
    ".beads/hooks/pre-push.backup"
  ],
  "process_identifiers": 0,
  "trivial_wrappers": 58
}
```
