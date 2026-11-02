# ISA 0.9 scheduler implementation and evidence

Contract: `bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md`, §11 and its
dependencies. Source checkout observed at `ecaaac060c44e317d1181345fa6b8e38c096c099`; specification and catalog bytes are unchanged from the accepted K82 pin.
The published `ref` JSON is specification data, vendored in `data/`; no sibling
implementation code is linked or copied. Beads issue `bennett-sched-akl` owns
implementation tracking. This document maps requirements and records evidence;
it does not replace Beads.

## Milestones

1. Implemented and focused-tested: binary ABI, exact decimals and canonical document identity.
2. Partial: schema/selectors/binding and closed constructors implemented; K82 binding and all published binding witnesses implemented; full functional family replay remains qnl.
3. Partial: memoryless oracle construction, source cleanup carriers/recompute and supported action traces, native/exact/approximate schemes and bound resource certificates implemented; ark tracks the remaining algorithms.
4. Audit performed: all ten object identities and the full controlled-action basis domain checked; every published case/manifest inventoried, remaining conformance tracked in qnl. Push protocol has a mock isolation test; the runner must verify the committed SHA.

## Requirement inventory

Clause identifiers below use the published `ISA-…` dependencies where provided;
K identifiers disambiguate subsequently ratified paragraphs in §11/§11.1.

| Clause | Required behavior | Evidence / state |
|---|---|---|
| §11 C declarations | Opaque owned requests/responses, all setters, copied buffers, status and typed OK/INFEASIBLE/UNSUPPORTED, reasons, accessors and frees | Exported; Rust ownership/null tests and C11 cdylib consumer (`ffi`, `c_link`) |
| ISA-11-P2 | Expand every row/instruction in specified source order; stable generated ordinals; removals only from library permission | Primitive action source expansion checked; memoryless oracle state/row/commit/history construction implemented; layout address/load/bounds-check root rows implemented; lproc and nested-call builders remain ark |
| ISA-11-P3 | JCS schedule; exact bindings; cleanup/gate/alloc/start entries, selectors and defaults | Strict schema and output binding tests; generated action schedules (`selectors`, `pipeline`) |
| ISA-11-P4, §8.3 | Interface/control placements, per-class vertices, lifetimes, adjacency, no routing | Default class/vertex placement, capacity and adjacency implemented; finite class/per-bit vertex search implemented; cost optimisation remains open (ark) |
| ISA-11-P5 / K52 | Invocation-chain prefixes; source-loop iteration, nested budget/step/phase; no `lift.call` edge | Nested coordinate resolver tested; action call source paths tested; memoryless oracle step/phase/support coordinates and reverse ordinals checked; nested oracle calls remain ark |
| ISA-11-P6 | Lexicographic specificity, maximal ties independent of document order and entry kind; defaults replace rather than inherit fields | Per-operation maximal ties and independent kinds tested, including K63 specificity neutrality |
| ISA-11-P7 / K55 | Admission/concrete validation stages, literal-only nested-budget derivation; exact rational decimal equivalence | Exact decimals implemented; linked source-path/request admission implemented; lproc concrete instance admission implemented; nested-budget/complete generated domains remain ark |
| K63 | Fixed-instance and semantic-call-occurrence identity and selectors, without extra specificity | Selectors and typed semantic-call occurrence spaces implemented; Finite positional lproc closure, row ranges, location IDs and activations formed; concrete selector keys validated; row emission remains ark |
| K64 | `$construct/$step/$commit/$release`, separate support ordinal spaces, B=0 construction/commit/release | All four support points emitted for memoryless oracles, including B=0; forward/reverse step coordinates and shared Rows/Next ordinals checked (`oracle`) |
| K65 | Non-emitting ordinal-0 whole-cleanup carriers, reverse/recompute selection before leaf expansion | Carriers formed from source version matches; all selections stage before mutation; reverse and primitive/intrinsic recompute checked (`cleanup`) |
| K81 | Source occurrence/version cleanup partition, indivisible joint inverses, intrinsic boundaries, first-release ownership | Semantic calls remain atomic; intrinsic boundaries and first-release ownership of joint inverses checked; broader source forms remain ark |
| K80 | Reverse retains written points; recompute replacement owns carrier-point contiguous ordinals and fresh semantic-call occurrences | Primitive/intrinsic recompute owns contiguous carrier ordinals and omits original loop coordinates; fresh semantic-call contexts/history replay remain ark |
| ISA-11.1-P1–P3 / K11 | Complete 219-recipe `ref@0.9`, exact envelope/hash, no witness hash substituted | `tests/canonical_cost.rs` catalog identity test |
| ISA-11.1-P4–P5 | Closed constructors, scalar expressions, loops, allocation identities, inverse traversal and retained ordinals | Constructor engine and inverse/identity tests; K82 named helpers implemented; 16 binding cases, 54 interfaces, 86 edge digests checked |
| K19 | Typed formals, capture-free bodies, positional wire binding, controls, linked symbols, gate/width/cycle validation | Typed binding, strict invoke scope, captured bodies, wires/angles and dependency cycles implemented; full source registry validation partial; K82 named interfaces import only declared by-name wire names |
| K82 | Named-interface recipes import exactly lexical wire bindings, typed recipes/templates stay closed, no non-wire fallback | All 16 published cases, 54 interfaces and 86 edge digests match (`named_interface`); catalog hash unchanged |
| K21 | Data-image byte order and unconditional X leaves | 8102 unconditional image witness and inverse machinery tested (`expansion`) |
| K22 | Indexed captured typed templates, apply-site controls, source row ordinals | Constructor support present; complete published template families remain unexercised (qnl) |
| K24 | Closure-bound argument vectors, exact iN/f64/token encodings, no implicit conversions | Value encodings/hash witness checked; finite lproc keys and resolved token/classical/region forwarding checked independently of runtime values; nested constructor calls remain ark; pseudo-location storage requires r28 |
| K77 | Explicit per-call oracle token context, captured across adapters, distinct from positional arguments | Deferred source-binding channel (ark); no root budget substituted for nested token parameters |
| K25 | Ordered source-validated byte/activation/capture/state/frame descriptors | Generic typed records implemented; source-derived State/frame sizing and ordered byte/location maps implemented, including layout counters, sparse images and pinned table initialization; memoryless adapter consumes this plan; activation/capture metadata and concrete call targets implemented; lproc row emission remains ark |
| K26 | Region shape, role, origin and ordered cells; reject same-width shape substitutions | Generic and source shape/role/origin forwarding implemented; all K79 address cases and 128 complete forward/inverse tuples checked |
| K27 | Class and allocation declarations, unconditional acquire/relinquish, shared lifetime state, fresh reacquisition | Acquire/relinquish, fresh reacquisition and inactive scope exit tests; translation-lemma source borrow frames and separate certificate records implemented; other T3 frame proofs remain open (ark) |
| K28 / K62 | qinplace t/f release sequence and source action adjoint before expansion | Primitive action-call adjoint path present; qinplace and complete source replacements deferred (ark) |
| ISA-11.1-P29 | Forward leaf ordinals before transforms/schemes, callee spaces, inverse retains identity, all ref removable sets empty | Bound qxor ordinals, inverse retention and empty removable sets tested; full source spaces deferred (ark) |
| §11 obligations / §6 | Cleanup windows, exact source versions/control sets, no measurement cleanup | Source semantic-call/intrinsic version matching, matched dependency windows and sufficient composed-use factorisation implemented; general factorisation and fresh-call/history recompute remain open (ark) |
| §8.1 / K9 | Closed cost AST, exact rationals, epsilon decade/log2, context variable rules | Exact AST/context/decade/log2 evaluation tested against published E vectors; imported model/version/resource quantity mapping checked directly against E-imports |
| §8.2 / K12 | Count once per native leaf, makespan, per-class occupancy/peak, borrowed rate replacement, exact rows and upward 34-digit displays | `tests/canonical_cost.rs` decimal witnesses |
| §8.3–8.4 | First matching native entry/scheme, own+effective controls, ancillae, capacity, accuracy and topology | Native matching, exact/approximate scheme error composition, capacity, adjacency and sequential timing tested; finite class/per-bit vertex search implemented; cost optimisation remains open (ark) |
| §9.1–9.2 | Canonical object parsing, signatures, semantic envelopes/SCC hashes and object/params/schedule identities | All ten normative object interface hashes, SCC envelopes and controlled fixture object hash checked; strict layout schema/ranges/images/mappings checked at load; complete source semantic admission remains partial |
| §9.4 | Complete typed frame descriptor: slots/scratch/history/borrow/ancillae and live ranges | Supported action slot/scratch/borrow/ancilla records emitted; memoryless oracle history records emitted separately; varying context frames require lzv contract decision |
| §12.1–12.3 | Bound certificate, realised multiplicities, claims with honest evidence, local/inclusive errors and diamond composition | Binding/resources/numerical claim emission tested for supported traces; rigorous checker replay and general composition proof remain open (ark/qnl) |
| §17 items 7–9, 15, 18 | Published default ref library, cleanup defaults, accuracy defaults, placement, full history | Ref identity, exact finite shares and placement checked; memoryless oracle full-history expansion checked; layout/lproc full history remains ark |

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
builders, nested-budget concrete validation, fresh semantic-call recompute and
nonzero target history replay, qinplace coverage, general factorisation and non-translation
borrow T3 proofs, nested local-region declaration coordinates and objective
optimisation. These are postponed algorithms with specific
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

Memoryless oracle evidence now includes full-domain two-bit increment at every
budget 0–4, arbitrary target/fault values and inherited control, and branching
success/fault at budgets 0–5. The normative inc8 object schedules through the
request ABI with three width-13 history frames and an invocation-bound exhaustive
oracle claim for independent replay. Every emitted selector is resolved against
its generated operation in that regression; source alloc selectors carry concrete
oracle step/phase to avoid duplicate maximal entries. Constructor visit/apply
provenance adds addresses without changing the bound operation list or hash.
Source locations are qualified internally to preserve valid names such as pc,
next, result and code. Constant/fault literal ranges reject without truncation.

## Borrow-frame claim schema gap — bennett-sched-awo

§5.6 allows a T3 proof on D_F, including entry workspace values, but §12.2 K69
resolves bundles/regions to interface components only, and an exhaustive claim
has no frame/lifetime subject selector. A CX(c,b) frame with workspace c=0 needs
both that restricted workspace domain and its selected frame. Translation-lemma
witnesses already carry their frame and B, so that implemented path is unaffected.
The exact proposed frame selector, domain.workspace component schema and
expected:"borrow_frame" tensor-factor relation are in the bead and queued in
.tmp/cross-repo-beads.md under COMMON rule 14. No unratified field is emitted;
entry-state analysis and other independent algorithms can still proceed.

Latest focused evidence: `lproc` forms the normative worker key [0,1,0], rows
2..7 and R_worker=8, and recursive sum's single empty key with 13 ordered
locations/R_sum=16. Fixed values preserve the explicit nested token tuple and
classical values independently of the outer budget. Equal root argument values
cannot merge distinct positional keys. Admission checks concrete lproc keys
against that finite closure. Memoryless object rules and root/token heap/stack
limits reject invalid input; token components fit uint64. Host nesting limits
return NoMem, and constructor execution uses an explicit bounded worker stack.
The oracle fault-rate field replays only its declared zero-input witness, with
inherited controls fixed at zero; no full-distribution assertion is inferred.
All 13 quantum regions in the ten normative objects are audited with a universal
exact target: seven OK and six explicitly postponed outcomes (layout/lproc,
classical pseudo-location and qinplace paths). This audits producer behavior,
not independent 0.9 checker success.

## Classical pseudo-location binding gap — bennett-sched-r28

§4.7 treats bit-readable classical parameters as pseudo-locations. The published
row recipes take wire formals, while K24 forbids implicit conversion and K25's
explicit State list omits their storage/initialization. Per-read scratch, shared
argument storage and constant-control pruning would give different charged leaf
streams. The exact proposed text adds immutable class-any pseudo-location State
records in a fixed order with unsigned iN/ptr/binary64 initial bits, excluding
tokens and without adding location IDs, rows, stack cells or R_f terms. It also
specifies separate records for distinct lproc keys and the frame records and
vectors. The proposal is in the local P0 bead and upstream queue; no unratified
wire adapter is guessed. Metadata forwarding and every independent algorithm
continue. The existing catalog bytes/hash can remain unchanged under this edit.

## Layout and construction planning evidence

The load-time validator checks exact layout/global/mapping fields, pointer and
lifted address bounds without machine wrap, disjoint nonempty ranges, global
data-image identity and size, allocation-table pre-entry limits, exact oracle
mapping owners/parameter names, byte cell widths, symbol/import name collisions,
and complete block-image sectors. Zero-size globals and separate oracles mapping
the same address remain valid. All ten normative objects still load.

The source state planner derives the finite lproc closure, global location
records, reference counter widths, W_cell, maximum positional displacement
widths, sparse ordinary-memory images, pinned alloctab bits and ordered byte
partitions. Mapped cells retain input identity; hidden ordinary bytes remain
zero. B frames are lazy metadata. Normative through has W_cell=5 and Q=25 at
depth_limit=1; sum_db has W_cell=32 and Q=106 at depth_limit=0. The existing
memoryless oracle adapter now consumes this shared plan. Layout/lproc gate
emission is still an explicit ark deferral, so descriptor tests do not claim
those complete schedules.

Host limits return NoMem rather than UNSUPPORTED: extent integers use an 8 MiB
host workspace, the materialized oracle state adapter budgets 64 MiB, and image
materialization takes an explicit caller buffer budget. The symbolic plan
does not allocate B frame records. These are resource failures, not ISA width
or budget restrictions. Constructor stacks remain explicitly provisioned.

Concrete call metadata resolves each positional binding key in the caller
instance, including recursive aliases. Activation records use exact global ids
and declaration slots. Captures preserve source/parameter identities and use
offsets computed from the complete frame slot widths. Return delivery sizes
include every closure location visited by R4, including unused wider locations;
a regression checks that case. The record-level tests pass; complete call/return
gate construction is still pending.

The first new full checkpoint reached pipeline tests without an assertion
failure but exhausted the 300-second execution budget on the shared runner.
The test profile now uses opt-level=1 while retaining debug assertions and
overflow checks. All exhaustive domains remain in the gate; latest verified
command outcomes are recorded in the Resume block.

## Layout root source expansion

Roots without lproc instances now bind the full source-derived State list,
table field views and ByteLayout descriptors to the published constructors.
lift.addr, lift.load and lift.boundscheck rows use the catalog recipes, including
inherent load bounds, explicit literal-length checks and generic fault successors.
Mapped byte inputs retain their identities; ordinary memory receives global
images and reverses them before release. Tests cover all 256 mapped byte values,
first-fault/step-exhaustion/control paths, crossing an allocation boundary,
global-image restoration, and an encoded request with an admitted schedule and
bound certificate. Oracle producer domains now explicitly include all interface
region cells, so they do not accidentally assert only the zero-region slice.
A mixed-width constant assignment regression exposed an oversized displacement
binding; constants now bind exactly their low-width frame slice.

Focused layout milestone: lproc 6, oracle_state 7 and oracle_memory 6 pass.
No complete lproc, allocator/store or nested-oracle scheduling claim follows.
