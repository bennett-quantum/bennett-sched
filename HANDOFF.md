# Handoff

State: ISA 0.9 partial scheduler implementation. All §11 C symbols, owned
request/response buffers, typed outcomes, canonical documents, exact costs,
selectors, closed reference constructors, object identities, primitive action
schedules and bound resource certificates are implemented. See docs/PLAN.md
for clause-by-clause coverage. No complete scheduler/conformance claim.

Main issue `bennett-sched-akl` remains in progress. P0 `bennett-sched-enz`
records the published catalog's free-wire binding contradiction and the exact
proposed normative edit. Required upstream `bd create` was attempted in
`bennett-isa-spec` and failed with `embeddeddolt ... openat LOCK: read-only file
system`. No upstream file was changed. An upstream contract decision is needed;
do not invent dynamic caller scope or change the bound library silently.

`bennett-sched-ark` records each postponed source/cleanup/frame/accuracy
algorithm. `bennett-sched-qnl` tracks the remaining conformance work. The
inventory covers 519 cases, ten normative objects and four portable manifests;
coverage mappings are not blanket pass verdicts. The installed reference's
0.9 admission probe exits 1 with `§1: expected isa=0.8`; its 0.8 compatibility
controlled-action fixture serves the complete four-input differential test.

Verification entry point: `scripts/pre-push --working-tree` (headless,
offline, four compiler/test threads, Rust tests, a linked C11 consumer, gate
protocol regression, formatting). `python3 scripts/conformance.py --write
--probe-reference --require-complete` records the inventory and exits 2 for
remaining vectors. Final command results are recorded in .tmp/PROGRESS.md.

The runner owns commits. No git command was run by the implementation agent,
and no commit SHA is claimed here. Default `scripts/pre-push` must still be run
by the runner on the committed pushed SHA: the mocked protocol regression
checks selection/isolation, not an actual committed snapshot. Changed paths:
src, tests, data, docs, scripts, Cargo.toml/Cargo.lock, README.md, HANDOFF.md,
SPEC_VERSIONS.toml and .beads/issues.jsonl. Hooks and worklog were untouched.
