# Multiplayer: modded-server parity, then past it

Date: 2026-10-26. Status: accepted 2026-10-27 except the VPS UDP port (owner decision 5).

## Intent

The game should play as a shared world the way a modded Minecraft server does:
a dedicated or in-game host, friends joining mid-session, separate inventories,
shared factories, shared enemies, one save. It is also a 3D falling-sand game,
so water, sand, fire and spell blasts must stay shared at a scale Minecraft never
replicates. Parity comes first. After parity, the goal is to beat Minecraft on
three things a player feels: zero-latency local movement and edits, smooth remote
players, and less bandwidth per player for the same world activity.

## What exists (read 2026-10-26)

`crates/net`, `crates/lobby` and `crates/app/src/multiplayer*` are untracked work
from another session. They provide:

- Packet format `VXMP` v2, 30 Hz host frames, 16 player slots, admission limits,
  `WorldIdentity` (seed, generator, cell size, executable digest).
- Two transports behind one `Host`/`Client` enum: a TCP relay through the lobby
  (`172.81.178.223:27021`) and direct Valve GameNetworkingSockets (UDP, encrypted,
  `multiplayer-steam` feature, needs a native build).
- A lobby with browse, session codes, kick, and a dedicated headless host mode.
- Per-player state by swapping `gameplay::Personal` (adventure, inventory,
  expedition, factory focus) into `World` around each actor's step.
- Inventory as semantic commands (`multiplayer/inventory.rs`). This is the right
  shape and stays.
- Block edit deltas, initial terrain transfer of edited chunks only, and host
  saves in `multiplayer-building/seed-<seed>.ron`.

## Gaps found

1. **Full-state images.** `gameplay::capture` serialises the whole factory, every
   water cell, adventure, chips, orbital, habitats and environment as RON, zlib
   compressed, up to 16 MiB, and sends one to every client every 100 ms. Cost
   grows with world activity, not with what the player can see.
2. **No local prediction.** Guest movement runs only on the host. Every step and
   every block edit waits one round trip. This is the biggest feel defect, and it
   is larger than TCP head-of-line stalls.
3. **The host is a special case.** The host's player is stepped directly and
   guests are stepped through `Personal::swap`. Two code paths diverge over time.
4. **One simulation anchor.** `simulation::fixed::finish` receives one player as
   the anchor. Water, sand and activity wake around the host or the first guest
   only. A guest far away walks into a frozen world.
5. **No chunk tickets.** Nothing keeps a factory running when no player is near,
   and nothing bounds which chunks a server keeps loaded.
6. **Transport.** The relay is TCP only, so lost packets stall all traffic behind
   them. The UDP path needs a direct address and port forwarding.
7. **Identity.** The executable digest is stricter than content compatibility
   (rebuilds refuse each other) and weaker than identity (anyone can claim any
   profile token). Relay traffic is plaintext.
8. **Missing parity features.** Shared machine screens with live state, chat,
   commands, permissions, whitelist, server config, death and respawn rules,
   sleep voting, and opening an ordinary single-player world to others.
9. **No measurement.** No network simulator, no bandwidth counters, no automated
   multiplayer test. README records no tests run.

## Goals

- Dedicated server and in-game host run the same server code. The in-game host
  talks to its own server through a loopback channel, as Minecraft's integrated
  server does.
- Guests join mid-session and are playable within 2 s of the nearest 3x3 chunk
  columns arriving.
- Everything gameplay can change is server-authoritative: terrain, cellular cells,
  factory, chips, inventory, enemies, spells, drops, calendar. Two things are
  owned by the client and validated by the server: its own body pose (section 4)
  and which enemy its shot hit (section 5).
- Local movement, block edits, casts and inventory clicks respond on the frame
  they happen (predicted), with server correction.
- Remote players and enemies move smoothly at 30 Hz snapshots under 0-150 ms RTT,
  20 ms jitter and 2% loss.
- Bandwidth per player (budgets, measured by the bench in M0):
  idle 2 KB/s, walking 8 KB/s, combat 24 KB/s, chunk streaming bursts capped at a
  server setting (default 1 MB/s).
- Server tick at 16 players with 64 active machines stays under 4 ms average at
  120 Hz on the reference desktop.

## Non-goals

- More than 16 players, or sharded servers.
- Clients with content different from the server (modded registry sync later).
- Anti-cheat beyond server authority and input validation.
- Peer-to-peer meshes. Every session has one server.
- Cross-platform byte determinism of the legacy simulation.

## Anti-goals (a failure even if it runs)

- A client is ever authoritative for inventory, placement cost or damage.
- Client and server worlds drift apart without the client detecting it.
- Bandwidth scales with total world activity instead of the player's interest set.
- Multiplayer work changes single-player results, saves or generator output
  without a version gate.

## Architecture

### 1. Server and client split

`Server` owns the authoritative `World`, the player table and the simulation.
`Client` owns a replica `World` for rendering, collision and prediction. The
in-game host runs both in one process with a `Loopback` transport. Dedicated mode
runs `Server` only (existing headless path). Every player, including the host,
goes through the same input queue. This removes gap 3.

Per-player state moves out of `World` singletons into `world.players:
BTreeMap<PlayerId, PlayerState>` holding body, inventory, adventure-personal,
expedition, focus and spawn point. Migration is incremental: keep `Personal::swap`
as the only access path until each system reads `PlayerState` directly, then
delete the swap. `BTreeMap` keeps iteration order stable.

### 2. Clocks and ticks

- Server simulation stays at 120 Hz (`STEP_SECONDS`). Factory timers may run at
  a divisor, as now.
- Snapshots go out at 30 Hz (server setting, 20-60).
- Clients send their pose and action inputs (cast, place, mine, interact, slot)
  every 120 Hz step, batched into one packet per 30 Hz window. Each packet repeats
  the last 8 unacknowledged steps, so a lost packet costs nothing.
- Clients estimate server tick from ping samples and keep an adaptive input lead
  (2-6 ticks) so inputs arrive just before the server needs them. Late inputs are
  reused from the last known input, never rewound.

### 3. Transport and channels

A `Transport` trait over the existing relay, GNS and the new loopback, with four
channels:

| Channel | Delivery | Carries |
|---|---|---|
| control | reliable ordered | join, chat, commands, container open/close |
| world | reliable ordered | block and cell deltas, machine events, spawn/despawn |
| state | unreliable, newest wins | player and entity snapshots, acks |
| bulk | reliable, fragmented, lowest priority | chunks, join image, chip designs |

A per-client send scheduler fills a byte budget each net tick in channel priority
order (control, state, world, bulk), so a chunk burst never delays snapshots at
the application layer. Over TCP the channels share one stream. Over UDP they map
to GNS lanes.

UDP for relayed sessions (gap 6): the lobby gains a UDP relay socket that forwards
datagrams tagged with a session token, and clients try a GNS direct connection
first (with lobby-assisted hole punching) before falling back to the UDP relay,
then the TCP relay. This waits for M0 measurements and owner approval of a new
port on the shared VPS.

### 4. Player movement

- **Local player: owner movement, server validation.** The client runs
  `player::update_player` on its replica world and sends its resulting pose and
  movement state each step, batched per net tick. The server does not re-simulate
  the body. It checks each pose against the previous accepted one: displacement
  within the movement state's speed bound plus slack, no body overlap with solid
  published cells, flight only with permission, and no fall-damage skips. A
  failed check returns a correction (pose plus correction id); the client snaps
  to it and ignores its own poses until it echoes that id. Minecraft and Mycopunk
  both work this way. It gives zero-latency movement with no rubber-banding when a
  replica's collision lags an edit, which input replay cannot avoid in an
  editable voxel world. Prediction with replay (sending inputs, server
  re-simulates) remains the fallback if validation proves too loose for PvP
  servers, and is a server setting, not the default.
- **Server-moved players** (knockback, teleport, mounts, conveyors) go through the
  same correction message, so the client never fights the server over the body.
- **Remote players and enemies:** an interpolation buffer 100 ms behind the newest
  snapshot, widened adaptively by measured jitter (up to 200 ms). On starvation,
  extrapolate one snapshot interval, then hold.
- **Quantisation:** position relative to the containing chunk at 1/256 voxel in
  `i16x3` plus chunk delta, yaw and pitch in `u16`, velocity in `i16` at 1/128
  voxel per tick.

### 5. Entity replication

Enemies, projectiles, drops, rigid bodies and remote players are entities with a
network id. Per client the server keeps:

- an **interest set** by distance per entity class (players and bosses 256 m,
  enemies 96 m, drops 48 m, small projectiles 64 m);
- a **priority accumulator** so near and fast entities update every snapshot and
  far ones update less often within the byte budget;
- a **baseline** per entity: the last state the client acknowledged. Snapshots
  send only changed fields against it, with a field bitmask.
- **dead reckoning** between updates: each update carries velocity, and clients
  advance low-priority entities by it until the next one arrives, so a far entity
  updated every fourth snapshot still moves smoothly.

Only the root pose and a few state bytes are replicated. Leg IK, bob, sway,
viewmodel motion and particles are derived on each client from that state.

Spell casts: the caster's client spawns the projectile immediately with a local
id. The server spawns the authoritative one from the cast input and sends the
mapping, and the client swaps its local projectile for the server's. Other clients
receive spawn events (spell id, origin, direction, server tick, seed) and simulate
the flight locally for visuals.

Hits on enemies are **claimed by the shooter's client**: it reports (entity id,
spell or weapon, hit point, the tick it was viewing). The client shows the hit and
health change at once. The server accepts the claim after plausibility checks
(target alive and within interest, distance within the weapon's range plus the
target's movement since that tick, fire-rate and mana spent, line of sight
against published cells) and computes damage itself from the spell table. The
client never sends an amount. A refused claim restores the target's health on
that client. This is what co-op shooters do for PvE, and it needs no rewind.
Enemy hits on players, explosions and terrain damage are decided on the server.

Lag compensation (rewinding targets to the attacker's view tick, bounded to
200 ms) is built only for PvP, which is off by default.

### 6. Terrain and chunks

- **Chunk tickets** (gap 5): player tickets (view distance), simulation tickets
  (simulation distance, smaller), loader tickets from a chunk loader machine,
  and short-lived tickets for explosions and machine output. Only ticketed chunks
  tick. Simulation takes the union of all player anchors (gap 4).
- **Streaming:** each client receives chunks nearest first, weighted toward its
  view direction. Each chunk carries a version counter.
- **Pristine chunks are not sent.** The client generates them from seed and
  generator version, as today. The server sends the hash of the expected pristine
  chunk. If the client's generated hash differs, the server sends that chunk in
  full. This makes worldgen float drift between CPUs self-correcting instead of a
  desync.
- **Edited chunks** go as palette plus run-length cells, compressed.
- **Client chunk cache** on disk keyed by (world id, chunk, version). On rejoin
  the client lists versions it holds and the server sends only newer chunks.
- **Block edits:** reliable deltas batched per net tick, each tagged with the chunk
  version it produces.
- **Predicted edits:** the client applies a place or break immediately and sends a
  conditional command with the observed cell (ADR 0026 semantics: `Applied`,
  `NoChange`, `Conflict`, `Protected`). It shows the edit as pending until the
  outcome arrives, and rolls back on refusal. Inventory cost is charged only by
  the server.

### 7. Cellular simulation (the falling-sand core)

Two tiers. Tier A ships first. Tier B is where this engine beats any block game
on bandwidth, and it waits on the deterministic simulation becoming the window's
authority (ADRs 0009, 0018, 0034).

- **Tier A, server deltas (legacy simulation).** The server runs water, sand, fire
  and thermal. Per net tick, changed cells in a client's interest set go out on
  the world channel, nearest first, within a per-client cell budget. When a region
  exceeds the budget, it is marked stale and resent as whole bricks once activity
  drops. Sleeping bricks cost nothing. This replaces the water list in the
  gameplay image.
- **Tier B, deterministic mirrors (`engine_sim`).** The client runs `engine_sim` on
  the bricks in its interest set, with the same seed and tick numbers. The server
  sends only what enters those bricks: edit commands, sources, entity
  interactions, and the boundary cells flowing in from bricks the client does not
  simulate. Every 30 CA ticks the server sends an 8-byte hash per active brick
  group. On mismatch it sends that brick's state at a known tick and the client
  fast-forwards. A dam break then costs a few edit commands instead of every moved
  cell. Requirements: count-based budgets, not wall-clock, in mirrored regions;
  the pinned hash and phase order of ADR 0008; and CPU/GPU equivalence for clients
  that run the GPU kernel.

### 8. Factory, machines and chips

- Factory, power, item networks and chip firmware run only on the server.
- Machine appearance (running, progress as `u8`, tank levels as `u8`, facing) is
  an entity-like component in the interest set.
- **Container sessions:** opening a machine or chest subscribes the client to that
  machine's full state at 10 Hz. Slot actions use the existing semantic
  `inventory::Command` pattern with a revision number. The client predicts the
  click, and the server's revision either confirms it or replaces the client's
  view. Several players may view one machine at once.
- **Items on belts and pipes** travel as (route, start tick, speed) and animate on
  the client. Per-item positions are never sent.
- Chip designs upload as a bulk blob with the existing design checks run on the
  server before installing.

### 9. Saves and per-player data

- Multiplayer uses the ordinary save format with a `players/` directory holding
  `SavedPlayer` per profile id. Shared state (world, factory, enemies, encounter
  ledger, calendar) stays in the world save. This needs a `SAVE_VERSION` bump.
  The bump happens in M5, after a grep for peers' pending bumps.
- Any single-player world can be opened to others from the pause menu, the same
  feature as Minecraft's Open to LAN. The host's own player becomes the first
  `players/` entry.
- The separate `multiplayer-building` save format is retired with a one-way import.
- Bed spawn points are per player (bed rules from 7d07c6c3).

### 10. Admission and identity

- **Content manifest** replaces the executable digest: a hash per `assets/*.ron`
  file plus generator version, protocol version and a simulation rules
  fingerprint. A mismatch names the differing files. Rebuilds with the same
  content can play together.
- **Player identity:** each profile gets a keypair. Join signs a server challenge,
  and whitelist, bans and operators are keyed by public key. This adds a
  signature dependency, which needs an ADR line.
- Relay traffic is encrypted once identity keys exist (key exchange derived from
  the same keys), or by moving relayed sessions onto GNS over the UDP relay.

### 11. Server operation

- `server.ron`: name, max players, view distance, simulation distance, whitelist
  on or off, PvP, difficulty, sleep rule, snapshot rate, chunk burst cap.
- Chat with rate and length limits. Commands: kick, ban, op, deop, whitelist, tp,
  time, gamemode, save, stop. Permission levels 0-3.
- Dedicated console accepts the same commands.
- Player-visible text follows `egui_ui/UI_RULES.md`.

### 12. Measurement

- `NetSim` transport wrapper with seeded latency, jitter, loss, duplication,
  reordering and a bandwidth cap.
- An in-process scenario runner: one server plus N clients over `NetSim`, driven
  by `assets/scenarios/mp_*.ron` input scripts. Checks: replica terrain equals
  server terrain at the end; predicted pose error stays under a bound; no client
  stalls longer than one snapshot interval at 2% loss.
- `just bench mp_bandwidth`: bytes per second per player for idle, walking,
  building, combat and dam-break scripts, gated against the budgets above.
- A desync report: on any hash mismatch, log the first differing chunk or brick
  and tick.

## Milestones

Each milestone is playable and tested on its own. Order matters: measurement
first, then the split that every later step builds on.

| # | Scope | Done when |
|---|---|---|
| M0 | `Transport` trait, loopback, `NetSim`, byte counters, in-process scenario runner, bandwidth bench | the current build's numbers are recorded as the baseline |
| M1 | server/client split, in-game host via loopback, `world.players`, multi-anchor simulation, chunk tickets, CI build without the renderer feature | host and guest share one code path; a far guest sees live water |
| M2 | channels and scheduler, entity interest, priority, delta baselines, quantisation, interpolation buffer; retire the 10 Hz gameplay image except at join | walking budget met at 2% loss, 100 ms RTT |
| M3 | owner movement with server validation and corrections, predicted edits, predicted inventory clicks | zero-frame local response; a scripted speed-hack and wall-clip are corrected in the netsim scenario; no correction fires on an honest route at 2% loss |
| M4 | chunk streaming order, pristine hashes, client chunk cache, Tier A cell deltas | rejoin sends only changed chunks; dam-break bench within budget |
| M5 | container sessions, machine components, item routes, enemies, spells and client-claimed hits with server checks, grave markers and respawn, claims, sleep vote, per-player save data, save bump, open to LAN | a survival session with shared factory and combat plays end to end |
| M6 | content manifest, identity keys, chat, commands, permissions, `server.ron` | whitelist and ops work by key; rebuilt exes with equal content connect |
| M7 | UDP relay (needs decision 5), GNS hole punching, transport fallback order; Steam Networking Sockets before a Steam launch | WAN session at 2% loss shows no stalls in the bench |
| M8 | Tier B deterministic cellular mirrors | dam-break bandwidth drops by at least 10x against Tier A with zero unresolved mismatches |

## Reference studies

- `docs/research/mycopunk-netcode.md`: what a shipped co-op voxel-adjacent shooter
  does for movement, hits, entity sync and joins, and what to avoid from it.

## Left to the builder

- Wire encodings, field bitmask layout, compression codec choice within existing
  dependencies.
- Interest radii and priority weights after M0 numbers exist.
- How `PlayerState` migration is sliced across systems.
- Scenario file schema for `mp_*.ron`.

## Builder calls (made by the design author, not the owner)

- **When the server exists.** Single player stays as it is: no server, no replica.
  Hosting or opening a world to others moves the live `World` into a `Server` in
  the same process. The host's client renders the authoritative `World` directly
  and submits its inputs through the same queue as guests, with prediction
  disabled because its latency is zero. Guests keep replicas. This keeps the
  single-player frame cost and results unchanged (anti-goal), and still gives one
  gameplay path for every player.
- **Chunk keys.** Tickets, streaming, caches and pristine hashes take an opaque
  `ChunkKey` with the flat-grid implementation today, so cube-sphere faces
  (decision 0024) add a key type instead of rebuilding M1-M4.
- **Cellular authority order.** Tier A is built on the legacy simulation regardless
  of the `engine_sim` migration date, because it is the only live path and its
  interest and budget machinery carries over to Tier B. M8 depends on the ADR
  0018/0034 workstream making `engine_sim` the window authority. Lifting ADR
  0034's "no multiplayer" restriction is M8's job.
- **Wire compatibility.** There are no released builds, so every milestone may
  bump the protocol version and lobby wire. Lobby redeploys are batched (M6 and
  M7 at most) because each needs the owner's VPS login.
- **Untracked net code.** This plan adopts `crates/net`, `crates/lobby` and
  `multiplayer*` only after their author commits them, then builds in a worktree.

## Owner decisions (answered 2026-10-27)

1. **Progression.** Discovery is per player: tech unlocks, the found-spell log
   and the chip library go in `players/`. One-time loot is instanced per player,
   as the world plan specifies. Habitat and orbital bookings are shared world
   state. Claims are built in, with per-claim permissions, and are the source of
   ADR 0026's `Protected` outcome. A "team progress" world option (shared unlocks)
   is not built now, but discovery data is keyed so that option is a world flag
   that selects a shared key instead of a per-player one, not a refactor.
2. **Dedicated server.** Headless mode of the Windows exe is enough for early
   access. A renderer-free Linux build is required by 1.0. It comes from a cargo
   feature on the same binary (`--no-default-features` drops wgpu, egui, winit and
   audio), not a crate split. The M1 server path must not reach the renderer, and
   a CI build without the feature enforces that from M1 on.
3. **Steam.** Transport stays behind the `Transport` trait. Own identity keys and
   own UDP relay come first, because CI needs a path with no Steam client. Steam
   Networking Sockets is added as a second transport before any Steam launch.
   Own keys are the canonical account; a Steam ID is a linked login, so itch and
   direct builds are not second-class.
4. **Death, PvP and sleep.** On death the inventory stays in a grave marker at the
   death position: no scatter, no experience loss. Keep-inventory and permadeath
   are world options. PvP is off by default and a server setting; claims override
   it inside claimed chunks. Nothing bad happens for not sleeping. A night skip
   needs a majority of online players through a vote prompt; one player in bed is
   a request, not a lock.
5. **VPS UDP port.** Open. If granted, the game relay gets its own system user,
   process and port range, with firewall rate limits from the first deploy.
6. **Roadmap.** Steps M0-M4 are NET-FOUNDATION-01 and start once Milestone 0 places
   the multiplayer crates and decides authority. M0 and M1 are tests and
   infrastructure, which the intake freeze does not block; M2-M4 are the feature
   half. M5-M7 are the Phase 3 operations layer. M8 waits on SIM-01's deterministic
   migration. Multiplayer stays out of release claims until M1-M4 have tests on
   `main`.

The four builder calls above were accepted, with one condition: the 10 Hz
full-state broadcast is the first thing replaced (M2), and chunk work in M4 must
not build on it.
