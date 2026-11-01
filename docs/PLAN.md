# ISA 0.9 scheduler implementation and evidence

Contract: `bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md`, §11 and its
dependencies. Source checkout observed at `4a937982bd3bd2e9f71b0d1c6d90532609e53206`.
The published `ref` JSON is specification data, vendored in `data/`; no sibling
implementation code is linked or copied. Beads issue `bennett-sched-akl` owns
implementation tracking. This document maps requirements and records evidence;
it does not replace Beads.

## Milestones

1. Implemented and focused-tested: binary ABI, exact decimals and canonical document identity.
2. Partial: schema/selectors/binding and closed constructors implemented; K82 binding and all published binding witnesses implemented; full functional family replay remains qnl.
3. Partial: supported action traces, native/exact/approximate schemes and bound resource certificates implemented; ark tracks the remaining algorithms.
4. Audit performed: all ten object identities and the full controlled-action basis domain checked; every published case/manifest inventoried, remaining conformance tracked in qnl. Push protocol has a mock isolation test; the runner must verify the committed SHA.

## Requirement inventory

Clause identifiers below use the published `ISA-…` dependencies where provided;
K identifiers disambiguate subsequently ratified paragraphs in §11/§11.1.

| Clause | Required behavior | Evidence / state |
|---|---|---|
| §11 C declarations | Opaque owned requests/responses, all setters, copied buffers, status and typed OK/INFEASIBLE/UNSUPPORTED, reasons, accessors and frees | Exported; Rust ownership/null tests and C11 cdylib consumer (`ffi`, `c_link`) |
| ISA-11-P2 | Expand every row/instruction in specified source order; stable generated ordinals; removals only from library permission | Primitive action source expansion checked; oracle/lproc builders deferred: ark |
| ISA-11-P3 | JCS schedule; exact bindings; cleanup/gate/alloc/start entries, selectors and defaults | Strict schema and output binding tests; generated action schedules (`selectors`, `pipeline`) |
| ISA-11-P4, §8.3 | Interface/control placements, per-class vertices, lifetimes, adjacency, no routing | Default class/vertex placement, capacity and adjacency implemented; global search deferred (ark) |
| ISA-11-P5 / K52 | Invocation-chain prefixes; source-loop iteration, nested budget/step/phase; no `lift.call` edge | Nested coordinate resolver tested; action call source paths tested; oracle source binding deferred (ark) |
| ISA-11-P6 | Lexicographic specificity, maximal ties independent of document order and entry kind; defaults replace rather than inherit fields | Per-operation maximal ties and independent kinds tested, including K63 specificity neutrality |
| ISA-11-P7 / K55 | Admission/concrete validation stages, literal-only nested-budget derivation; exact rational decimal equivalence | Exact decimals implemented; linked source-path/request admission implemented; lproc/nested-budget concrete staging deferred (ark) |
| K63 | Fixed-instance and semantic-call-occurrence identity and selectors, without extra specificity | Selectors and typed semantic-call occurrence spaces implemented; lproc instance formation deferred (ark) |
| K64 | `$construct/$step/$commit/$release`, separate support ordinal spaces, B=0 construction/commit/release | Reserved point syntax accepted; source support emission requires oracle builder (enz/ark) |
| K65 | Non-emitting ordinal-0 whole-cleanup carriers, reverse/recompute selection before leaf expansion | Cleanup schema and matching kind implemented; carrier/pair formation deferred (ark) |
| K81 | Source occurrence/version cleanup partition, indivisible joint inverses, intrinsic boundaries, first-release ownership | Deferred explicitly (ark); no primitive-circuit cancellation is claimed as a source-call partition |
| K80 | Reverse retains written points; recompute replacement owns carrier-point contiguous ordinals and fresh semantic-call occurrences | Deferred explicitly (ark); no recompute schedule emitted |
| ISA-11.1-P1–P3 / K11 | Complete 219-recipe `ref@0.9`, exact envelope/hash, no witness hash substituted | `tests/canonical_cost.rs` catalog identity test |
| ISA-11.1-P4–P5 | Closed constructors, scalar expressions, loops, allocation identities, inverse traversal and retained ordinals | Constructor engine and inverse/identity tests; K82 named helpers implemented; 16 binding cases, 54 interfaces, 86 edge digests checked |
| K19 | Typed formals, capture-free bodies, positional wire binding, controls, linked symbols, gate/width/cycle validation | Typed binding, strict invoke scope, captured bodies, wires/angles and dependency cycles implemented; full source registry validation partial; K82 named interfaces import only declared by-name wire names |
| K82 | Named-interface recipes import exactly lexical wire bindings, typed recipes/templates stay closed, no non-wire fallback | All 16 published cases, 54 interfaces and 86 edge digests match (`named_interface`); catalog hash unchanged |
| K21 | Data-image byte order and unconditional X leaves | 8102 unconditional image witness and inverse machinery tested (`expansion`) |
| K22 | Indexed captured typed templates, apply-site controls, source row ordinals | Constructor support present; complete published template families remain unexercised (qnl) |
| K24 | Closure-bound argument vectors, exact iN/f64/token encodings, no implicit conversions | Value encodings/hash witness checked; constructor forwarding present; source token semantics deferred (ark) |
| K77 | Explicit per-call oracle token context, captured across adapters, distinct from positional arguments | Deferred source-binding channel (ark); no root budget substituted for nested token parameters |
| K25 | Ordered source-validated byte/activation/capture/state/frame descriptors | Generic typed records implemented; source descriptor formation and full forwarding validation deferred (ark) |
| K26 | Region shape, role, origin and ordered cells; reject same-width shape substitutions | Generic and source shape/role/origin forwarding implemented; all K79 address cases and 128 complete forward/inverse tuples checked |
| K27 | Class and allocation declarations, unconditional acquire/relinquish, shared lifetime state, fresh reacquisition | Acquire/relinquish, fresh reacquisition and inactive scope exit tests; translation-lemma source borrow frames and separate certificate records implemented; other T3 frame proofs remain open (ark) |
| K28 / K62 | qinplace t/f release sequence and source action adjoint before expansion | Primitive action-call adjoint path present; qinplace and complete source replacements deferred (ark) |
| ISA-11.1-P29 | Forward leaf ordinals before transforms/schemes, callee spaces, inverse retains identity, all ref removable sets empty | Bound qxor ordinals, inverse retention and empty removable sets tested; full source spaces deferred (ark) |
| §11 obligations / §6 | Cleanup windows, exact source versions/control sets, no measurement cleanup | Source semantic-call/intrinsic version matching, matched dependency windows and sufficient composed-use factorisation implemented; general factorisation and pair/carrier realisation remain open (ark) |
| §8.1 / K9 | Closed cost AST, exact rationals, epsilon decade/log2, context variable rules | Exact AST/context/decade/log2 evaluation tested against published E vectors; imported model/version/resource quantity mapping checked directly against E-imports |
| §8.2 / K12 | Count once per native leaf, makespan, per-class occupancy/peak, borrowed rate replacement, exact rows and upward 34-digit displays | `tests/canonical_cost.rs` decimal witnesses |
| §8.3–8.4 | First matching native entry/scheme, own+effective controls, ancillae, capacity, accuracy and topology | Native matching, exact/approximate scheme error composition, capacity, adjacency and sequential timing tested; global search deferred (ark) |
| §9.1–9.2 | Canonical object parsing, signatures, semantic envelopes/SCC hashes and object/params/schedule identities | All ten normative object interface hashes, SCC envelopes and controlled fixture object hash checked; complete source semantic admission remains partial |
| §9.4 | Complete typed frame descriptor: slots/scratch/history/borrow/ancillae and live ranges | Supported action slot/scratch/borrow/ancilla records emitted; history deferred (ark), varying context frames require lzv contract decision |
| §12.1–12.3 | Bound certificate, realised multiplicities, claims with honest evidence, local/inclusive errors and diamond composition | Binding/resources/numerical claim emission tested for supported traces; rigorous checker replay and general composition proof remain open (ark/qnl) |
| §17 items 7–9, 15, 18 | Published default ref library, cleanup defaults, accuracy defaults, placement, full history | Ref identity, exact finite shares and placement checked; oracle full-history expansion deferred (ark) |

## Gaps and evidence limits

The published C header still labels itself 0.8, while its §11 declarations match
the 0.9 spec. Consumers must use the 0.9 document for semantics. No contract change
is inferred from the comment. Reference CLI schedule execution/target estimation
is documented as staged; admission and semantic execution are distinct evidence.
Every unavailable vector and postponed algorithm will be recorded with its Beads
issue and exact missing behavior. No passing test alone establishes all of §11.

## Resolved helper contract — bennett-sched-enz

Accepted K82 in ISA commit `4a937982bd3bd2e9f71b0d1c6d90532609e53206`
settles the named helper interface. The `ref` catalog and envelope hash are
unchanged. The rule is isolated in `src/ref_contract.rs`; it imports exactly
the named wire interface at an invoke site and retains closure for typed
recipes/templates. `tests/named_interface.rs` directly checks every published
binding case, interface and expansion digest. The tests also exposed inactive
declaration use and stale inverse scratch reuse; both are fixed. The earlier
free-wire diagnostic is no longer an expansion blocker.

## Frame schema gap — bennett-sched-lzv

One linked flip closure can be called with its mut:0 slot on class A and on
class B in the same legal root. §12.3 currently supplies one concrete §9.4 frame
per closure, so neither descriptor covers both invocations. The mixed-class
regression returns explicit UNSUPPORTED instead of misreporting one frame.
The exact proposed `frame_contexts` schema and complete-context selection rule
are queued in `.tmp/cross-repo-beads.md` for upstream filing under COMMON rule 14.
No unratified certificate field is emitted. Independent work continues.

## Postponed mechanics and conformance limits

`bennett-sched-ark` tracks oracle/lproc source state and fixed-instance
builders, nested-budget concrete validation, K65/K80/K81 cleanup partition,
carriers/recompute, qinplace coverage, general factorisation and non-translation
borrow T3 proofs, nested local-region declaration coordinates and global
placement/objective search. These are postponed algorithms with specific
UNSUPPORTED reasons, not permanently excluded capabilities. Malformed source
and target inputs produce argument/target errors. Ordinary supported actions
have tested OK paths.

This continuation implements padded while loops, their source action adjoints
and literal double adjoints, local region image lifetimes, coherent memory,
source region forwarding, linked path/request admission, translation-lemma
borrow frames, source atomic call/intrinsic version matching, safe restored
dependency windows, region-cell window closure, sufficient exact factorisation
of composed uses, imported quantities, approximate schemes, ordered zero-time
peaks and child scheme ancilla accounting. The general algorithms above remain
required. Structural and numerical certificate entries are producer claims;
independent checker replay is not claimed.

`bennett-sched-qnl` tracks the remaining conformance. The inventory has 519 A–H
cases: 12 direct fixture mappings, 17 equivalent witness mappings, 20 partial ref
families, 5 other scheduler vectors, and 465 executor/checker vectors not replayed
by this scheduler. Five portable manifests are recorded: K79's 12 source address
cases (4 accepted, 8 rejected) and all 128 forward/inverse tuples, K82's 16 binding
cases/54 interfaces/86 digests pass; K78/K80/K81 remain open. Original 0.8 source
bodies in K79 are re-enveloped and rehashed for 0.9 without changing instructions.
Equivalent witnesses do not claim direct original object/request replay.
Every source hash is recorded in `docs/conformance-report.json`; the inventory
regression checks missing/stale entries. Functional correctness of the full ref
catalog does not follow from its binding digests.

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
