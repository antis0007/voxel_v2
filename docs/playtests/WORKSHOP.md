# Workshop friends playtest

Windows, 2–4 players. Co-op survival, shared building and machine networks.
The default uses the game's existing session-code relay, with no Steam login.
Everyone needs the identical package. Steamworks requires a separate native package.

## Start

1. Host runs `Host.cmd`, enters a player name, and waits for the world to load.
2. Copy the eight-character session code shown by the session/chat to friends.
3. Friends run `Join.cmd`, enter their own name and the session code.
4. `Server.cmd` runs a windowless server. Players join its printed code.

The default relay is the game's configured service. A private LAN relay can be
selected with `-Lobby IP:27021` on Host/Server and Join. The lobby executable is
`voxel-lobby --bind IP:27021`. This package does not deploy or change that service.
Steamworks host/join is available with `-Transport Steamworks`; Steam must be
running and signed in and the host's address is `steam:ID` (development App ID 480).

The host's world is `data/world/workshop.ron`. Guest identities live in
`data/player-NAME`. Keep the same name/data folder on return. Copying a package
to friends should happen before launching it, so it contains no player keys or saves.
World and profile files are local to this package. Regular game saves are separate.
Close through the game menu; a dedicated server accepts `quit` to save and stop.
The server writes an autosave about every 10 seconds when state changes.
Inventory, combat and objective progress also count as changes. Only one Host or
Server can open the same package world at a time.

PvP and spawn protection start off. Building and gathering at camp are shared.
The first launch writes `data/world/server.properties`; later launches retain edits.

## The game

The party has two connected objectives. They can be tackled in either order.

* **Reclaim the workshop.** Follow the compass to the nearby workshop and lit
  mine. Craft and equip a Stone Pickaxe at the bench with three cobblestone and
  two planks from your starter kit. Use it to gather copper, coal, lumen fibre
  and crystal. Use the smelter and the inventory's wand recipes. Prepare food,
  descend together, clear the guardian vault, and return to the workshop bench.
  Melee and engineering
  solutions count too; a particular weapon is not required for completion.
  Craft a Lumen Wand or copper weapon, or equip an assembled melee tool, to
  advance the compass to the mine. Select the wand in your hotbar before casting.
* **Establish workshop supply.** Gather and craft a generator, cables, electric
  furnace, item pipe and storage. Put the combustion generator outside or vent
  its exhaust outdoors. Leave a service aisle between the door and the line.
  Connect and fuel the power network. Right-click a machine input to load one
  item, or left-click to take one back. **Load All** fills matching inputs. Feed
  iron ore (or crushed iron dust) into the furnace and route its output through an
  item pipe into storage. Iron from the electric furnace, steel from the Alloy
  Furnace or Blast Furnace, and bio-ethanol from the Cultivator all count when
  they reach storage through a pipe. Pipe modes and face roles are in the
  machine panel. The recipe browser shows material costs.

Pipe-delivered resources are in storage's **Machine** tab. The ordinary slot
grid holds loose items; the machine panel exposes its resource buffer.

The inventory's Workbench tab shows the shared workshop checklist. Clearing the
vault and returning home completes the expedition. A supply delivery completes
the factory objective. Both together finish the slice. Continue building,
processing ore and exploring afterward; the world does not reset.

Items and equipment are personal. Loot caches and machines are shared. Supply
each other through storage or dropped stacks. The server resolves cache actions
in order against current contents. Death retains the normal survival rules:
recover dropped equipment, or use a friend's help. Workshop bench interaction
sets a personal respawn point when no enemy is nearby.

## Default controls

WASD move; Space jump; Ctrl sprint; Shift sneak; G interact; E/Tab inventory;
left mouse mine/melee; right mouse place/use food; R cast; T next spell; M map.
Bindings in Options take precedence. The compass points toward the current
workshop task. Recipes and machine panels are available through the inventory.

## Single-player continuation

Projects → Machinery → Orbital Supply links to the two station orders and pins
Meridian Terminal. The Station Salvage Manifest lists the locker contents and
links to builds that use them. Choose one supply order for Mithril.

- **Steel.** Run two Alloy Furnace batches with four iron ingots and two coal.
  Pipe the output to storage to prove the line, then take four steel ingots to
  the station.
- **Bio-ethanol.** Connect a powered Mycelial Cultivator to storage with an item
  pipe. Use the four-unit fuel order in Production and feed it eight biomass and
  one lumen fibre. Withdraw the four bio-ethanol from storage's Machine tab.

Board the lift, cycle both station hatches, and place one order at Station
Freight. Close the cabin hatch before departure. Use **Pin Cargo** in Projects
to find the shipment at the lift base or the selected ground receiver. Open a
Battery Bank with Bronze or Resonant cells and choose **Install Mithril**
for 64 kWh. The first bank upgrade costs
two bronze ingots, one induction coil, and one iron ingot.
Wind a coil at the workbench from two copper ingots and one lumen fibre. The
powered fabricator yields two coils from those materials.

On Normal, compare the routes from separate fresh worlds with the same seed.
Track time spent gathering and building separately from time spent waiting for
machines or freight. Note where you got lost, ran short on supplies, or had to
repeat work. Check that either route can finish the battery upgrade with the
materials it teaches you to make.

## Package identity

The current executable hash and packaged file hashes are in `build.json`.
The launcher checks its executable against that manifest. Share the fresh zip
before launching it; do not redistribute a package containing `data`.
Extract updates into a separate folder and keep the previous folder and saves.

Recent source changes address conflicting LOD requests, retired horizon owners,
near-terrain refinement and repeated shadow invalidations. These changes still
need a party playthrough to establish smooth exploration, mining and combat.
The workshop marker accepts melee preparation, neutral/dead creatures no longer
block camp respawn, and autosave change detection includes non-voxel gameplay.
Fresh packages include a short player guide, low-spec launchers and an exclusive
host-world lock. No automated tests or hosted CI were run for these changes.

The traversal update adds a craftable chest-slot hang glider, wind-aware flight,
gravity-relative glide and sling momentum, and authoritative jetpack fuel use
with gameplay-image version 6. The player guide lists its recipe and controls.
Cloud adaptation preserves detail when the march is cheap despite other GPU
load. Movement feel, fuel exhaustion/rejoin, cloud appearance and frame-time
improvement need native review for this executable.

The Projects update adds visible inventory navigation, existing quest tracking,
machine routes and placed counts, custom-part and casting links, wand-tier costs
and live drop-source hovers. Journals with no selection receive a destination;
completed tracked quests advance without overriding an explicit Untrack.
Part Builder cost displays and payment share the voxel-volume conversion.
Native layout, complete crafting paths and joined-client guidance need review.

## Limits and historical evidence

This is a bounded workshop expedition and factory slice. Resident trading,
companions, mounts and cross-realm travel are outside this playtest's accepted
scope. Multiplayer does not yet have whole-game completion acceptance.

Source fixes include authoritative loot-cache slots, stale-cache protection,
farming/travel delta sections, cache/block-data/rules in multiplayer save images,
and persistent shared workshop objectives. Existing worlds keep their generation.

An earlier release-fast candidate was built offline on November 5, 2026. That
historical executable's SHA-256 was
`FE6E9E057286E6E0D2ADE714E3907E61146F7867018FD4BB71068A7EC3C5102C`.
A dedicated server and two clients connected through the public relay on one
Windows machine. One client ran headless; the other rendered the remote player
and workshop. The server saved and closed, and the rendered client's fresh quest
journal was created without a save error. The native capture is recorded in the
development checkout at `target/workshop-acceptance/capture.png`.

Startup fixes include the globe shader binding visibility, optional description
copy failing without crashing the game, and creation of quest-save directories.
The stationary 960x540 low-spec capture run on an RTX 4090 averaged 143 FPS;
its largest frame was 392 ms and GPU frame-wait retries were logged. This is
startup/rendering evidence, not smooth combat or normal-resolution acceptance.

Still unverified: a human party completing both objectives, simultaneous shared
cache interactions under latency, earned-progress save/rejoin, separate-machine
WAN play, and a long session. No automated test suite or hosted CI was run.
This candidate does not establish flawless play or superiority to modded Minecraft.

The sandbox denied local sockets (Windows error 10013). A dedicated Steamworks
startup also failed inside that sandbox before anonymous login. Relay startup
and the two-client session succeeded outside the sandbox. Steamworks startup and
completed gameplay acceptance remain unverified.

## Local rebuild

For split-base regression coverage, run `tools/Review-TwoBases.ps1 -Executable
<release executable>` from PowerShell 7 with the local `voxel-lobby.exe` built.
The runner launches a loopback relay, a disposable dedicated host and two
independent native headless clients. It verifies stock at both workshops,
production 384 m away while both players are home, a server-authoritative
terrain blast replicated to both clients, and same-profile rejoin with the
outpost position and changed terrain intact. The run folder keeps the
executable hash and timing logs. `-TraceWorld` adds host stall breakdowns.

This scenario covers placed, locally powered machines while a player remains
online. An empty dedicated host pauses. It does not cover wireless power,
server restart, WAN latency or rendered frame pacing.

The air cache review passed the full headless route. The host's blast window
reported 42.92 ms p95 update time, compared with 134.96 ms on the earlier
save-guarded build. Later windows still reached 174.22 ms p95. These are
separate local runs, not a controlled benchmark or a latency measurement.
The industrial air solver remains a stall source even with local cache
invalidation and less copying. Do not use this result to certify smooth play.

`tools/Build-Multiplayer.ps1 -Transport Relay -Profile release-fast -Offline -TargetDir target/friend-slice-build`
uses a separate build cache so editor checks
and other builds do not hold its output lock. Then run
`tools/Package-Workshop.ps1 -Output target/workshop-playtest-NEW -Archive`. The packaging
script refuses to replace an existing package or its saves.
