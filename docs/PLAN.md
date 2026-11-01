# ISA 0.9 scheduler implementation and evidence

Contract: `bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md`, §11 and its
dependencies. Source checkout observed at `16376717b9ff7ea288456f19e2958740b31ecbbd`.
The published `ref` JSON is specification data, vendored in `data/`; no sibling
implementation code is linked or copied. Beads issue `bennett-sched-akl` owns
implementation tracking. This document maps requirements and records evidence;
it does not replace Beads.

## Milestones

1. Implemented and focused-tested: binary ABI, exact decimals and canonical document identity.
2. Partial: schema/selectors/binding and closed constructors implemented; full library blocked by enz.
3. Partial: supported action traces, native/exact schemes and bound resource certificates implemented; ark tracks the remaining algorithms.
4. Audit performed: all ten object identities and the full controlled-action basis domain checked; every published case/manifest inventoried, remaining conformance tracked in qnl. Push protocol has a mock isolation test; the runner must verify the committed SHA.

## Requirement inventory

Clause identifiers below use the published `ISA-…` dependencies where provided;
K identifiers disambiguate subsequently ratified paragraphs in §11/§11.1.

| Clause | Required behavior | Evidence / state |
|---|---|---|
| §11 C declarations | Opaque owned requests/responses, all setters, copied buffers, status and typed OK/INFEASIBLE/UNSUPPORTED, reasons, accessors and frees | Exported; Rust ownership/null tests and C11 cdylib consumer (`ffi`, `c_link`) |
| ISA-11-P2 | Expand every row/instruction in specified source order; stable generated ordinals; removals only from library permission | Primitive action source expansion checked; oracle/lproc builders blocked/deferred: enz/ark |
| ISA-11-P3 | JCS schedule; exact bindings; cleanup/gate/alloc/start entries, selectors and defaults | Strict schema and output binding tests; generated action schedules (`selectors`, `pipeline`) |
| ISA-11-P4, §8.3 | Interface/control placements, per-class vertices, lifetimes, adjacency, no routing | Default class/vertex placement, capacity and adjacency implemented; global search deferred (ark) |
| ISA-11-P5 / K52 | Invocation-chain prefixes; source-loop iteration, nested budget/step/phase; no `lift.call` edge | Nested coordinate resolver tested; action call source paths tested; oracle source binding deferred (ark) |
| ISA-11-P6 | Lexicographic specificity, maximal ties independent of document order and entry kind; defaults replace rather than inherit fields | Per-operation maximal ties and independent kinds tested, including K63 specificity neutrality |
| ISA-11-P7 / K55 | Admission/concrete validation stages, literal-only nested-budget derivation; exact rational decimal equivalence | Exact decimals implemented; admission/concrete source-domain staging deferred (ark) |
| K63 | Fixed-instance and semantic-call-occurrence identity and selectors, without extra specificity | Selectors and typed semantic-call occurrence spaces implemented; lproc instance formation deferred (ark) |
| K64 | `$construct/$step/$commit/$release`, separate support ordinal spaces, B=0 construction/commit/release | Reserved point syntax accepted; source support emission requires oracle builder (enz/ark) |
| K65 | Non-emitting ordinal-0 whole-cleanup carriers, reverse/recompute selection before leaf expansion | Cleanup schema and matching kind implemented; carrier/pair formation deferred (ark) |
| K81 | Source occurrence/version cleanup partition, indivisible joint inverses, intrinsic boundaries, first-release ownership | Deferred explicitly (ark); no primitive-circuit cancellation is claimed as a source-call partition |
| K80 | Reverse retains written points; recompute replacement owns carrier-point contiguous ordinals and fresh semantic-call occurrences | Deferred explicitly (ark); no recompute schedule emitted |
| ISA-11.1-P1–P3 / K11 | Complete 219-recipe `ref@0.9`, exact envelope/hash, no witness hash substituted | `tests/canonical_cost.rs` catalog identity test |
| ISA-11.1-P4–P5 | Closed constructors, scalar expressions, loops, allocation identities, inverse traversal and retained ordinals | Constructor engine and inverse/identity tests; full catalog blocked by undeclared helpers (enz) |
| K19 | Typed formals, capture-free bodies, positional wire binding, controls, linked symbols, gate/width/cycle validation | Typed binding, strict invoke scope, captured bodies, wires/angles and dependency cycles implemented; full source registry validation partial |
| K21 | Data-image byte order and unconditional X leaves | 8102 unconditional image witness and inverse machinery tested (`expansion`) |
| K22 | Indexed captured typed templates, apply-site controls, source row ordinals | Constructor support present; complete published template families remain unexercised (qnl) |
| K24 | Closure-bound argument vectors, exact iN/f64/token encodings, no implicit conversions | Value encodings/hash witness checked; constructor forwarding present; source token semantics deferred (ark) |
| K77 | Explicit per-call oracle token context, captured across adapters, distinct from positional arguments | Deferred source-binding channel (ark); no root budget substituted for nested token parameters |
| K25 | Ordered source-validated byte/activation/capture/state/frame descriptors | Generic typed records implemented; source descriptor formation and full forwarding validation deferred (ark) |
| K26 | Region shape, role, origin and ordered cells; reject same-width shape substitutions | Generic shape/role/origin binding present; source region adapters deferred (ark) |
| K27 | Class and allocation declarations, unconditional acquire/relinquish, shared lifetime state, fresh reacquisition | Acquire/relinquish, fresh reacquisition and inactive scope exit tests; source borrowed frames deferred (ark) |
| K28 / K62 | qinplace t/f release sequence and source action adjoint before expansion | Primitive action-call adjoint path present; qinplace and complete source replacements deferred (ark) |
| ISA-11.1-P29 | Forward leaf ordinals before transforms/schemes, callee spaces, inverse retains identity, all ref removable sets empty | Bound qxor ordinals, inverse retention and empty removable sets tested; full source spaces deferred (ark) |
| §11 obligations / §6 | Cleanup windows, exact source versions/control sets, no measurement cleanup | Primitive matched inverse cleanup and read-only/predicate checks; general factorisation/windows and call-pair proofs deferred (ark) |
| §8.1 / K9 | Closed cost AST, exact rationals, epsilon decade/log2, context variable rules | Exact AST/context/decade/log2 evaluation tested against published E vectors; imported-model quantity conversion remains open (ark) |
| §8.2 / K12 | Count once per native leaf, makespan, per-class occupancy/peak, borrowed rate replacement, exact rows and upward 34-digit displays | `tests/canonical_cost.rs` decimal witnesses |
| §8.3–8.4 | First matching native entry/scheme, own+effective controls, ancillae, capacity, accuracy and topology | Native matching and exact schemes, capacity, adjacency and sequential timing tested; approximate schemes/global search deferred (ark) |
| §9.1–9.2 | Canonical object parsing, signatures, semantic envelopes/SCC hashes and object/params/schedule identities | All ten normative object interface hashes, SCC envelopes and controlled fixture object hash checked; complete source semantic admission remains partial |
| §9.4 | Complete typed frame descriptor: slots/scratch/history/borrow/ancillae and live ranges | Supported action slot/scratch/ancilla records emitted; history, borrow and varying context frames deferred (ark) |
| §12.1–12.3 | Bound certificate, realised multiplicities, claims with honest evidence, local/inclusive errors and diamond composition | Binding/resources/numerical claim emission tested for supported traces; rigorous checker replay and general composition proof remain open (ark/qnl) |
| §17 items 7–9, 15, 18 | Published default ref library, cleanup defaults, accuracy defaults, placement, full history | Ref identity, exact finite shares and placement checked; oracle full-history expansion deferred (enz/ark) |

## Gaps and evidence limits

The published C header still labels itself 0.8, while its §11 declarations match
the 0.9 spec. Consumers must use the 0.9 document for semantics. No contract change
is inferred from the comment. Reference CLI schedule execution/target estimation
is documented as staged; admission and semantic execution are distinct evidence.
Every unavailable vector and postponed algorithm will be recorded with its Beads
issue and exact missing behavior. No passing test alone establishes all of §11.

## Determining contract gap — bennett-sched-enz (P0)

The published catalog contains these records (shown without their large bodies):

```text
bind.value.add.parameters = [N:integer, a:wire[N], b:wire[N], t:wire[N]]
bind.value.add.body       = [{"invoke":"value.add","args":["N"]}]
value.add.parameters      = ["N"]
value.add.body            = … gates reading a, b and t …
qxor.parameters          = ["N"]
qxor.body                = … gates reading s and t …
```

K19 requires rejection of unbound names and says there are no implicit bindings
or defaults. Named helpers are not captured body/template actuals. Their free
wires therefore require an additional binding rule or explicit formals. A valid
envelope hash proves byte identity, not a missing lexical binding semantics.
`expansion::undeclared_helper_operands_are_reported_with_the_contract_gap`
reproduces the error without loading any sibling implementation.

Proposed exact normative insertion after K19 positional invoke binding:

> A named recipe body is closed over its declared parameters and its own lexical
> declarations; invoke does not inherit caller bindings except through explicit
> actuals. Every recipe in the bound library MUST satisfy this closure condition.

Corresponding exact catalog change: replace every integer-only helper's formal
list with its complete ordered typed free-variable interface and add those
explicit actuals to each invoke. Regenerate the catalog and publish the resulting
envelope hash in §11.1 and normative fixtures. Add a closed-binding
`bind.value.add` conformance vector. An alternative retaining the accepted hash
must explicitly specify the existing helpers' free-variable binding; the
scheduler cannot choose that rule independently.

Required upstream filing was attempted with `bd create` from the ISA repo, but
failed before creation with:

```text
failed to open database: embeddeddolt: init schema: embeddeddolt: open db:
failed to load database "b_isa_spec": openat LOCK: read-only file system
```

No upstream file was modified. The local P0 issue preserves the exact requested
change for transfer upstream when writable. This is an unresolved contract
choice, not permission to remove arithmetic or oracle scheduling from scope.

## Postponed mechanics and conformance limits

`bennett-sched-ark` names the missing oracle/lproc source state and fixed-instance
builders, K55 admission/concrete staging, K65/K80/K81 cleanup carriers and
recompute, qinplace coverage, borrowed source frames, local region adapters,
while flags, general factorisation/window proofs, context-varying frame
reports, approximate scheme error allocation, imported-model conversion and
global placement/objective search. Known encountered features produce a named,
tracked UNSUPPORTED reason. Unknown or malformed input is an argument/target
error; ordinary supported actions have a tested OK path. No generic
UNSUPPORTED default represents the required capabilities as permanently absent.

`bennett-sched-qnl` owns completion of independent conformance. The current
inventory has 519 A–H cases: 11 mapped direct fixture tests, 11 mapped equivalent
witness tests, 20 partial/blocked ref families, 7 remaining scheduler vectors and
470 executor/checker vectors not replayed by this scheduler. Equivalent
witnesses do not assert direct replay of the original object/request. Four
portable manifests contain 23 vectors; they are all explicitly unexercised.
Every case and manifest has its source SHA-256 recorded in
`docs/conformance-report.json`; a regression checks for missing/stale entries.

All ten §16.3 object interface hashes are tested, including cyclic `sum` and the
layout/path envelopes. The published controlled-action object hash, frame-bound
resource rows, exact-scheme expansion and all four basis configurations are
checked. Differential execution uses only the published reference binary and
its 0.8 compatibility fixture. Its attempt to admit the actual 0.9 object exits
1 with `§1: expected isa=0.8`. It supplies no 0.9 schedule/target/checker verdict.
Numerical certificate claims emitted here remain producer claims for independent
checking; no proof/evidence label or rigorous interval-checker success is claimed.

The full working-tree gate is `scripts/pre-push --working-tree`. The default
hook gates each stdin local SHA in an isolated worktree beside the repo. Its
protocol/isolation regression uses mock executables, never real git. The
implementation agent is forbidden to run git; the runner must run the default
hook after committing. Working-tree checks and the mock regression do not
substitute for that actual committed-SHA verification.
