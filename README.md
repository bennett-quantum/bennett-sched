# bennett-sched

Scheduler and cost model targeting Bennett ISA 0.9 §11, built as a Rust
`cdylib` and `rlib` for plugin and VM consumers across the C ABI.
**Status: partial implementation.** The complete 0.9 contract remains the scope;
this repository does not claim full §11 conformance. See [docs/PLAN.md](docs/PLAN.md)
for the clause inventory, remaining algorithms and accepted contract additions.

All §11 C functions are exported. Requests copy their object, target, parameter
and string inputs; responses own their reason, schedule and certificate until
freed. Accessor buffers remain valid after freeing the request. Declaration
headers are in the published ISA repository and [data/bennett_sched.h](data/bennett_sched.h).
`bennett_sched_version()` is an additional compatibility probe returning 9.

The implemented path covers memoryless oracles, layout-backed root oracles with
address/load/store/bounds-check/allocator/free rows and lproc call/return rows, and actions, plain/while loops and source adjoints,
qif, copy/XOR/exchange, local regions and coherent memory, action and explicit-token oracle calls and
qinplace with complete finite-domain inverse-pair verification, and translation-lemma borrow frames. Native and approximate/exact target schemes
use deterministic sequential timing, class capacity and topology placement,
exact rational costs, imported quantities, occupancy and ordered lifetime peaks.
It emits JCS schedules and bound certificates with exact resource fractions and
34-digit upward displays. Producer claims require independent checking.

Load-time layout checks validate ranges, images, exact interface mappings and
pre-entry capacity. The state planner derives ordered byte/location descriptors,
reference counters, activation/frame sizes and sparse initial images. Lproc call/return
expansion preserves positional binding keys and uses the published activation recipes. Root memory reads preserve mapped
input identity, restore global images and include input regions in oracle claim
domains.

The complete published 219-recipe `ref@0.9` is vendored as specification data.
Accepted K82 governs named helper wire interfaces; all 16 binding cases, 54
interfaces and 86 published expansion digests pass without changing its hash.
Typed recipes/templates remain closed. Linked source builders validate typed header arguments and preserve the explicit oracle invocation channel across calls, bodies and templates; each tuple builds its own source trace and retains callee provenance. Source version checking treats semantic
calls and intrinsic copy/load pairs atomically, checks dependency windows and
uses a sufficient causal factorisation proof for other workspace. Cleanup carriers
respect intrinsic boundaries and joint inverse ownership; primitive/intrinsic
recompute preserves carrier coordinates. Fresh semantic-call replay keeps separate caller-leaf and call-occurrence counters, source callee adjoints and exact allocated pre-call histories. Finite class/vertex search finds a
feasible assignment when an earlier assignment fails.

Lifted semantic-call rows now bind independent tokens and source coordinates. Matched lifted uncalls have per-step cleanup carriers and fresh oracle-delta recompute, including fixed lproc instances. Remaining domain checks, general mutable-interface/borrowed input-history replay, general cleanup and borrow proofs,
and objective optimisation remain `bennett-sched-ark`. Accepted K83 action-region
support coordinates and recursive declaration collection are implemented and
verified under `bennett-sched-ecz`. Qinplace proofs currently use bounded exhaustive tables/replay; scalable proof methods remain `bennett-sched-a0v`. A host-workspace failure reports `BENNETT_E_NOMEM` and never substitutes a smaller domain. Bounded-memory
streaming remains `bennett-sched-2u4`: normative sum_db at B=3 currently exhausts
the explicit materialized leaf workspace and reports `BENNETT_E_NOMEM`.
Accepted K87/K90 resource and frame contexts are implemented: every reached invocation has its own records and the first canonical resource projection remains available. K89 general borrow proofs remain `bennett-sched-awo`. New K84–K93 adoption is tracked by `vt8`, including K91 ABI negotiation (`xzv`), whose setter and identity propagation are implemented; see [the ABI contract](docs/ABI.md). The draft forbids inventing negotiation while b-isa-spec-b3ui is pending. Classical pseudo-location initialization requires
`bennett-sched-r28`; its exact state/cost proposal is queued upstream. Borrow-owner
designation (`qm2`), explicit interface class choices (`4tj`) also have queued contract proposals. Differing call classes, controls and budgets retain complete context records. Encountered
postponed features return `UNSUPPORTED` with a specific
reason and issue; constraint failures return `INFEASIBLE`; malformed requests
return a `bennett_status` error. Inverse translation frames use the published adjoint-table rule. Raw-inverse cleanup selects in forward source space before reversing the selected expansion. The deterministic candidate is not claimed optimal.

Build headlessly with four compiler jobs:

```sh
mkdir -p .tmp/sol-sched/tmp
env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u BROWSER \
  TMPDIR="$PWD/.tmp/sol-sched/tmp" CARGO_TARGET_DIR="$PWD/.tmp/sol-sched/target" \
  cargo build --offline -j4
cargo test --offline -j2 --test pipeline -- --test-threads=2
python3 scripts/conformance.py --write --probe-reference --require-complete
```

The last command intentionally exits 2 while conformance vectors remain
unexercised. [docs/conformance-report.json](docs/conformance-report.json)
inventories every published case and separates direct fixture tests,
equivalent witnesses and remaining work. All ten normative object hashes are
tested. The controlled-action trace is compared on its complete basis domain
with the published reference interpreter. Its debug CLI now admits the actual 0.9 fixture and our emitted schedule/certificate metadata, and rejects an absent source point. This is metadata admission; scheduled execution and rigorous certificate-claim replay remain unclaimed. Its release binary still requires the 0.8 compatibility fixture.

The push hook delegates to the shared tool extracted from the meta repository's
committed objects at the tool_sha in gate.toml. It checks the pushed SHA with
siblings and the reference binary at their declared pins. The orchestrator owns
full gate runs; scripts/check-working-tree remains an optional local checkpoint
for the runner. Its result does not establish pinned push verification.
The tests assert this repository's config/stub contract; shared tool self-tests
own the isolation and ref protocol. The gate ISA pin currently needs alignment
with SPEC_VERSIONS.toml before a complete pinned result can be claimed.

Tests read published fixtures from sibling bennett-isa-spec;
BENNETT_REF_BIN may select its published reference binary. No sibling
implementation code is linked or copied.
