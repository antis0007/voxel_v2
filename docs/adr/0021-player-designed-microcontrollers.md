# 0021: Player-designed microcontrollers (H1, first H2 slice)

Status: H1 implemented; a first planar fabrication slice (H2) implemented
with game-preset durations and materials.

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

## H1 completion

- Sockets on pumps (Tank Fill: tanks on the fluid network), processors
  (Crate Fill: crates on the item network, fullest compartment) and arcane
  piles (Heat Guard: own core heat; a held pile has its rods in, coolant
  still works). A part installs only where its task fits.
- Watchdog module: with one on the peripheral bus the core restarts after
  a brownout; without one a brownout while running hangs the core (output
  off) until the player resets it. Each wake spends two instructions
  kicking the watchdog. `Model.recovers` defaults to true for revisions
  released before the module existed. `COMPILER_VERSION` is 2.
- Compiler messages follow `crates/app/src/egui_ui/UI_RULES.md`: states and
  counts, no prose.

## H2 slice: wafer lots

- `fab::route(layers)` is an ordered planar route: clean, oxidize, pattern
  diffusion, implant, anneal, gate, contact, then per metal layer a
  dielectric and via mask and the metal itself, passivate, probe, package.
  Every exposure follows a resist coat; resist strip is a sealed organic
  clean and removes no oxide. No real recipes or mixtures appear.
- A mask set is content-addressed from the revision fingerprint and the
  patterned sites per layer, made once and reused by every lot.
- A lot is billed up front (wafer, mask blanks on the first lot, resist,
  solvent, dopant, furnace gas, metal, filters for the clean hood) and then
  advances at game time from the bay clock. Each step writes a record from
  `mix(lot seed, step)`: film thickness, overlay, dose, residue. Outcomes do
  not depend on service cadence and survive a save mid-route.
- Yield `Y = exp(-D·A)` with D in defects/cm², raised by each contamination
  event; an exposure whose overlay exceeds the process tolerance opens a
  fraction of that layer's features. Speed bins A/B/C come from a per-die
  critical-path spread. All bins run the same firmware; the bin rates the
  clock. Energy per step is not drawn in this slice.
- Labs are content rows: Open Bench (18/cm², sigma 8%) and Clean Hood
  (4/cm², sigma 3%, filters per step).

## Rejected

- A new factory `Kind` for a controller block: needs a machine-table bump and
  a placement path; a socket on existing machines is smaller.
- A sidecar file next to the save: the background save pipeline commits a
  manifest atomically, and a second file could disagree with it.
- Per-cycle or per-gate simulation, and an instruction interpreter: firmware
  tasks with a fixed instruction mix give honest timing without a scripting
  runtime.
