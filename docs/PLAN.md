# ISA 0.9 scheduler implementation and evidence

Contract: `bennett-isa-spec/spec/Bennett-ISA-Spec-0.9.md`, §11 and its
dependencies. The published ISA pin is `dbc884525331fe5c5748c50662ae401ac5e7c79b`, including accepted K84–K94, closed native-model/fixed-image encodings and the K91 per-role feature negotiation ABI. gate.toml and SPEC_VERSIONS.toml record that commit and its verified committed spec digest. The catalog bytes/hash remain unchanged. New contract adoption is tracked by vt8; no complete conformance is claimed.
The published `ref` JSON is specification data, vendored in `data/`; no sibling
implementation code is linked or copied. Beads issue `bennett-sched-akl` owns
implementation tracking. This document maps requirements and records evidence;
it does not replace Beads.

## Milestones

1. Implemented and focused-tested: binary ABI, exact decimals and canonical document identity.
2. Partial: schema/selectors/binding and closed constructors implemented; K82 binding and all published binding witnesses implemented; full functional family replay remains qnl.
3. Partial: memoryless/layout oracle construction, lproc call/return rows, source cleanup carriers/recompute and supported action traces, native/exact/approximate schemes and bound resource certificates implemented; ark/2u4 track remaining algorithms.
4. Audit performed: all ten object identities and the full controlled-action basis domain checked; every published case/manifest inventoried, remaining conformance tracked in qnl. The installed pinned-tool stub and gate.toml have a focused contract test; the orchestrator must verify the committed SHA and matching ISA pin.

## Requirement inventory

Clause identifiers below use the published `ISA-…` dependencies where provided;
K identifiers disambiguate subsequently ratified paragraphs in §11/§11.1.

| Clause | Required behavior | Evidence / state |
|---|---|---|
| §11 C declarations | Opaque owned requests/responses, all setters, copied buffers, status and typed OK/INFEASIBLE/UNSUPPORTED, reasons, accessors and frees | Exported; Rust ownership/null tests and C11 cdylib consumer (`ffi`, `c_link`) |
| ISA-11-P2 | Expand every row/instruction in specified source order; stable generated ordinals; removals only from library permission | Primitive action source expansion checked; memoryless oracle state/row/commit/history construction implemented; layout address/load/bounds-check root rows implemented; lproc call/return rows implemented; store/allocator/free rows implemented; lifted semantic calls implemented with independent token contexts; general proofs remain ark |
| ISA-11-P3 | JCS schedule; exact bindings; cleanup/gate/alloc/start entries, selectors and defaults | Strict schema and output binding tests; generated action schedules (`selectors`, `pipeline`) |
| ISA-11-P4, §8.3 | Interface/control placements, per-class vertices, lifetimes, adjacency, no routing | Default class/vertex placement, capacity and adjacency implemented; finite class/per-bit vertex search implemented; cost optimisation remains open (ark) |
| ISA-11-P5 / K52 | Invocation-chain prefixes; source-loop iteration, nested budget/step/phase; no `lift.call` edge | Nested coordinate resolver tested; action call source paths tested; memoryless oracle step/phase/support coordinates and reverse ordinals checked; action oracle-call contexts implemented; lifted calls retain independent source/step spaces (y0n) |
| ISA-11-P6 | Lexicographic specificity, maximal ties independent of document order and entry kind; defaults replace rather than inherit fields | Per-operation maximal ties and independent kinds tested, including K63 specificity neutrality |
| ISA-11-P7 / K55 | Admission/concrete validation stages, literal-only nested-budget derivation; exact rational decimal equivalence | Exact decimals implemented; linked source-path/request admission implemented; lproc concrete instance admission implemented; known generated domains checked; unresolved nested-budget domains remain ark |
| K63 | Fixed-instance and semantic-call-occurrence identity and selectors, without extra specificity | Selectors and typed semantic-call occurrence spaces implemented; Finite positional lproc closure, row ranges, location IDs and activations formed; concrete selector keys and published call/return row emission tested |
| K64 | `$construct/$step/$commit/$release`, separate support ordinal spaces, B=0 construction/commit/release | All four support points emitted for memoryless oracles, including B=0; forward/reverse step coordinates and shared Rows/Next ordinals checked (`oracle`) |
| K83 | Action-local recursive declaration collection, unconditional entry/exit support, contiguous ordinals and adjoints | `regions` checks zero-trip loops, both qif arms, images, multiple declarations, source adjoints, calls, exact/empty selector domains and certificates; full 129-test gate passed |
| K65 | Non-emitting ordinal-0 whole-cleanup carriers, reverse/recompute selection before leaf expansion | Carriers formed from source version matches; all selections stage before mutation; reverse, primitive/intrinsic and lifted-uncall delta recompute checked (`cleanup`, `lifted_cleanup`); carriers retain each containing step/phase and lproc instance |
| K81 | Source occurrence/version cleanup partition, indivisible joint inverses, intrinsic boundaries, first-release ownership | Semantic calls remain atomic; intrinsic boundaries and first-release ownership of joint inverses checked; broader source forms remain ark |
| K80 | Reverse retains written points; recompute replacement owns carrier-point contiguous ordinals and fresh semantic-call occurrences | Primitive/intrinsic recompute owns contiguous carrier ordinals and omits original loop coordinates; Fresh semantic-call contexts and allocated input-history replay implemented (7va); general mutable-interface/borrowed history remains required |
| ISA-11.1-P1–P3 / K11 | Complete 219-recipe `ref@0.9`, exact envelope/hash, no witness hash substituted | `tests/canonical_cost.rs` catalog identity test |
| ISA-11.1-P4–P5 | Closed constructors, scalar expressions, loops, allocation identities, inverse traversal and retained ordinals | Constructor engine and inverse/identity tests; K82 named helpers implemented; 16 binding cases, 54 interfaces, 86 edge digests checked |
| K19 | Typed formals, capture-free bodies, positional wire binding, controls, linked symbols, gate/width/cycle validation | Typed binding, strict invoke scope, captured bodies, wires/angles and dependency cycles implemented; linked parsed-source registry/interface and per-invocation body construction implemented (6qc); lifted row binding/projection implemented (y0n); K82 named interfaces import only declared by-name wire names |
| K82 | Named-interface recipes import exactly lexical wire bindings, typed recipes/templates stay closed, no non-wire fallback | All 16 published cases, 54 interfaces and 86 edge digests match (`named_interface`); catalog hash unchanged |
| K21 | Data-image byte order and unconditional X leaves | 8102 unconditional image witness and inverse machinery tested (`expansion`) |
| K22 | Indexed captured typed templates, apply-site controls, source row ordinals | Constructor support present; complete published template families remain unexercised (qnl) |
| K24 | Closure-bound argument vectors, exact iN/f64/token encodings, no implicit conversions | Value encodings/hash witness checked; finite lproc keys and resolved token/classical/region forwarding checked independently of runtime values; nested constructor argument forwarding/captures checked (6qc); lifted row binding/projection implemented (y0n); pseudo-location storage requires r28 |
| K77 | Explicit per-call oracle token context, captured across adapters, distinct from positional arguments | Action oracle_call and ref linked source builders capture explicit tuples. Five adapter forms and semantic forwarding preserve K77; published two-token inc8 witness checked; lifted rows use the same captured channel; K91 identities are implemented; published sched feature negotiation is implemented |
| K25 | Ordered source-validated byte/activation/capture/state/frame descriptors | Generic typed records implemented; source-derived State/frame sizing and ordered byte/location maps implemented, including layout counters, sparse images and pinned table initialization; memoryless adapter consumes this plan; activation/capture metadata, concrete call targets and lproc call/return row emission implemented |
| K26 | Region shape, role, origin and ordered cells; reject same-width shape substitutions | Generic and source shape/role/origin forwarding implemented; all K79 address cases and 128 complete forward/inverse tuples checked |
| K27 | Class and allocation declarations, unconditional acquire/relinquish, shared lifetime state, fresh reacquisition | Acquire/relinquish, fresh reacquisition and inactive scope exit tests; translation-lemma and finite selected K89 frame proofs implemented; remaining entry/lifetime mechanics are vt8.2 |
| K28 / K62 | qinplace t/f release sequence and source action adjoint before expansion | Qinplace emits target/fault allocation, G/exchange/H and target/fault release; source adjoints preserve forward child construction and swap call occurrences. Complete finite-domain inverse-pair proof and K4 certificate entries implemented (0dr); scalable proofs remain a0v |
| ISA-11.1-P29 | Forward leaf ordinals before transforms/schemes, callee spaces, inverse retains identity, all ref removable sets empty | Bound qxor ordinals, inverse retention and empty removable sets tested; full source spaces deferred (ark) |
| §11 obligations / §6 | Cleanup windows, exact source versions/control sets, no measurement cleanup | Source semantic-call/intrinsic version matching, matched dependency windows and sufficient composed-use factorisation implemented; general factorisation and mutable-interface/borrowed history recompute remain open (ark/7va) |
| §8.1 / K9 | Closed cost AST, exact rationals, epsilon decade/log2, context variable rules | Exact AST/context/decade/log2 evaluation tested against published E vectors; imported model/version/resource quantity mapping checked directly against E-imports |
| §8.2 / K12 | Count once per native leaf, makespan, per-class occupancy/peak, borrowed rate replacement, exact rows and upward 34-digit displays | `tests/canonical_cost.rs` decimal witnesses; designated owner alias/accounting needs qm2 |
| §8.3–8.4 | First matching native entry/scheme, own+effective controls, ancillae, capacity, accuracy and topology | Native matching, exact/approximate scheme error composition, capacity, adjacency and sequential timing tested; finite class/per-bit vertex search implemented; cost optimisation remains open (ark) |
| §9.1–9.2 | Canonical object parsing, signatures, semantic envelopes/SCC hashes and object/params/schedule identities | All ten normative object interface hashes, SCC envelopes and controlled fixture object hash checked; strict layout schema/ranges/images/mappings checked at load; complete source semantic admission remains partial |
| §9.4 | Complete typed frame descriptor: slots/scratch/history/borrow/ancillae and live ranges | Supported action slot/scratch/borrow/ancilla records emitted; memoryless oracle history records emitted separately; K90 complete sorted frame_contexts emitted with per-invocation owned workspace; mixed A/B, differing budgets and zero-leaf contexts tested (lzv closed) |
| §12.1–12.3 | Bound certificate, realised multiplicities, claims with honest evidence, local/inclusive errors and diamond composition | Binding/resources/numerical claim emission tested for supported traces; K87 complete resource_contexts and first projections emitted, with child-local exact timing and post-scheme native gate counts (1na closed); rigorous checker replay and general composition proof remain open (ark/qnl) |
| §17 items 7–9, 15, 18 | Published default ref library, cleanup defaults, accuracy defaults, placement, full history | Ref identity, exact finite shares and placement checked; memoryless oracle full-history expansion checked; layout/lproc call/return history checked on small full domains; materialized large sum_db requires2u4 |

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

## Accepted context repairs — lzv / 1na / awo

The live ISA now settles the earlier schema requests: K90 defines exclusive
uniform frame or complete canonical frame_contexts, K87 defines complete
resource_contexts and first-context projections, and K89 defines selected
borrow_frame/workspace entry proofs. K87/K90 producer algorithms now replace the earlier diagnostics (1na/lzv closed). K89 finite selected-frame production is implemented; general entry/lifetime mechanics remain vt8.2 and pinned-reference admission is blocked on its workspace reader. The earlier proposals
remain in the local cross-repo queue as historical context.

## Postponed mechanics and conformance limits

`bennett-sched-ark` tracks remaining budget/source admission,
general mutable-interface/borrowed target history replay, general factorisation and non-translation
borrow T3 proofs and objective
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

`bennett-sched-qnl` tracks the remaining conformance. The inventory has 559 A–H
cases: 18 direct fixture mappings, 17 equivalent witness mappings, 20 partial ref
families, 8 explicitly rebound K83 schedule fixtures, 5 other scheduler vectors,
and 491 executor/checker vectors not replayed
by this scheduler. Eight portable manifests are recorded (K84 metadata admission and K91 metadata/source scheduling are exercised): K79's 12 source address
cases (4 accepted, 8 rejected) and all 128 forward/inverse tuples, K82's 16 binding
cases/54 interfaces/86 digests pass; K80 source trace fields and all 80 published basis tuples pass after version rebinding; K78/K81 direct manifest replay remains open. Original 0.8 source
bodies in K79 are re-enveloped and rehashed for 0.9 without changing instructions.
Equivalent witnesses do not claim direct original object/request replay.
Every source hash is recorded in `docs/conformance-report.json`; the inventory
regression checks missing/stale entries. Functional correctness of the full ref
catalog does not follow from its binding digests.

All ten §16.3 object interface hashes are tested, including cyclic `sum` and the
layout/path envelopes. The published controlled-action object hash, frame-bound
resource rows, exact-scheme expansion and all four basis configurations are
checked. Differential execution uses the published debug CLI and four-input
compatibility fixture. That debug binary now admits the actual 0.9 object and
our emitted schedule/certificate metadata; an absent source point rejects.
Only the pinned debug binary is exercised here. CLI metadata admission is distinct from
scheduled execution and rigorous certificate-claim replay, both still unclaimed.
Numerical certificate claims emitted here remain producer claims for independent
checking; no proof/evidence label or rigorous interval-checker success is claimed.

The push gate is the published pinned-tool stub. It extracts the tool from
committed meta objects at gate.toml's tool_sha and delegates pushed-SHA isolation,
sibling materialisation and reference builds to that tool. tests/gate.rs checks
the installed stub/config contract; shared gate self-tests own protocol coverage.
scripts/check-working-tree preserves the optional local, git-free runner
checkpoint. Under the 2026-11-03 R3 note the agent runs focused tests only; the
orchestrator runs full gates and hook-check. Earlier working-tree gate evidence
below predates this adoption and does not prove pinned push verification.

The root gate.toml and SPEC_VERSIONS.toml agree on the ISA commit and spec
digest. The conformance inventory is regenerated from that gate pin, including
the K91 cases and manifest. The orchestrator still verifies the pushed SHA.

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

## Accepted borrow-frame claim contract — bennett-sched-awo

K89 now defines selected borrow_frame subjects, domain.workspace and
entry_products over normative K76/K78 entry domains. Finite primitive frames
emit complete schedule-bound exhaustive claims, including clean workspace and
causally restored constants, independent B fibers and visit-ordered union
products. Remaining opaque-call, nested-frame and lifetime correspondence
mechanics are vt8.2, not missing schema. The pinned reference still rejects
workspace at metadata admission; the unchanged conformance regression is red.
See [the per-clause evidence](isa-additions.md). No field is dropped to fit the
old consumer, and no independent frame-checker success is claimed.

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
exact target: seven OK, five explicit UNSUPPORTED outcomes (nested calls, classical pseudo-
location paths, including inverse-pair oracles), and one BENNETT_E_NOMEM outcome (sum_db B=3).
Sum_db also has a successful B=0 construction/commit/release witness. This audits producer behavior,
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
memoryless oracle adapter now consumes this shared plan. Address/load/boundscheck and lproc call/return rows now have source expansion
tests. Nested semantic-call adapters remain explicit ark
deferrals, so descriptor tests alone do not claim their complete schedules.

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
a regression checks that case. Call/return gate construction now invokes the published activation recipes;
full small input/target/fault/control domains at B=0..5 pass using a64-lane
basis interpreter checked against the ordinary map interpreter.

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
Lproc call/return source expansion now has full small-domain tests. Store/allocator/free source expansion is now implemented under px2;
nested semantic-call schedules remain required.

## Design-review fixes and remaining contract decisions

Review 3k1 exposed an adjoint call rewriting its caller's dynamic loop
coordinates. Remapping is now limited to the adjointed invocation and its
descendants, and descendant invocation metadata follows the remapped events.
Caller/callee loops, nested adjoints and while/double-adjoint regressions pass.

Review mi3 exposed a blanket refusal of repeated callees with workspace.
Identical interface contexts now share a closure record containing every
realised dynamic allocation in first-allocation order. Dynamic ids remain
distinct; the one/two/three-call regression checks complete frames and unchanged
physical peak/occupancy. Different interface slot classes still need lzv.

Review io7 exposed an unbound root interface class choice. Without topology
the scheduler now preserves the document's first-capacity class defaults.
Explicit scratch alloc.class choices and topology interface_vertices still
carry class choices. 4tj queues the exact optional interface_classes contract
extension for non-topology choices; no unratified field is emitted.

Review 2qd exposed enumeration of irrelevant vertex permutations and repeated
source expansion. The trace is now prepared once. Adjacency conflicts backtrack
the decision prefix affecting the failed operands and realization; class
mismatches skip unrelated vertex decisions. This retains finite exhaustive
choices without an attempt cutoff or inserted routing. Tests include a feasible
later class and infeasible two/eight-wire graphs.

Review qm2 remains open. A current C ABI probe reproduces capacity3 INFEASIBLE
and capacity8 peak4/occupancy14. The current model creates a separate dirty
requirement, with no represented owner alias. Reusing the caller's idle wire
needs a reproducible owner designation. The exact borrow_from/default and
charging proposal is queued upstream; no unrelated owner's charge is silently
subtracted. Dirty borrowing remains required scope.

The gate now reads free -g and uptime and selects two build/test threads when
MemAvailable is below6GiB, otherwise at most four. The last complete full checkpoint passed 129 tests after admission, resource-
context guards, K83 support and the inventory/pin refresh. The exact command was
`timeout 660 scripts/pre-push --working-tree` (exit 0).


Review760 admission now checks declared allocation classes and gate schemes,
and maximal matching ties on each source-generated gate/allocation/start/cleanup
operation whose domain is known at admission. It accepts shielded duplicate
patterns and empty generated domains; K55 deferred nested-oracle domains remain
at their specified concrete check. Producer validation reuses the prepared trace.

Review1na confirms order-dependent non-root resource reports. Identical repeated
contexts remain supported. Differing exact rows or inclusive gate counts now
return the explicit1na contract diagnostic rather than choosing the last call.
The proposed per-invocation realisation_contexts schema and uniform legacy-map
rule are queued upstream. This guard does not complete mixed-context reporting;
resources and gates_used need a contract decision distinct from error bounds.


Accepted K83 adds action-local $construct/$release support domains, recursive
once-per-invocation qregion collection even in zero-trip loops and both qif
arms, unconditional preparation/restoration and contiguous support ordinals.
Implemented under bennett-sched-ecz: recursive collection in canonical text
order, distinct forward entry/exit domains, source-adjoint support renaming and
reserved action path admission. Source version and borrow-scope adapters now
handle support leaves without treating them as explicit cleanup instructions.
Focused region/structure/admission/borrow tests pass (17 tests), followed by
full gate success (129 tests). Ecz is closed.
Observed ISA main is ecb8551250bca5e0702c6c7894b603388b2e51bf with spec hash
7458b362d22d2b89912331da8866e334385686faf8e2b51d8d49684e522c3034.


Lifted memory milestone px2 uses published lift.store, lift.alloc and lift.free
recipes. Four allocator displacement fields use the global source slot widths,
with heap and table flags evaluated before exchanges. Length locations retain
their declared width; source literals must fit ptr_width, use at least one bit,
and are explicitly initialized and cleared around the row. Free binds the
catalog's unused length formal without allocating extra wires. Load/store widths
retain the specified 8..128-bit whole-byte limit.

Focused memory/allocator verification passed 16 tests: all small allocator
input/target/fault/control tuples over B/heap 0..3, zero-size allocation,
double-free and non-reused table entries, same-base pinned zero-size globals,
wide location lengths, invalid literal bounds, an encoded schedule/certificate,
every input/old mapped-byte pair and both controls for store/load, and full
arbitrary target/fault/control domains for store cuts and bounds faults. These
are explicit finite test domains, not an independent checker verdict. The additional load-width regression and the full gate passed: 140 tests via
`timeout 660 scripts/pre-push --working-tree` (exit 0). Px2 is closed.


Action oracle calls (u8a) now bind each explicit token independently of the
root budget and signature args. Call expansion remaps all interfaces, controls,
private identities and source locations while retaining the bound oracle's own
step/phase and ordinal spaces. Source action adjoints reverse call order but
keep each oracle involution's complete forward construction. The selected region
call graph includes zero-trip structure and lproc semantic edges; cycles and
misplaced/unknown opcodes reject before unrolling.

Focused calls/regions/structure pass 20 tests. These include full small interface
and control domains, forward/adjoint/double-adjoint calls and caller loops,
malformed bindings, zero-trip graph cycles, concrete selector resolution and a
single-call encoded request with separate nested history records. K77's actual
published inc8 two-token witness passes all 256 inputs and both controls at zero
target/fault; it is recorded separately from equivalent witnesses. Different
budget contexts still need the1na resource schema to emit a complete combined
certificate. Lifted oracle/permutation rows remain required. The full combined gate passed 150 tests, including repeated same-tuple certificate checks and K83 fixtures.


Published K83 conformance replay (nvp) loads all six source objects verbatim,
checks all trace fields and allocation widths, and executes every external
basis value plus raw inverse. Eight schedule fixtures omit required expansion
bindings and have zero placeholder target hashes. Their strict parser errors
are preserved; explicitly supplying the manifest-bound library and target
cb549be29a6b7c237960c0d0b058bcf1be12d4a373f6769326680a14104a3123 yields the published two accepts/six rejects. The concrete target is
in data/k83-target.json. This is labelled rebound admission, not verbatim
acceptance of the incomplete positive documents. Exact fixture repair text is
queued upstream under COMMON14; qnl retains that original-fixture gap.

The earlier K83 checkpoint recorded 533 cases and six manifests:18 direct fixture tests,
17 equivalent witnesses,8 rebound schedule fixtures,20 partial reference
families,5 remaining scheduler and465 remaining executor/checker cases.
require-complete still exits2, including for rebound-only schedule evidence.

## Qinplace construction and inverse-pair claims — 0dr / a0v

Qinplace binds both oracle signatures, explicit budget and typed arguments,
preserves all readonly inputs/region cells and controls, and emits the required
private target/fault lifetime order. Both complete basis-domain compositions
are checked for inverse behavior and fault freedom at the selected argument
values and budget. Claims include every input bundle and every interface-region
cell; iN/ptr/f64 arguments use singleton unsigned bit domains and token arguments
retain exact tuples. No invocation field is added to the K4 inverse-pair claim.

Current proof tables have a 16 MiB workspace bound and replay has 100,000,000
native-leaf credits. Exceeding either returns NoMem with its full-domain reason;
no domain is sampled or narrowed. A valid 21-bit pair exercises this boundary.
Scalable source/library proof methods remain required in bennett-sched-a0v.
Instruction-private buffers stay outside source value provenance; cleanup sees
the atomic permutation and its inverse, including the input-version transfer.

Focused qinplace/oracle_calls/regions/structure tests pass 30 tests, covering
full small input/control/region domains, source adjoints, raw inverses, malformed
bindings, typed arguments, certificate entries and workspace limits. The 0ed
regression also verifies that each actual adjoint allocation resolves its
emitted allocation selector: source adjoints now update operation kinds along
with leaf operations. Full gate: timeout 660 scripts/pre-push --working-tree, exit 0, 160 tests plus C11 consumers, reference differential, inventory and formatting. 0dr/0ed are closed; a0v remains open.

## Fresh recompute calls and input histories — 7va

Cleanup replay rebuilds source callees in the selected orientation, rebinds
interfaces and proofs, assigns fresh allocation identities and independent call
occurrences, and retains each callee's own source points and ordinal spaces.
Nested cleanup selectors apply after fresh path rebinding. Caller leaves at the
carrier exclude callee subtrees; obsolete inverse invocation records are removed.
Empty callee spans follow actual insertion boundaries.

Allocated pre-call state is reconstructed from its complete causal source
history, rooted in unchanged readonly interfaces and clean allocations. A
separate baseline computes f(h) XOR h, so independent call cleanup restores
the exact pre-call h without taking ownership of prior computing instructions.
Non-permutation history cannot select this permutation-only realisation.
General arbitrary mutable-interface and borrowed-entry histories still need
the remaining 7va algorithm and receive that precise diagnostic. Large replay
uses an explicit 2,000,000-leaf whole-source workspace bound (generic recipe
expansion still uses 1,000,000); scalable materialisation remains
2u4 and no domain or row is discarded.

Tests cover the K80 two-call occurrence/ordinal witness, K81 nonzero qcopy
history, fresh nested cleanup selectors, while/local-region source adjoints,
empty callee spans, oracle tokens/forward step-phase coordinates and concrete
schedule admission of fresh call occurrences. All five published K80 source
traces and 80 full basis tuples, including raw inverse, pass after re-enveloping
their unchanged 0.8 instruction bodies for 0.9. This is version-rebound evidence,
not verbatim 0.9 object admission. The combined full gate passed: timeout 660 scripts/pre-push --working-tree, exit 0, 169 tests plus C11/reference differential/inventory/fmt. 7va remains in progress for general history.

## Linked source builders and captured invocation context — 6qc

Library registration binds a parsed source closure, its complete typed header
interface and a per-call body builder. The explicit OracleInvocation channel
holds exactly the validated token tuple; Arguments captures it independently of
the positional values. Forwarding through invoke, semantic calls, captured
bodies and templates preserves the captured channel. The ref catalog JSON,
envelope hash and constructor grammar are unchanged; a new JSON context key
still rejects.

Source builders validate values, region shapes/roles, controls, aliases and the
callee's budget, then construct and bind that invocation's native trace. Source
provenance retains the shared trace and event index, including source-generated
ordinals, frame/proof metadata and invocation identities. Native source leaves
charge the constructor workspace budget and raw inverse retains their captured
context and forward identity.

The actual K77 inc8 witness passes all 256 inputs and both controls, plus full
raw inverse restoration, through direct calls, manual captures and typed
body/template adapters. Further tests cover nested semantic forwarding, exact
i128/f64 NaN/token/ptr values, mutable region cells, role/type/width mismatch,
missing context, invalid/source-inconsistent tuples and explicit NoMem.
Lifted oracle/uncall/permutation row argument binding and outer row/child
coordinate projection remain the next ark integration step; this registry alone
does not claim those rows. Full gate: timeout 660 scripts/pre-push --working-tree, exit 0, 175 tests plus C11/reference differential/inventory/fmt. 6qc is closed.

## Lifted source calls and non-emitting contexts — y0n

Lifted oracle/uncall/permutation rows now bind complete typed interfaces to the
linked source builder. Exact tokens remain separate from positional arguments.
Root and fixed lproc environments retain token/region forwarding, with source
location and alias checks; tokens still cannot be read as wires.

Projection preserves each callee's invocation, own source points and native
ordinals, frames and proof metadata, independently of outer oracle steps.
Raw inverse retains forward identities and mirrors source coordinates.
Callee leaves do not consume caller ordinals. Identical decisions at a shared
nested selector address merge; incompatible decisions constrain placement search.

A non-emitting source-call channel carries spans and constructor provenance,
so zero-leaf callees retain invocation records and certificate multiplicity.
Inverse mirrors spans and reverses call order while keeping forward call numbers.
Metadata has an explicit 65,536-record host workspace bound; larger streaming
remains 2u4. A host failure never drops a call or narrows a quantum domain.

Focused lifted/registry tests pass 12 tests, including complete small domains,
nested token budgets, retained empty contexts and their certificate multiplicity.
Two additional fixed-lproc tests cover distinct token positions and coherent
branches over every external input, target, fault and control value. The
sequential two-call case correctly faults when the monotonic stack pointer
exceeds its eight-cell budget; the branch case fits one six-cell activation.
These tests passed separately with one test thread (69 and 87 seconds, peak RSS
3.8 and 2.8 GiB respectively). Full current-tree verification is reserved to the
orchestrator under R3; no new pinned/full-gate result is claimed.

§5.5 source admission now follows reachable CFG paths in root and fixed-lproc
activations, retaining per-key LIFO calls, exact token identities and argument
values, and location versions. It rejects missing/ambiguous/non-LIFO matches
and intervening input/output writes, accepts stores and balanced loops, and
restores versions on matched uncalls. Nine focused tests pass (r0u closed).
The finite worklist has explicit 32 MiB/8,192-state host bounds; scalable proofs
for unbounded pending stacks remain required (kbc), with a NoMem regression.
K65 lifted-uncall carriers and fresh oracle-delta recompute are implemented
(uwu): source matches survive into non-emitting ordinal-0 carriers, each containing
step/phase/instance is selected independently, and Rows/Next retain a shared caller
ordinal space. Recompute uses fresh oracle calls to compute the XOR delta on clean
target/fault buffers, XORs it into the current targets and cleans the fresh buffers;
this works for arbitrary prior targets because §5.5 calls are XOR involutions.
Complete small domains, nonzero pre-call local history, one/all-step selection,
fixed lproc instances, action callers, fresh call admission and maximal ties pass.
Deep projection now preserves intermediate oracle coordinates in invocation
metadata as well as native events. Raw-inverse borrowed frames now preserve their forward translation proof and add the adjoint_table subclaim required by normative Example 3. Complete small dirty/input/target/fault/control domains and emitted forward/inverse proof references pass. General non-translation borrow proofs remain awo/vt8.

Exactness propagates through every oracle and permutation-action descendant.
Native and scheme leaves use zero epsilon share in those subtrees, so a later
exact target entry can be selected after an approximate one. Approximate-only
exact subtrees are INFEASIBLE; ordinary actions retain positive accuracy shares.
Two request-level regressions plus 26 target/pipeline/source-pair tests pass
(krn closed). The focused 13-region normative audit passes in 7.90 seconds,
retaining 7 OK / 5 tracked UNSUPPORTED / 1 explicit NoMem (4j8 closed); this
does not discharge the scalable sum_db algorithm in 2u4.

The source registry and C ABI bind a selected device closure within its device
object. That object hash identifies the device artifact, not an enclosing native
binary. A mixed fat binary may carry several such objects and request schedules
per selected closure. Host entry points, host code inventory and host handles
remain outside these records; no launch-ABI field or whole-binary assumption is
introduced (standing constraint bennett-4ht).


## Accepted K84–K93 additions — vt8

Exact pinned quotations, red-first findings and evidence limits are in
[isa-additions.md](isa-additions.md). The umbrella remains open for the named
mechanics and contract/reader blockers; this is not complete conformance.

| Clause | Scheduler obligation | Current evidence / remaining implementation |
|---|---|---|
| K84 | Treat nonmandated checker diagnostic names as implementation-defined | All three portable diagnostic witnesses pass pinned reference metadata admission without asserting implementation-defined code spelling (accepted_contracts) |
| K85 | Fixed-image claim bindings and conditional proof/evidence distinctions over the selected device closure | Unconditional claims remain; fixed-image production/preparation/coverage/outcome mechanics are vt8.3 |
| K86 | Exact evaluated native-model binding, phase/control reconstruction and target identity | Model bytes, explicit syntax, rational unitarity, resolved arity and zero-epsilon phase checks implemented (native_models); general reconstruction/accuracy remains vt8.1 |
| K87 | Canonical complete resource_contexts, first projection, native leaf counts and child-local exact accounting | Implemented complete canonical resource contexts, exact child-local views and first projections; 1/3 witness and both controlled-call orders pass (1na closed) |
| K88 | Per-wire class vectors and canonical frame slot fragments without splitting source slots | Topology-bound per-wire allocation and canonical merged slot fragments inherited by callees pass slot_fragments; explicit no-topology class bindings remain 4tj |
| K89 | Selected maximal borrow_frame relations and workspace entry components/products | Finite selected-frame claims, K76/K78 entry constants, full B fibers and visit-ordered entry_products pass seven semantic tests; pinned reader regression is red; remaining mechanics vt8.2 |
| K90 | Exclusive all-context-valid frame or canonical complete frame_contexts | Implemented complete canonical frame contexts with per-invocation workspace and slots; A/B and B2/B3 histories pass (lzv closed) |
| K91 | token_aliases equality partitions, captured source identities and negotiated /1 estimation ABI | Setter, canonical partitions, global identities and all producer bindings implemented; nine published fixture admission polarities and both source partitions pass. Negotiation remains xzv / upstream b-isa-spec-b3ui |
| K92 | Complete owner-local qinplace pair coverage over all reached contexts | Owner-local transitive entries, forwarded budgets, adjoints and zero-trip static inventory implemented; static theorems preserve root bindings and alias consistency. Pair coverage 3 and qinplace 10 pass; scaling/library proof remains a0v |
| K93 | Flatten ordered bundle MCX controls, check distinct wires and scalar target | Written bundle order, full control flattening, scalar-target preflight, distinctness and literal k+c cost are tested across all 64 basis tuples (mcx_bundles) |


## Context records and common errors — 1na / lzv / tgt

Resource and frame records cover the same complete reached invocation set,
including defaults, disabled predicates and empty callees. Keys preserve every
outer and inner oracle coordinate, fixed instance and forward call occurrence.
Repeated regions receive sorted context arrays; root/singleton regions retain a
single frame. Every frame contains only that invocation's owned workspace.
Compatibility resources/gates/estimate project the first canonical context.
Root accounting counts each native event once; child views use invocation-local
time and restricted interface/control footprints. Gate counts follow target
scheme expansion. The K87 1/3 witness checks exact fractions and 34-digit upward
resource displays without summing rounded child values.

Common local error bounds cover every context of each variant/control key;
inclusive bounds compose through the realised call tree. Numerical subjects and
multiplicity totals use those common bounds. Exact target-derived rationals drive
accuracy decisions; finite error enclosures are composed from the declared local
bounds, with additional precision when a strict requested margin requires it.
Terminating error decimals retain all digits. The enclosure workspace is bounded
at 32 MiB with an explicit NoMem result, not an ISA domain restriction.
Focused canonical_cost/context_reports/exact_regions/pipeline tests pass 23 tests,
including both class/call orders and a near-1/3 request needing over 34 digits.
No independent rigorous certificate-claim replay is inferred from these tests.

## Raw inverse cleanup — mt9

A raw inverse selects the forward construction and then reverses its emitted
trace, retaining forward source points, ordinals and scratch/call identities.
The cleanup realiser now extracts the captured raw subtree, normalises it to
forward source order, resolves choices through a coordinate view, and reinverts
the selected expansion. Views preserve the actual schedule meaning of inner
oracle phases while temporary source traversal changes direction. Source
adjoints remain distinct from raw inverse metadata.

Rebuilt invocation, cleanup, borrow and qinplace records replace the retired
subtree. Event-boundary maps preserve empty call anchors, and native spans are
refreshed through the invocation tree. Complete 128-state interface domains pass
for forward-only, inverse-only and combined choices, nested oracle phase views,
and empty semantic-call cleanup/replay contexts. The original failure was a
spurious nonzero-history refusal in the inverse phase. Focused timeout 240 cargo test --offline -j2 --test raw_inverse_cleanup
--test lifted_cleanup --test k80_vectors --test review_regressions --test differential
-- --test-threads=2 passes 22 tests. mt9 and y0n are closed for their integration
scope; new K91 token identity binding remains xzv, and general input histories
remain 7va.


## K91 identity bindings — xzv

The setter and source/certificate binding implementation is described in
[ABI.md](ABI.md). All five three-token partitions and both equal-tuple action
partitions pass; the latter retain the complete 64-state input/target/fault
domain while producing different K65 cleanup carriers. Captures preserve global
ids despite identical callee-local partitions. Fixed-lproc forwarding, request
replay rejection, atomic pointer/buffer handling and dynamic C11 linkage pass.
Token-free legacy requests now emit the inferred partition and the schedule
schema requires it. K83 fixture tests separately record missing token_aliases
and expansion bindings, then explicitly rebind their portable templates.
Inverse-pair entries preserve their primary/argument partition and exact nested
token tuples; increasing only the primary budget cannot retain a shared alias.
The draft's advisor R21 note leaves feature-query negotiation pending
b-isa-spec-b3ui and forbids invented protocols, so xzv is not closed.

The 2026-11-03 batch review adds fdu (P0 source-adjoint ordinal formation), jez
(P1 class search scaling) and b08 (P1 overstated E-vector evidence). These are
required fixes; review and full gates remain orchestrator-owned.
