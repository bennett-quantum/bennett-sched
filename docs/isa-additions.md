# Accepted ISA 0.9 additions

Contract: gate.toml selects published ISA commit
`62a0cf0ef9f1f1c1db12c84ed24846b6b9695cdc`, spec SHA-256
`8b377477f135b82da3cc0a4caa96be9db8c5cee4fd53ed87116ba2e1e259b1f5`.
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

Topology-bound interfaces now select each wire independently, preserving one
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

Five semantic tests pass in `borrow_frames`. The initial three tests failed on
the old non-translation Unsupported; the 16-visit test additionally failed on
lexical iteration ordering before the visit-order repair. The independent
metadata-admission regression intentionally still requires successful admission
and is currently **RED**: pinned reference 62a0cf0 rejects domain.workspace as
an unknown field despite accepted K89. The producer retains the specified field;
this is not independent checker acceptance. An upstream reader repair is queued
in the uncommitted cross-repo bead requests. Remaining producer mechanics are
`bennett-sched-vt8.2`; awo remains open until those obligations are complete.
