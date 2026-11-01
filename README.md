# bennett-sched

Scheduler and cost model targeting Bennett ISA 0.9 §11, built as a Rust
`cdylib` and `rlib` for plugin and VM consumers across the C ABI.
**Status: partial implementation, blocked on a reference-library binding
decision.** The complete 0.9 contract is the intended scope; this repository
does not claim full §11 conformance.

All §11 C functions are exported. Requests copy their object, target, parameter
and string inputs; responses own their reason, schedule and certificate until
freed. Accessor buffers remain valid after freeing the request. Declaration
headers are in the published ISA repository and [data/bennett_sched.h](data/bennett_sched.h).
`bennett_sched_version()` is an additional compatibility probe returning 9.

The implemented path schedules primitive actions, static loops, qif,
copy/XOR/exchange and supported action calls using native target gates or exact
target schemes. It uses deterministic sequential timing, class capacity and
topology placement, exact rational costs and per-class resource rows. It
emits JCS schedules and bound certificates with exact resource fractions and
34-digit upward display values. Numerical claims are producer assertions for
an independent checker; they are not checker verdicts or proof labels.

The constructor engine reads the complete published 219-recipe `ref@0.9`
document as specification data. Its envelope hash is checked. Typed closed
recipes, captured bodies/templates, images, ordinals, allocation identities and
inverse leaves are implemented. Some published helpers have undeclared wire
operands despite §11.1 forbidding implicit bindings. `bind.value.add` is a
minimal reproduction. No caller-environment fallback is invented to make that
catalog appear conformant; see `bennett-sched-enz` and [docs/PLAN.md](docs/PLAN.md).

Oracle/lproc source construction, staged nested-domain checks, whole-cleanup
recompute, general cleanup proofs, qinplace, borrowed source frames, local
region adapters, while-loop flags, approximate schemes and varying invocation
frame descriptors remain tracked in `bennett-sched-ark`. Encountered postponed
features return `UNSUPPORTED` with the named feature and issue. Constraint
failures return `INFEASIBLE`; malformed requests return a `bennett_status`
error. Other inputs, budgets and arguments are never silently substituted.
Imported-model quantity conversion and global placement/objective search also
remain open. The scheduler currently chooses a deterministic candidate rather
than claiming an optimal objective value.

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
