# bennett-sched

Scheduler and cost model targeting Bennett ISA 0.9 §11, built as a Rust
`cdylib` and `rlib` for plugin and VM consumers across the C ABI.
**Status: partial implementation.** The complete 0.9 contract remains the scope;
this repository does not claim full §11 conformance. See [docs/PLAN.md](docs/PLAN.md)
for the clause inventory, remaining algorithms and frame-schema decision.

All §11 C functions are exported. Requests copy their object, target, parameter
and string inputs; responses own their reason, schedule and certificate until
freed. Accessor buffers remain valid after freeing the request. Declaration
headers are in the published ISA repository and [data/bennett_sched.h](data/bennett_sched.h).
`bennett_sched_version()` is an additional compatibility probe returning 9.

The implemented path covers memoryless oracle construction and actions, plain/while loops and source adjoints,
qif, copy/XOR/exchange, local regions and coherent memory, action calls and
translation-lemma borrow frames. Native and approximate/exact target schemes
use deterministic sequential timing, class capacity and topology placement,
exact rational costs, imported quantities, occupancy and ordered lifetime peaks.
It emits JCS schedules and bound certificates with exact resource fractions and
34-digit upward displays. Producer claims require independent checking.

The complete published 219-recipe `ref@0.9` is vendored as specification data.
Accepted K82 governs named helper wire interfaces; all 16 binding cases, 54
interfaces and 86 published expansion digests pass without changing its hash.
Typed recipes/templates remain closed. Source version checking treats semantic
calls and intrinsic copy/load pairs atomically, checks dependency windows and
uses a sufficient causal factorisation proof for other workspace. Cleanup carriers
respect intrinsic boundaries and joint inverse ownership; primitive/intrinsic
recompute preserves carrier coordinates. Finite class/vertex search finds a
feasible assignment when an earlier assignment fails.

Layout/lproc construction, nested calls/domain checks, fresh semantic-call
recompute and input-history replay, qinplace, general cleanup and borrow proofs,
nested region coordinates, and objective optimisation remain `bennett-sched-ark`.
Varying invocation frame descriptors require the `bennett-sched-lzv` contract
decision; restricted T3 borrow certificates require the `bennett-sched-awo`
frame-domain decision. Encountered postponed features return `UNSUPPORTED` with a specific
reason and issue; constraint failures return `INFEASIBLE`; malformed requests
return a `bennett_status` error. The deterministic candidate is not claimed optimal.

Build headlessly with four compiler jobs:

```sh
mkdir -p .tmp/sol-sched/tmp
env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u BROWSER \
  TMPDIR="$PWD/.tmp/sol-sched/tmp" CARGO_TARGET_DIR="$PWD/.tmp/sol-sched/target" \
  cargo build --offline -j4
scripts/pre-push --working-tree
python3 scripts/conformance.py --write --probe-reference --require-complete
```

The last command intentionally exits 2 while conformance vectors remain
unexercised. [docs/conformance-report.json](docs/conformance-report.json)
inventories every published case and separates direct fixture tests,
equivalent witnesses and remaining work. All ten normative object hashes are
tested. The controlled-action trace is compared on its complete basis domain
with the reference interpreter's published 0.8 compatibility fixture. That
binary rejects 0.9 admission and cannot certify 0.9 schedule execution.

The push hook's default mode tests every supplied local SHA in a clean detached
worktree beside the repo. `--working-tree` is an explicitly labelled checkpoint
for the commit-owning runner and runs no git commands. Tests read published
fixtures from sibling `bennett-isa-spec`; `BENNETT_REF_BIN` may select its
published reference binary. No sibling implementation code is linked or copied.
