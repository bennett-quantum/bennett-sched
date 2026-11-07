# bennett-sched

Workspace handoff: [bennett/HANDOFF.md](../bennett/HANDOFF.md).

Scheduler and cost model targeting Bennett ISA 0.9 section 11, with Rust `rlib`
and C `cdylib` interfaces. The complete published contract remains the scope;
this is a partial implementation. See [docs/PLAN.md](docs/PLAN.md), the
[ABI contract](docs/ABI.md), and [quality evidence](docs/quality.md).

Requests copy their inputs. Responses own their reason, schedule and certificate
buffers until freed. The local header is [data/bennett_sched.h](data/bennett_sched.h).
Token identity bindings and the additive alias setter are implemented; the
loader negotiation ABI still needs a precise published protocol.

Implemented paths include source actions and adjoints, lifted oracle and local
procedure expansion, memory/allocator rows, finite-domain inverse-pair proofs,
translation-lemma borrow frames, exact rational costs and resource aggregation.
Schedules and certificates retain invocation contexts and generated ordinals.
Producer claims still require independent checking. Scalable construction and
pair proofs, general cleanup, native realisation models and classical
pseudo-location initialization remain open; see the quality record for their
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

The default suite retains the known pinned reference metadata-admission failure:
that interpreter rejects `token_aliases`. The reviewed ISA re-pin belongs to the
orchestrator. Ten slow exhaustive simulations run with `cargo test --offline
-j3 -- --ignored --test-threads=4` and are mandatory in the push gate.

`python3 -B scripts/conformance.py --write --require-complete` inventories every
published case and separates direct tests, equivalent witnesses and unexercised
work; it intentionally exits 2 while the inventory is incomplete. Complete
controlled-action basis comparison with the pinned reference passes. Scheduled
execution and independent certificate-claim replay are not claimed.

The push hook runs the shared tool at the committed `tool_sha` in `gate.toml`,
against the pushed SHA and pinned siblings. The orchestrator runs that full
gate; local tests and coverage measurements do not establish push verification.
