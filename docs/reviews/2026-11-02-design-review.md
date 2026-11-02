# bennett-sched design review — 2026-11-02 (PARTIAL)

Review against ISA 0.9 §11 at pinned revision `0469741`. Stopped early for a network
outage. Source has since moved (at least `fced0ad`); findings were re-verified at
HEAD `7f9ceb5` (below) and filed as beads. Coverage is incomplete:
`src/oracle.rs`, `src/expansion.rs` and `src/cleanup.rs` were not read in depth.

Probes: `.tmp/opus-reviews/probe/tests/probe.rs` (red tests named below, not committed;
`.tmp` is ignored). Existing suite at the pin: all green.

**Re-verified 2026-11-02 at HEAD `7f9ceb5`** (probe crate pointed at a worktree at that sha, `cargo test --offline -j2 --test probe`): S1-S7 all still red, controls s1, s2, s4 green; s4 took 116 s. One bead per finding, ids in the headings.

## Findings

### S1 (bennett-sched-3k1 P1). `adj` call rewrites the caller's loop coordinate
`remap_adjoint_iterations` (`src/source.rs`) rewrites every path element, including those
of callers outside the adjoint. Witness: `qloop 3 { adj action_call @flip (t) () () () }`.
Caller iteration in execution order is `[2],[1],[0]`, expected `[0],[1],[2]` (§11 Matching).
The forward-call control passes. Selectors emitted for such an operation therefore address
a different iteration. Probe: `s1_adjoint_call_keeps_caller_loop_iteration`.

### S2 (bennett-sched-mi3 P2). A callee with scratch cannot be called twice
Two `action_call @sc` to a callee with `qalloc` return UNSUPPORTED ("context-varying frame
descriptors"), though both contexts are identical; one call is OK. The check in
`engine::certificate` rejects any repeat invocation owning an allocation. This blocks
repeated subroutines generally. The reason cites `ark`, which does not describe this case
precisely. Probe: `s2_scratch_callee_called_twice_schedules`.

### S3 (bennett-sched-io7 P2). Interface class chosen by search is not bound by the schedule
Target with classes a, b and `x` only on b: outcome OK, `entries=[]`, frame slot class b.
Without topology the document has nowhere to bind a slot class, and §11's default is the
first class, a. bennett-estimate at `519b045` rejects the same request
(`E_GATE_UNSUPPORTED`), so the two tools disagree. May need a contract decision.
Probe: `s3_interface_class_is_reproducible_from_schedule`.

### S4 (bennett-sched-2qd P2). Infeasible placement search does not terminate in practice
`engine::run` retries every placement and rebuilds the trace each time. 8-wire interface,
12 vertices, no edges, one CX: no outcome within 30 s. Even the 2-wire control took most of
an 88 s debug test run. Probe: `s4_infeasible_topology_terminates`.

### S5 (bennett-sched-qm2 P2, contestable). Borrowed wires are charged as new wires
Caller with idle `y` calling a borrowing callee, capacity 3: INFEASIBLE. Capacity 8,
dirty_rate 0.5: peak 4 and occupancy 14; my reading of §8.2 ("replacing the owner's charge")
and §9.4 gives peak 3 and occupancy 10. The source has no operand naming the idle owner, so
this may be a contract gap rather than a code bug. Estimate agrees with sched on root-level
borrow rows, so agreement is not evidence here. Probes: `s5_*`.

### S6 (bennett-sched-1na P3, contestable). Callee report depends on call order
`regions[h].resources` is overwritten per invocation: time 100 or 1 for the same two calls
(controlled and uncontrolled) in different order. §12.3 keeps one map per closure and does
not say which context it reports. Probe: `s6_callee_report_independent_of_call_order`.

### S7 (bennett-sched-760 P3). `validate_request` is not full admission
It admits duplicate maximal root entries (K55 requires rejection) and an alloc entry naming an
undeclared class, while PLAN.md describes admission as implemented. Sched mostly produces rather
than consumes schedules, hence the priority. Probes: `s7_*`.

## Known gaps confirmed, already tracked
Lproc/layout oracles and oracle calls from actions return UNSUPPORTED (`bennett-sched-ark`);
classical pseudo-location reads are blocked on `b-isa-spec-9y4`. Examples 2, 7 and 10 do not
schedule, which keeps the north-star path blocked.

## Differential
39-case corpus and the ten normative objects against bennett-estimate: rows agreed wherever
both produced them, one corpus difference was an invalid witness of mine and one both refused
with different codes. Both have the same author, so this does not rule out shared misreadings.
