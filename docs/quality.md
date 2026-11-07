# Scheduler quality evidence

Evidence is against the ISA revision in `gate.toml`
(`97e7b9f16977905be8308c2ca33b9d470f83756f`). This is a remediation record,
not a claim that all published capabilities or quality targets are complete.

The reference metadata-admission differential remains active in the default
suite. The reference interpreter at this pin rejects `bindings.token_aliases`;
upstream **b-isa-spec-3wl9** owns that failure. Only the orchestrator advances
the reviewed ISA pin. A coverage run that continues after this failure is
measurement, not a successful test run.

Inherited review work:
- **bennett-sched-fdu**: valid layout fixtures; three adjoint tests pass and
  disabling replacement expansion fails all three on primitive ordering.
- **bennett-sched-jez**: capacity conflicts retain live class dependencies;
  a deterministic thirty-released-choice regression fails before the fix.
- **bennett-sched-b08**: exact published shares, costs, error categories/reasons
  and fixed grid placement verdicts are checked. A changed native duration
  fails the published literal-time assertion.
- **bennett-sched-nsy**: inventory regenerated from the gate pin; its complete
  case and manifest hash test passes (536 cases, seven manifests).
- **bennett-sched-xzv**: setter/identity tests and actual C linkage pass.
  Section 10.1 still needs the precise negotiation ABI (symbol/signature/result).
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
of the pinned commit. README and the conformance inventory now attribute the
current reference failure instead of repeating the old successful-admission
claim. Ten oracle-pair tests pass after diagnostic cleanup.

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

Final verification (Rust source unchanged through d066757):

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
ark and vt8. The known upstream reference issue is b-isa-spec-3wl9. Critical
scheduler/gate review and the council round remain the orchestrator responsibility;
this record does not claim CLEAR or a green push gate.

<!-- quality-baseline -->
```json
{
  "complexity_mass": {
    "scripts/conformance.py": 91,
    "scripts/pinned_sibling.py": 15,
    "scripts/quality.py": 78,
    "src/admission.rs": 218,
    "src/borrow.rs": 54,
    "src/canonical.rs": 205,
    "src/cleanup.rs": 184,
    "src/cleanup/raw.rs": 49,
    "src/cleanup/replay.rs": 268,
    "src/decimal.rs": 57,
    "src/engine.rs": 316,
    "src/engine/error_bounds.rs": 21,
    "src/expansion.rs": 930,
    "src/expansion_env.rs": 0,
    "src/ffi.rs": 35,
    "src/layout.rs": 77,
    "src/lib.rs": 0,
    "src/lproc.rs": 219,
    "src/object.rs": 576,
    "src/oracle.rs": 270,
    "src/oracle_activation.rs": 15,
    "src/oracle_calls.rs": 124,
    "src/oracle_memory.rs": 42,
    "src/oracle_pairs.rs": 120,
    "src/oracle_state.rs": 113,
    "src/qinplace.rs": 35,
    "src/ref_contract.rs": 75,
    "src/request.rs": 16,
    "src/resources.rs": 112,
    "src/schedule.rs": 126,
    "src/source.rs": 834,
    "src/source/call_graph.rs": 102,
    "src/source/oracle_call.rs": 83,
    "src/source/qinplace.rs": 108,
    "src/source_ir.rs": 188,
    "src/source_registry.rs": 97,
    "src/target.rs": 399,
    "src/token_aliases.rs": 28
  },
  "function_lengths": {
    "count": 423,
    "maximum": 298,
    "over_100": 42,
    "over_30": 131,
    "over_300": 0
  },
  "litter": [
    ".beads/hooks/pre-push.backup"
  ],
  "process_identifiers": 0,
  "trivial_wrappers": 58
}
```
