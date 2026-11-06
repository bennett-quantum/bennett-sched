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
