# 0021: Player-designed microcontrollers (H1 slice)

Status: first slice implemented. Prepared blanks only; lithographic
fabrication (masks, wafer lots, binning) is the next phase.

## Decision

- `crates/chip` (`engine_chip`) owns the design language as pure data and
  functions: the verified module registry (`assets/chip_modules.ron`), the
  site-grid floorplan with typed ports and named nets, multi-layer wires with
  explicit vias, a bounded deterministic router, design checks with one
  concrete fix each, first-order characterisation, firmware tasks and the
  event-driven runtime state of an installed part. It depends on `serde` and
  `ron` only, both already workspace dependencies. No new external dependency.
- The app owns installation, power, persistence and UI
  (`world/factory/chips.rs`, `egui_ui/chip_*.rs`). The first controllable
  machine is the pump; sensors are the tanks on its fluid network.
- Revisions are immutable: a release copies the draft, its FNV-1a fingerprint
  (design text + `COMPILER_VERSION`) and the runtime `Model`. Installed parts
  run their revision's frozen model; later edits and compiler changes do not
  alter them. Reports shown in the UI are recomputed, not saved.
- Hardware figures are derived, never authored: clock from the routed
  critical path, wake work from the firmware mix and whether an ALU is on the
  core's ALU bus, response from the timer's shortest period and ADC
  conversions, power from module leakage, clock-net switching and per-wake
  energy. Equations are in `characterize.rs`.
- Runtime: a controller is serviced only when due (min-queue of wake ticks at
  the 120 Hz fixed step); idle gaps are charged at the next service. Energy is
  paid from the machine's buffer in whole 250 J units; an empty buffer browns
  the part out and holds the machine off. A controller cannot move more water
  than the pump delivers.
- Save 34 appends the chip library, installed controllers and the bay clock
  as length-prefixed RON after the dropped stacks. Saves 1-33 load with an
  empty library. Decode rejects unissued serials and a package present twice.

## Rejected

- A new factory `Kind` for a controller block: needs a machine-table bump and
  a placement path; a socket on existing machines is smaller.
- A sidecar file next to the save: the background save pipeline commits a
  manifest atomically, and a second file could disagree with it.
- Per-cycle or per-gate simulation, and an instruction interpreter: firmware
  tasks with a fixed instruction mix give honest timing without a scripting
  runtime.
