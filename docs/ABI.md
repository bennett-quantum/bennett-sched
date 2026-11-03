# Scheduler C ABI and token identity bindings

`data/bennett_sched.h` exposes the ISA 0.9 scheduler ABI. Requests copy all input
buffers; response buffers remain owned by the response until it is freed. A
failed setter preserves the previous value. A failed schedule call clears its
output handle. Status errors describe invalid requests; successful calls return
OK, INFEASIBLE or UNSUPPORTED with an owned reason string.

K91 adds `bennett_sched_request_set_token_aliases(request, aliases_jcs, length)`.
The bytes must be a canonical JCS array of nonnegative integer labels. Labels
follow first appearance from zero, without gaps. Oracle subjects list their
primary token first, then token arguments in signature order. Action subjects
list only token arguments. Equal labels require equal validated budget tuples;
different labels can have equal tuples. Arity and tuple consistency are checked
before expansion. The setter copies the array and rejects malformed syntax
atomically with E_ARG.

Legacy token-free requests infer `[0]` for oracles and `[]` for actions. Legacy
requests containing token arguments require the setter and otherwise return
E_ARG. Every consistent supplied partition is supported by the binding layer.
Source forwarding, captures, fixed local procedures, cleanup matching, inverse
pair proofs and reversal retain actual identities across contexts. Each nested
context canonicalizes its own partition; a local label zero is not a shared
identity with another context's local zero.

Every emitted schedule carries `bindings.token_aliases`; invocation-bound
certificate claims and inverse-pair entries carry their partitions too.
`params_hash` still hashes only typed argument tuples. The schedule parser
requires the new binding and rejects a partition mismatch even when parameter
hashes agree. Pre-K91 portable templates need an explicit, documented rebinding;
their original bytes are never silently upgraded by the parser.

The ISA draft names `token_aliases/1` but its §10.1 advisor R21 note explicitly
forbids inventing loader negotiation pending **b-isa-spec-b3ui**. Therefore this
library does not publish a custom query or claim completed negotiated support.
The existing `bennett_sched_version()` compatibility probe is not capability
negotiation. Consumers must implement the eventual published negotiation before
relying on negotiated alias-aware launches. This does not remove the published
setter or identity semantics from scope.

Focused tests: `token_aliases`, `ffi`, `c_link` and the typed-argument case in
`qinplace`. The C tests exercise actual dynamic linkage, copied partitions,
legacy diagnostics, both equal-tuple partitions and certificate bindings. The
legacy C consumer also compiles against the older published header.
