# bennett-sched

## What this repo is

The scheduler and cost model behind the ISA spec's scheduling interface (sched 0.3, Bennett-ISA-Spec.md section 11). Linked by the plugin at compile time and by the VM at run time. Exposes a C ABI (cdylib + rlib).

It is one of several independent repositories in the Bennett reboot. It talks to
the others only through published specs.

## Where the spec is

Scheduling interface: Bennett-ISA-Spec.md section 11 (sched 0.3). Cost/target model: sections 8 (target 0.3).
The conformance suites for the versions in SPEC_VERSIONS.toml live in bennett-isa-spec.
Spec sources of truth: Bennett-ISA-Spec.md and Bennett-NorthStar.md in the Bennett.jl repo
until bennett-isa-spec is published.

## Rules

The one rulebook is bennett/docs/process/COMMON.md; it binds here. Gate: `scripts/pre-push`. Tasks: `bd` (`bd prime` lists its commands).
