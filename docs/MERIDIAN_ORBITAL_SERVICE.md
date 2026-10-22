# Meridian Lift: first engine increment

The Meridian Lift is an old orbital commons whose atmospheric corridors still
operate. It transports correspondence, industrial feedstock, tension braid and
living archives. Published manifests disclose handling requirements, not private
messages or the coordinates of distant civilizations. Its custodians conserve
mass, protect sealed cargo and acknowledge a narrow service return. This is a
local transport service, not a signal announcing the planet to the universe.

The visual reference is inhabited speculative-fiction infrastructure: long
suspension lines, ceramic cargo shells, tension forks and a carriage descending
through the atmosphere. Three sizes share one machinery family: Needle (12 m),
Talon (32 m), Harbour (80 m). This is an atmospheric elevator interpretation;
there is no rotating momentum-exchange capture simulation in this increment.

## Implemented

- Fixed seeded corridors across the world, independent of loaded chunks and
  player movement. The starting region has one discovery corridor at metre
  coordinates (1100, -900); other 4096 m cells have a 46% corridor probability.
- Each corridor has its own seeded phase on a three-world-day timetable.
  A visit lasts 0.22 days: 40% descent, 20% hold, 40% ascent, with smooth velocity
  joins. An alignment notice precedes it by 0.025 days (36 world minutes).
- The upper carriage remains on its tether between visits. The low endpoint is
  360 m above sampled terrain/water; these are sealed atmospheric holds.
- World-space suspension braids and three carriage scales rendered through the
  ordinary scene mesh, depth buffer, atmosphere and weather. Orbital meshes have
  their own 5–6 km coverage veil rather than disappearing at resident chunk range.
  Thin braid geometry uses two-pixel raster support with physical-width coverage
  to avoid broken subpixel lines; it does not acquire a distance-dependent glow.
  Moving orbital geometry does not enter terrain streaming or collision. The
  ground receiver described below uses normal editable, colliding world blocks.
- E / inventory > Orbital receives nearby identities, bearings, range, manifest,
  current phase and time until its next transition. Radio and resonant returns
  request an additional inspection descent. The first increment makes this
  survey receiver available directly in the inventory; equipment progression is
  deferred. The two returns reach the same ledger and have the same timing.
- One outstanding request per world. Requests cannot interrupt a visit or overlap
  the next scheduled approach. A request never changes the route, payload locks
  or cargo ownership and yields no resources. There is no idle maintenance tax.
- Save version 24 persists the requested corridor, world calendar start and
  channel. Earlier saves default to no request. Natural traffic reconstructs
  from world seed and the existing saved calendar, not elapsed wall time.
- Save version 25 adds the ground receiver, paid shipment deadline and permanent
  completion flag. Version 24 and older saves start without a registered receiver.
- Bounded discovery (25 candidate cells) and a 64-entry unsaved terrain-height
  cache. There is no background simulation per generated corridor.

The timetable's phase and corridor population provide global variation; this
increment implements one coherent event family. It does not yet implement
distress, salvage, merchants, factions or interception.

## Ground receiver and commissioning shipment

One receiver may be registered within 96 m of the starting corridor at metre
coordinates (1100, -900). Prepare a flat, dry, solid foundation, face it from four
metres away, and choose **Inventory > Orbital > Build receiver**. Cost: 8 iron
ingots and 4 copper ingots. The footprint is approximately 2–3 m wide depending
on voxel scale, with three metres of clear height. No existing blocks are removed.
Protected factory cells and unavailable edited chunks reject deployment before
payment. Its stone footing and workbench console are real editable world blocks;
antenna fittings are scene geometry. The registration coordinate stays fixed.

Aim at the registered console or stone plinth and press F to open the service ledger. Operate it
within 6 m. Contract **ML-C01** costs 4 copper ingots and 1 resonant crystal and
delivers **8 steel ingots and 4 ancient circuits**, once per world. The contents
are a finite commissioning parts allocation, not an infinite exchange recipe.
Existing inventory items and units are used; no new material or machine IDs.
Parts use a separate sealed maintenance capsule even on conservatory visits;
the biological archive's quarantine is never opened for this contract.

Ordering reserves a free descent when possible, otherwise uses an existing or
the next regular service hold without interrupting another corridor's booking.
The final cargo capsule descends from the atmospheric platform to the receiver
over 0.012 world days and appears as a sealed crate on arrival. This moving mesh
is presentation; the paid cargo entitlement lives in the saved receiver ledger.
The receiver quotes world minutes to delivery before payment, then shows the
remaining time for the saved shipment after ordering.

Cargo waits if the player leaves the region or misses the visit. An unloaded
region needs no ticking: arrival is reconstructed from its saved deadline and
world calendar. **Collect sealed cargo** inserts the complete bundle into a
temporary inventory copy, then commits inventory and the completed ledger
together. A full pack leaves all cargo secured and unclaimed. Completion cannot
be reset by another order, clock reversal, reloading or rebuilding the console.

If the console is mined, neither paid cargo nor completion history is erased.
Replace the workbench at its registered cell or use **Restore console** nearby
for 2 iron ingots when the cell is empty and supported. Replacement never creates
a second contract. Normal world saves retain the block edits, inventory, booking
and contract ledger in the same existing checksummed save transaction. Receiver
actions request a save immediately; unsuccessful disk saves still require the
engine's normal persistence error handling.

Rideable local coordinate frames, passenger boarding, general freight inventories,
arbitrary receiving ports, automation, moving voxel structures and whole-city
collection require their own increments. Platforms remain observation geometry.

## Review

Rust/shader validation: `cargo check --offline --target-dir target`.
No tests or hosted CI are needed for this iteration unless requested.

Use a fresh build for a visual review. Open Orbital, request a return on a free
corridor, and look along its listed bearing. The world clock controls the wait;
paused/frozen time does not advance service. Check descent, atmospheric hold and
ascent, and save/reload during a booking before calling persistence accepted.
Compile success alone does not establish visibility, UI layout or save/reload
acceptance. The fixed-pose benchmark can capture the origin corridor from near
(650, -450), looking toward (1100, -900). Reproducible Harbour hold:

```powershell
$env:VOXEL_CAPTURE_DIR = "$PWD/target/meridian-review"
./target/release/minecraft_style_voxel_prototype.exe --benchmark-world=hills `
  --benchmark-seed=74 --benchmark-weather=clear --benchmark-capture-frames `
  --benchmark-view-chunks=4 --benchmark-width=1280 --benchmark-height=720 `
  --benchmark-terrain-camera=650,-450,20,0.785398,0.46 `
  --benchmark-review-day=1 --benchmark-review-time=0.6292005 `
  --benchmark-warmup=3 --benchmark-duration=35
```

For the receiver page, also set `VOXEL_REVIEW_PANEL=8` and pass
`--benchmark-capture-hud`. Remove the panel environment variable afterward.

For matched terrain/lighting benchmarks, set `VOXEL_REVIEW_NO_ORBITAL=1` to
exclude orbital scene geometry. This override only applies when the invocation
contains a `--benchmark-` argument; it does not disable traffic in normal play.
Orbital rendering intentionally applies to older generator versions as a runtime
feature. Generator version alone does not establish a terrain-only comparison.

### Local evidence (2026-10-21)

Offline Cargo check and the final release build succeeded, including composed
WGSL validation. The release build needed TMP/TEMP redirected to the writable
`target/orbital-compiler-tmp` directory after the sandbox temp archive rename
failed. No tests or hosted CI were run.

The fresh release was launched and the Harbour hold inspected at
`target/meridian-final/camera.png`. An initial dashed tether was replaced with
filtered physical coverage and recaptured. The receiver page was inspected at
`target/meridian-receiver/bench-003.png`, with available return buttons and
multiple corridor forecasts. These captures establish visibility and page
layout, not complete-cycle motion, clicked requests or save/reload acceptance.
The optional local Kev review was unavailable (localhost socket error 10013).

### Receiver increment validation

The receiver implementation passes offline Cargo check. An optimized
`release-fast` build succeeded outside the sandbox; the regular release profile
was blocked by a stale build-script fingerprint lacking
`VOXEL_VALIDATED_WORLD_FNV`. The sandbox additionally denied Cargo's workspace
input scan for `release-fast`; running that local build outside it resolved the
scan error. No renderer source was changed to bypass fingerprint validation.

No native run, new visual capture, tests or hosted CI were performed for this
increment. Deployment, capsule arrival, full-pack collection and save/reload of
the completed contract still require gameplay verification. Earlier screenshots
in this document predate the receiver and do not establish those checks.

### Native receiver acceptance (2026-10-21; supersedes pending checks above)

Passed the scripted native-game scenario in
`reviews/meridian-receiver-verified-20261021/result.txt`. The game rendered the
receiver and capsule, invoked the production receiver actions, saved through the
normal asynchronous checkpoint worker, loaded through the production decoder,
and restarted a fresh game process from that checkpoint. Review inputs supplied
materials, filled the pack and moved the clock to the start of the final capsule
descent; they did not grant the contract reward or set its completed flag.
Actions were invoked through the confined review interface, not manual clicks.

Verified: exact deployment/payment, visible capsule descent and arrival, partial
first-stack rollback, failure of the second cargo stack with no partial payout,
one successful full-bundle collection, two completed checkpoint reloads, console
restoration, and rejected duplicate collection/order/deployment after both reload
and process restart. Final stock remained **8 steel ingots and 4 ancient circuits**.
The current shared save writer is version 26; this verification did not change
the save format or reset the receiver ledger.

The physical capture exposed the console being shielded by its own footing.
`receiver::focused` now accepts the actual registered stone plinth as well as the
console, within the existing 6 m reach. Replaced/obstructing unrelated blocks do
not become interaction targets. The rerun passed the grounded ray check and
shows the F prompt on the front footing in `07-arrived-physical.png`.

Scope: Superflat generator 25, one-metre voxels, indirect lighting disabled,
1280×900 capture. An earlier default-generator/indirect-enabled native run lost
its GPU device during terrain loading; renderer-default acceptance remains open.
No renderer changes or performance claims were made for this receiver work.
No personal saves were used. All owned review processes have exited.

Reproduce locally with `tools/Review-MeridianReceiver.ps1`; it requires an
up-to-date `target/release-fast/minecraft_style_voxel_prototype.exe`, creates a
fresh isolated review directory, restores its environment overrides, and stops
only the native processes it starts. The opt-in `src/receiver_review.rs` fixture
is gated by benchmark mode, `VOXEL_RECEIVER_REVIEW=1`, and a save directory exactly
matching the capture directory's `disposable-saves` child.

Implementation: `src/world/orbital.rs`, `src/mesh/models/orbital.rs`,
`src/egui_ui/orbital.rs`; narrow integration in world, storage, scene build,
inventory, main and the world shader. The shader coverage marker is surface.y =
2 on machinery/glow families 41/40 and the dedicated filtered tether family 61;
it does not change voxel materials. Tether surface.w stores physical coverage.

### Atmospheric slewing and radiator deployment (2026-10-21)

The suspended machinery now slews through a seeded 149-degree turn beneath a
fixed guide collar, with up to 18 degrees of bank and 10 degrees of pitch during
travel. It unfolds hinged radiator wings and eases into its berth heading before
the hold. Archives have paired radiator banks; smaller carriers have one bank on
each side. The braid remains world-anchored while the deck rotates about its
upper suspension joint. Ascent reverses the manoeuvre. The player's camera is
unaffected.

All angles and hinge deployment derive from the existing timetable's lowering
fraction and corridor key. There is no new animation state, random draw, save
field, contract change or extra chunk allocation. Existing atmosphere, cloud
occlusion and filtered cable coverage still apply.

Native visual review: `reviews/meridian-descent-20261021-verified/descent-00.png`
through `descent-05.png` record one continuous scheduled descent from a fixed
camera; `reviews/meridian-descent-20261021-landscape/bench-001.png` shows the
settled silhouette above the coastline. The moving sequence shows rotation,
deployment and increasing apparent scale. Its final frame reaches the lower
edge of the fixed upward view; the separate landscape view frames the berth.
Review scope is Gen25 hills, seed74, clear weather, indirect disabled, 1280x900.
Default-renderer stability, night/storm appearance and native ascent playback
were not reverified in this increment. All owned review processes exited.

`cargo build --offline --profile release-fast --target-dir target` passed. An
initial incremental build was refused at startup by the existing shader receipt
guard. Rebuilding with `CARGO_INCREMENTAL=0` produced matching validator/runtime
fingerprint `1f0a4904da0c5c06`; the guard and renderer source were left intact.
No tests or hosted CI were run. Reproduce the moving review with
`tools/Review-MeridianDescent.ps1`, which confines its fixture to disposable saves
and restores its environment. It invokes the normal timetable and clock rather
than supplying mesh poses.

### Orbital structure and finish pass (2026-10-22)

Every braid now terminates in a modelled upper anchorage at the existing 18 km
endpoint. Courier corridors have compact uncrewed relay buses and paired arrays.
Freight and archive corridors add 200 m and 420 m diameter segmented habitat
rings, four recessed illuminated modules, bearing spokes and a separate fixed
service spine. Rings make eight turns per saved calendar day; arrays stay fixed.
These are stratospheric service structures of the orbital commons, not a claim
that an 18 km altitude is a ballistic orbit. They remain distant scene models:
no interiors, collision bodies, simulated occupants or orbital mechanics yet.

The carriage now has four tension braces and a recessed bridge window. Dedicated
model family 62 distinguishes matte ceramic, metallic alloy and dark radiator
finishes. Its UVs are baked before rigid animation. All detail uses the shared
5 cm pixel lattice and filters to its average at distance, including one-texel
panel joints. This bypasses the generic world-space machinery grain and dither
for those parts. Normal lighting, atmosphere/cloud occlusion and the existing
5–6 km horizontal visibility fade remain active; the distant tether retains its
dedicated filtered-coverage path. No vertex-layout or shader-composition change.

An analytic suspension response adds small damped motion to the planned slew,
using sqrt(g / suspension length) for its frequency. Its driving envelope closes
at both ends of the travel leg, so time skips cannot accumulate spring energy.
This is kinematic presentation, not a rigid-body solver. The cargo capsule also
weathervanes and sways on its final line, settling to its original exact pad
position and orientation at arrival. Receiver stock, timing, claim conditions
and persistence are unchanged; no save-version bump was needed.

Final native evidence:

- `reviews/meridian-fidelity-final-20261022/`: six chronological descent captures,
  a close carriage view, and two upper-habitat captures four seconds apart. The
  latter show the ring advancing while the arrays and central spine stay fixed.
- `reviews/meridian-relay-final-20261022/`: seed 0's smaller uncrewed relay.
- `target/meridian-fidelity-final-build.log`: successful offline release-fast
  build, validating 16 WGSL programs; world fingerprint `e4d66ef10f9273d0`.

Both runs used Gen25 hills, clear weather, indirect lighting disabled, 1280x900
and disposable saves. All owned review processes exited. Default renderer,
storm/night appearance, physical contact, and the updated capsule motion were
not reverified in this pass. Performance has not been benchmarked comparatively.
No tests or hosted CI were added or run. The fixture only moves the review
camera/calendar and never supplies habitat mesh poses or grants rewards.

Reproduce with `tools/Review-MeridianDescent.ps1`; `-Seed 0 -AnchorageOnly`
reviews the courier relay. Default seed 74 includes descent and habitat views.
Implementation is confined to the orbital model/module, its new
`src/mesh/models/orbital/architecture.rs`, narrow world shader material handling,
and the opt-in native review fixture/harness.

### Communications progression and practical lighting (2026-10-22)

Inventory → Orbital now includes a persistent communications branch. This is
equipment installed for the world/player, not new inventory item types or a
repeatable XP purchase. Knowledge is a finite set of decoded concepts:

| Level | Equipment | Build cost | Capability / prerequisite |
| --- | --- | --- | --- |
| 1 | Crystal receiver | 2 copper + 1 crystal | 1.5 km reception; listening teaches carrier waves |
| 2 | Valve transceiver | 4 copper + 2 iron | Requires carrier knowledge; 3 km reception, 1.5 km transmission |
| 3 | Superheterodyne set | 4 copper + 4 iron + 1 crystal | Requires a verified handshake/modulation; 6 km reception, 3 km transmission |
| 4 | Directional dish station | 8 iron + 6 copper + 2 steel | Build outdoors on a flat supported site four metres ahead; aim at a corridor; operate within 8 m for a 6 km uplink |
| 5 | Resonant phased array | 4 steel + 2 ancient circuits + 2 crystals | Requires directional tracking learned through an aimed dish handshake; strongly reduces storm interference |

Costs refer to ingot/resource stacks. Tier 4 includes the physical workbench-core
console, stone footing, curved segmented reflector, feed supports and aimable
head. Tier 5 upgrades its feed markers. Its unique anchor, level, knowledge and
selected target survive saves. Aim is initially unset; an ordinary portable
handshake cannot teach directional tracking. A missing console can be restored
for 2 iron without resetting research. There is one registered dish per world.
Away from it, the player retains the level-3 portable set. Dish gain requires
an intact loaded console and twelve metres of clear loaded overhead cells; this
bounded clearance check is not full long-distance RF ray tracing.

The published carrier list provides automatic tuning presets. Link quality
depends on equipment, distance and storm intensity. Weak sets can lose a storm
uplink; the advanced array has coherent reception. Exact schedule changes and
manifests require reception. The backend validates uplinks independently of the
UI. Private traffic remains encrypted. Resonant returns remain the wand route;
the existing receiver shipment uses its dedicated service port, so this new
branch does not strand a paid commissioning delivery.

Save format **28** adds the radio fields after the receiver ledger. Header and
payload both use 28; versions 1–27 retain their prior layout and start the new
radio branch untrained. The first disposable native run caught a mismatched
header; this was corrected before acceptance. No personal save was used.

Night review exposed decks and uncrewed buses disappearing into the sky.
Protected warm deck lights, cyan cargo bay lights, illuminated equipment bays,
paired navigation markers and eight archive modules now describe their shape.
The fixed service crown/antenna remains separate from the rotating ring. Hulls
and radiator panels retain normal material lighting and atmospheric occlusion.
These are emissive fittings, not an added dynamic-light/shadow simulation.

Radio acceptance is recorded in `reviews/meridian-radio-verified-20261022/`:
all five paid levels, exact aggregate cost, finite knowledge, rejected locked
upgrades/free uplinks, explicit dish aim, weather/range/roof restrictions,
paid repair, two checkpoint reloads and a fresh process restart. The real save
worker and production decoder were used. `19-dish.png` and `21-radio-ui.png`
show the installation and controls. Reproduce with
`tools/Review-MeridianRadio.ps1` using an up-to-date release-fast executable.
The final extended run in `reviews/meridian-radio-uplink-verified-20261022/`
also accepted a real radio alignment request through the gated production entry
point, then compared the complete orbital ledger across checkpoint reload.
Fresh-process restart retained level 5, knowledge, dish and target.

Normal-renderer visual acceptance:

- `reviews/meridian-night-accepted-20261022/`: clear-night carriage descent,
  ground view, close detail, upper archive and ring rotation.
- `reviews/meridian-relay-storm-accepted-20261022/`: courier relay during a night
  storm, including two upper-anchorage poses.
- `reviews/meridian-day-storm-accepted-20261022/`: daytime-storm carriage descent,
  ground view and upper archive/ring rotation.

These runs used the then-current default generator **29**, the normal renderer
with **indirect lighting enabled**, 1280x900 and three view chunks. Captured shot
receipts record `indirect=true`, time 0.9650 at night or 0.6292 by day, and
cumulonimbus 0.820 for storms. No legacy generator or indirect-disable flags were
passed. Inspection cameras visit the upper anchorage; those close-up images are
not claimed to be its appearance from the surface. All captures were inspected.
Earlier Gen25 captures do not establish this acceptance.

The reruns exposed a real default-renderer panic in
`src/render/streaming/edit_proxies.rs`: the shared coordinate set also contains
quality-only columns, but the priority sorter indexed the persistent edit map
unconditionally. It now obtains age from the edit record or quality scheduler,
with the current tick as fallback. Scheduling and ownership limits stay intact.
The accepted matrix completed after this fix. The Windows review command-file
writer also now replaces complete commands atomically to avoid a reader/open
race; this tooling repair does not change gameplay.

`reviews/meridian-receiver-v28-verified-20261022/` reruns the full cargo scenario
on save version 28: deployment/payment, visible descent, both full-pack rollback
cases, one claim, two completed reloads, console restoration and fresh-process
duplicate rejection passed. The final stock remained 8 steel and 4 circuits.
No personal saves, hosted CI or unit tests were used. Offline release-fast
builds passed with build-time validation of all 16 composed shader programs.

See [MERIDIAN_PHYSICS_REFERENCE.md](MERIDIAN_PHYSICS_REFERENCE.md) for the
2025/2026 research, physical versus apparent dimensions, and reproducible
surface-view calculations. Current 18 km structures are explicitly atmospheric
magitech service hardware. A real orbital skyhook is not yet rendered or
dynamically simulated; the regular scene far plane is only 24.576 km.
