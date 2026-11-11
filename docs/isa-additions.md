# Accepted ISA 0.9 additions

Contract: gate.toml selects published ISA commit
`dbc884525331fe5c5748c50662ae401ac5e7c79b`, spec SHA-256
`a86aec98d41f5c042425253b10ff632099c02fa427b220203913a861722b3d83`.
Quotes below come from that committed specification. Producer tests establish
scheduler behavior; they do not establish an independent checker verdict.
The mixed fat-object inventory remains the selected device root and its
transitive ISA closure (§12.3); native host entry/libc and general host-memory
launch contracts remain the explicit `bennett-4ht` boundary. The immutable
`ref@0.9` catalog is unchanged.

## K93 — bundle controls

§4.4: “The target denotes exactly one wire (a scalar name or indexed operand).”
“overlapping or repeated entries reject as ill-formed, never deduplicate them
to repair an operand list.”

Source binding flattens every control bundle LSB first in written list order,
validates a scalar target before flattening, and retains duplicates for normal
wire-disjointness validation. Empty controls still apply X under the effective
predicate. `mcx_bundles` checks all 64 basis tuples for two mixed list orders
and the empty list, literal k+c native costs, and repeated/overlapping controls
and multiwire targets. Before the fix the negative test failed because
`mcx (c) t` admitted a two-wire target; the positive test already passed.

## K88 — per-bit slot classes

§9.4: “Each source slot still has exactly one record, its original id and role,
and its complete signature width. Exactly one of class and fragments is
present.” “Adjacent equal-class fragments must be merged; use the class form
when there is only one fragment.”

Topology-bound interfaces prefer homogeneous slots, then search per-wire
assignments when no homogeneous schedule succeeds, preserving one
source slot and one interface_vertices array. Frames derive every bit's actual
allocation class, merge adjacent equal classes, and inherit the entire vector
through calls. `slot_fragments` checks a width-three A/B/B root and child,
canonical fragments, exact per-class occupancy/peak and homogeneous encoding;
insufficient total capacity returns INFEASIBLE with no documents. The A/B/B
positive witness failed before implementation because the allocator required
one class with capacity three. Explicit caller-selected class vectors without
a topology still require the request binding tracked by `bennett-sched-4tj`;
no host register/view ABI is invented here.

## K86 — native realisation models

§8.3: “Closed-schema target readers MUST be upgraded for realisation_model;
missing or unsupported models follow the reporting rules above and MUST NOT
be silently substituted by ideal operations.” “A supplied model that
contradicts a zero-epsilon contract rejects that target claim.”

Targets retain model bytes in target_hash. The version-1 reader validates
basis/arity, canonical reduced rational expressions, declared parameters,
control-rule and permutation syntax, matrix dimensions and exact rational
unitarity. Matching validates resolved control/operand counts and whether eps
or theta is actually bound. Rational controlled_U models for the phase-fixed
X family, swap, CZ and S/Sdg are checked against a zero-epsilon opcode.
`native_models` has four positive/negative tests; all failed red on the old
unknown-field rejection and pass after implementation. Legacy missing models
and future versions retain scheduling compatibility; their independent
verification obligations are not discharged by that acceptance.

Independent symbolic/Kraus/instrument reconstruction, general allocation-basis
permutation checking and positive-epsilon operator-norm verification remain
required under `bennett-sched-vt8.1`. No numerical checker success or
model-complete target status is claimed by the scheduler.

## K89 — selected borrow frames

§12.2: “For fixed non-B entry values, varying B over all its values must leave
B unchanged and produce the same complete non-B output, with exact +1 basis
phase.” “entry_products denotes the union of its product domains: list products
in normative visit order”.

The finite primitive path emits exhaustive expected:borrow_frame claims with
maximal source first/last points, lifetime and root-relative invocation context,
full independent interface/B domains and every live workspace bit. K76/K78
entry constants come from the specified all-operand causal stacks, including
readers and normalized X-family identities; polynomial simplification supplies
no constants. Every B fiber is replayed, complete surviving non-B outputs are
compared, and clean internal release obligations are checked. Repeated visits
use the union of entry products in visit order, including decimal iteration
indices above nine. Replaced/reversed traces rebuild selected proof bindings.
The full borrowed tuple retains every K69/K70 lifetime position and width,
including completed/future lifetimes and callees. Non-live systems are untouched
spectators; active non-B borrowed systems remain part of the fiber output check.
Sequential one-/two-bit and preceding-callee regressions both failed before
correcting domain coverage and the selected lifetime ordinal.

Seven semantic tests pass in `borrow_frames`. The initial three tests failed on
the old non-translation Unsupported; the 16-visit test additionally failed on
lexical iteration ordering before the visit-order repair. The independent
metadata-admission regression still requires successful admission
and is currently **RED**: pinned reference dbc8845 rejects domain.workspace as
an unknown field despite accepted K89. The producer retains the specified field;
this is not independent checker acceptance. The reader blocker is
`bennett-sched-vt8.4`; the exact upstream repair request is recorded in that bead. Remaining producer mechanics are
`bennett-sched-vt8.2`; awo is closed as the now-answered schema request.

## K84 — diagnostic codes

§12.3: “Checker diagnostic codes other than codes explicitly mandated elsewhere
in this specification are implementation-defined.” “Such codes are
scoped by checker name and version”.

The scheduler does not manufacture a checker verdict or depend on a checker's
local diagnostic spelling. `accepted_contracts::published_diagnostic_witnesses_preserve_verdicts_without_requiring_local_code_spelling`
replays all three pinned portable witnesses with the published reference CLI:
the complete NOT table admits, missing expected and duplicate resolved
components reject. It checks source digests, acceptance polarity and a
nonempty rejection diagnostic, with no guessed E_* spelling. This is reference
metadata/domain admission evidence, not independent schedule or claim replay.
The producer ABI/status contract and mandated K55 diagnostics remain unchanged.

## K91 — token identity and ABI

§10.1 defines `bennett_sched_features(const uint8_t** jcs_out, size_t* len_out)`
and the exact sched/0.9 manifest advertising `token_aliases/1`. The earlier
negotiation contract gap is resolved in this pin.

The copied additive setter, canonical equality partitions, source identity
propagation and schedule/certificate alias bindings are preserved.
`accepted_contracts` checks nine published portable object admission polarities
and schedules the published source under both same/distinct partitions. Source
object and parameter identities match the manifest; alias bindings and cleanup
carriers match the selected partition, and schedule hashes differ. Its target
is explicitly rebound to the local exact target; this is not original published
schedule-byte replay. `token_aliases`, `ffi` and `c_link` cover malformed/missing
partitions, all five three-token partitions, forwarding/captures, tuple
consistency and binary setter ownership.

The provider now exports the published query with immutable static JCS bytes,
length excluding a terminator, no request dependency, concurrent/repeated byte
identity and E_ARG for either NULL output without writing either output. A new
Rust regression failed to compile before the missing export was implemented.
The C alias consumer negotiates before setting aliases and verifies the manifest
survives scheduling and request destruction. Each launch provider and independent
reader still has its own obligations; sched negotiation asserts no launch role.
No manifest field is added to schedules, certificates or parameter hashes.

## K87 — complete resource contexts

§12.3: “For compatibility, unkeyed resources/gates_used/estimate MUST be the
first canonical context's projection.” “The root counts its realised event
list once, including semantic callee leaves.”

The existing producer implementation is preserved and rechecked by
`context_reports::published_resource_context_witness_preserves_exact_rationals_and_canonical_projection`:
all reached keys, child-local time, native leaf counts, complete frame/resource
context coverage, exact 1/3 and 2/3 fractions and 34-digit upward displays.
The adjacent error-bound tests exercise call-order independence and negative
accuracy cases with no output documents. `context_reports::` passed all three
existing tests at this pin before the additional root-isolation witness.
`bennett-sched-1na` remains closed; no stale context-gap Unsupported is present.
Owner-designated dirty-borrow replacement remains the separate `qm2` contract
and accounting work, rather than a reason to omit context reports.

## K90 — invocation frames

§12.3: “Each scheduled region record contains exactly one of frame and
frame_contexts.” “Each root has its own [] context; a descriptor from another
root is validated against the receiving root's bindings and placements.”

The existing complete frame-context implementation remains in place:
`pipeline::context_varying_frame_classes_emit_complete_canonical_descriptors`
passes at this pin, including exclusion of a false uniform descriptor and
exact A/B child records. The added
`context_reports::selected_roots_keep_shared_callee_frames_and_inventories_bound_to_their_own_target`
checks two roots sharing a callee, each under its own A/B topology/target.
Each certificate contains only the selected root and its callee, retains the
object binding and root key [], and emits that invocation's actual slot class.
The new fixture initially overrequired an optional singleton resource_contexts
array; that test expectation was corrected to K87's explicit omission rule.
The complete `context_reports::` filter passes four tests after correction. No host/container
launch ABI is inferred, and `bennett-sched-lzv` remains closed.

## K85 — fixed-image claims (remaining)

§12.2: “Each resolved fixed region occurs exactly once and MUST NOT also occur
in domain.regions. A mut-role region cannot be fixed by this mechanism.”
“A supplied label is a request, not a producer assertion.”

The scheduler's existing unconditional claims retain complete interface-region
domains and make no image-relative success assertion. The optional fixed-image
producer workflow, preparation/restoration bindings, exact unrestricted-D
coverage and independent exclusive outcome classification remain unimplemented
under `bennett-sched-vt8.3`. This is an accepted contract with remaining
mechanics, not a schema gap. No image-conditioned producer/checker conformance
test is claimed as passing. Mixed-launch association and general host-memory
bindings remain the separate `bennett-4ht` boundary; they do not justify
requiring native host functions to acquire ISA certificates.

## K92 — complete inverse-pair ownership

§12.3: “A qloop 0 body requires static pair inventory but has no applicability
visits”. “A disabled coherent predicate does not remove an entered instruction's
obligation.”

`pair_coverage` checks nested owner-local entries in forward/adjoint calls,
independently forwarded token budgets and rejection of an insufficient inner
budget despite a larger root budget. A zero-trip source body now retains a
verified static owner entry without adding oracle visits, gates or scratch
allocations. Inventory-only proofs are distinct from executed-call workspace;
all adapters and cleanup paths preserve that distinction.

The zero-trip test failed before implementation on an empty inverse_pairs array.
A separate red assertion caught small domain bounds encoded as strings. One
§9.1 integer encoder now serves schedules and pair/oracle/borrow domains;
bounds through 2^53-1 are JSON numbers, larger bounds remain decimal strings.
Existing expectations for a 32-bit pointer domain and the width-two full range
were corrected to this canonical rule. All three pair-coverage tests and all ten
`qinplace::` regressions pass. Static proof search may establish a standalone theorem at a larger primary
budget, without changing the root invocation or inventing applicability visits.
Its primary token equality class remains tuple-consistent. Zero-trip fixtures
cover both zero and one tokens and a primary/argument alias. Structural/library
proof scaling remains `a0v`; host exhaustion yields no reduced-domain claim.
No sampled or truncated-domain entry is substituted for a covering proof.
