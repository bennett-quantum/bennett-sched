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
