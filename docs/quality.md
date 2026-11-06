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
