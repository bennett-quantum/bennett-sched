# bennett-sched

Workspace handoff: [bennett/HANDOFF.md](../bennett/HANDOFF.md).

Scheduler and cost model targeting Bennett ISA 0.9 section 11, with Rust `rlib`
and C `cdylib` interfaces. The complete published contract remains the scope;
this is a partial implementation. See [docs/PLAN.md](docs/PLAN.md), the
[ABI contract](docs/ABI.md), and [quality evidence](docs/quality.md).

Requests copy their inputs. Responses own their reason, schedule and certificate
buffers until freed. The local header is [data/bennett_sched.h](data/bennett_sched.h).
Token identity bindings, the additive alias setter and the published
`bennett_sched_features` negotiation query are implemented; see the ABI contract.

Implemented paths include source actions and adjoints, lifted oracle and local
procedure expansion, memory/allocator rows, finite-domain inverse-pair proofs,
translation-lemma borrow frames, exact rational costs and resource aggregation.
Schedules and certificates retain invocation contexts and generated ordinals.
Producer claims still require independent checking. Scalable construction and
pair proofs, general cleanup, symbolic native-model verification, fixed-image
claim production and classical pseudo-location initialization remain open; see the quality record for their
evidence and contract boundaries.

The 219-recipe `ref@0.9` catalog is vendored specification data. Tests use only
the ISA checkout and reference binary selected by `gate.toml`. The spec commit
and committed file digest agree with `SPEC_VERSIONS.toml`. A missing materialized
pin fails with a diagnostic identifying the required revision; a live sibling
is never a fixture fallback. `BENNETT_REF_BIN` must remain inside that pin.

Build and test headlessly:

```sh
mkdir -p .tmp/development/tmp
env -u DISPLAY -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS -u BROWSER \
  TMPDIR="$PWD/.tmp/development/tmp" CARGO_TARGET_DIR="$PWD/.tmp/development/target" \
  cargo test --offline -j3 -- --test-threads=4
```

The controlled-action and selected borrow-frame regressions check emitted
schedule/certificate metadata admission against the pin. Borrow frames use the
published closed source/allocation occurrence objects and forward acquisition
ordinals; see [accepted ISA additions](docs/isa-additions.md). The scheduler
explicitly binds the immutable original catalog hash, which the published
contract preserves alongside the new r1 default. Ten slow exhaustive simulations
run with `cargo test --offline -j3 -- --ignored --test-threads=4` in the full tier
after landing.

`python3 -B scripts/conformance.py --write --require-complete` inventories every
published case and separates direct tests, equivalent witnesses and unexercised
work; it intentionally exits 2 while the inventory is incomplete. Complete
controlled-action basis comparison with the pinned reference passes. Scheduled
execution and independent certificate-claim replay are not claimed.

The push hook runs the shared tool at the committed `tool_sha` in `gate.toml`,
against the pushed SHA and pinned siblings. The orchestrator runs that full
gate; local tests and coverage measurements do not establish push verification.
