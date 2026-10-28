# bennett-sched

The scheduler and cost model behind the ISA spec's scheduling interface (sched 0.3, Bennett-ISA-Spec.md section 11). Linked by the plugin at compile time and by the VM at run time. Exposes a C ABI (cdylib + rlib).

Depends (by published spec, not by shared code) on: bennett-isa-spec only.

Part of the Bennett reboot workspace. Status: skeleton; see HANDOFF.md.
