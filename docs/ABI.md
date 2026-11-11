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

K91 publishes `bennett_sched_features(const uint8_t **jcs_out, size_t *len_out)`.
It returns the immutable provider-owned JCS manifest
`{"abi":"sched/0.9","features":["token_aliases/1"],"schema":"bennett.abi-features/1"}`.
Its length excludes a terminator; the storage remains valid until provider unload.
Either NULL output returns E_ARG without writing either output. Repeated and
concurrent queries return identical bytes without requiring or mutating a request.

A loader must query this selected scheduler, validate the exact schema and sched
ABI, find `token_aliases/1`, and resolve both the setter and schedule entry points
in the same provider before alias-aware work. Missing or malformed capability
metadata is a loader compatibility error; it must preserve the unresolved
partition and must not retry with a tuple-only request. Launch providers require
separate negotiation. The manifest is deployment metadata and adds no artifact
fields or hashes. `bennett_sched_version()` remains a compatibility probe, not
capability negotiation. This implements the formerly missing published query;
no scheduler negotiation contract gap remains.

Focused tests: `token_aliases`, `ffi`, `c_link` and the typed-argument case in
`qinplace`. The C tests exercise actual dynamic linkage, copied partitions,
legacy diagnostics, both equal-tuple partitions and certificate bindings. The
legacy C consumer also compiles against the older published header.
