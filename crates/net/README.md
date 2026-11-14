# Multiplayer foundation

## Steamworks and Steam Datagram Relay

`multiplayer-sdr` uses the Steamworks SDK's `ISteamNetworkingSockets`, Steam
rendezvous, authenticated Steam transport identities and Valve SDR. It keeps the
same authoritative gameplay, signed player profiles, commands and bounded
replication as the standalone path. Voice chat is excluded.

```powershell
.\tools\Build-Multiplayer.ps1 -Transport Steamworks -Offline
.\tools\Play-Multiplayer.ps1 -Role Host -AppId 480
.\tools\Play-Multiplayer.ps1 -Role Join -Address steam:<host-SteamID64> -AppId 480
```

App ID 480 (Spacewar) is the development default, not a shipping App ID. Normal
hosting requires the Steam client to be running and signed in. `-Role Server`
instead uses an anonymous Steam game-server login. The Linux package uses
`tools/run-steam-server.sh`, a private runtime home, and Valve's 64-bit server
library; it does not run SteamCMD or a graphical Steam client in the background.
Cold anonymous login has a bounded 90-second deadline.

Player-hosted Steam sessions require no game traffic through the owned lobby
VPS. Normal routing allows Valve to choose direct connectivity or SDR. For
acceptance, `-ForceSdr` disables ICE so the test cannot pass via direct UDP.
`VOXEL_STEAM_FORCE_SDR=1` provides the same option to the executable.

The SDK is initialized once per process and released after sockets on exit.
Listener callbacks are drained without blocking. Bulk traffic stops at half
the native queue; realtime frames and bounded chat/command queues retain room.
Empty hosts pause simulation. These limits cap work and memory rather than
raising the server's bandwidth or CPU usage without bounds.

Steamworks builds use a deterministic source-and-content compatibility ID,
independent of executable format, so matching Windows and Linux builds can
join. This is not an authentication credential. Player authentication continues
to use fresh signed challenges.

Live acceptance: `target/sdr-nonblocking-gameplay` passed real forced-SDR host
plus two clients, PvP death/respawn, public/private chat, permissions and combat
audio. `target/sdr-pvp-disabled-gameplay` passed `pvp=false` through SDR.
`target/steamworks-core-tests.log` contains 74 passing networking tests.
`target/sdr-final-gameplay` repeats PvP/respawn on the final Windows candidate.
`target/sdr-cross-platform` passed a native Linux host in WSL and a Windows
client through SDR, including authoritative mining, placement and inventory.
The native Linux package also passed listener startup, idle pumping and clean
shutdown (`target/sdr-linux-local-host.log`). The Windows and Linux builds have
compatibility ID `16f5f7f50d9df4b060a6ed8705284fb9f7d71b3a32161d68bbc3a46f5533ad88`.
`target/sdr-rendered/captures/bench-001.png` was inspected and shows the remote
player body/nameplate in a synchronized rendered SDR session.
Cross-machine WAN acceptance passed on 2026-10-28 after the user approved upload
to `ubuntu@172.81.178.223:~/voxel-sdr-review-20261028/`. The Linux VPS host and
Windows clients used Steam identities with forced SDR at both ends, without
port forwarding or firewall changes. `target/wan-sdr-pvp` covers combat,
death/respawn, chat, commands and game audio; `target/wan-sdr-building` covers
mining, placement, inventory and a rendered third client. The inspected capture
is `target/wan-sdr-building/captures/bench-001.png`.
See `docs/reviews/2026-10-28-steamworks-wan.md` for measurements and limits.
The transfer manifest is `target/sdr-wan-payload.json`; the package excludes
source, saves, player keys, credentials and SDK cache/logs. Both temporary hosts
exited successfully and were confirmed stopped; the existing lobby remains active.

## Native gameplay and operating cost

Build the actual native game, including its static Valve library:

```powershell
.\tools\Build-Multiplayer.ps1 -Transport Standalone -PrebuiltRoot <trusted-gns-0.3.0-build-output> -Offline
.\tools\Play-Multiplayer.ps1 -Transport Standalone -Role Server -Address 0.0.0.0:27020
.\tools\Play-Multiplayer.ps1 -Transport Standalone -Role Join -Address <host-IP>:27020
```

The prebuilt path is optional; without it the pinned upstream C++/vcpkg build
runs. It is implemented by the local sys-crate patch, not by a documentation-only
build mode. The reusable game is `target/multiplayer-steam.exe`.

Native protocol 6 queries seed, generator, world kind, voxel scale and build before
joining. Direct addresses and 12-character invites are accepted in the ordinary
join field; the query runs on a worker and the native gameplay connection opens
on the game thread. No manual seed/generator matching is needed for a direct join.

The eight-character invite code also opens a direct UDP connection. A rebuilt
`voxel-lobby` publishes the host's observed IP address and configured UDP port,
which defaults to 27020 and can be set with `--multiplayer-udp-port=`. The host
must be reachable on that UDP port. The lobby only handles discovery and offers,
so active sessions continue when it goes offline.

Host authority covers melee/projectile/beam/blast player hits, armour, wards,
health, death drops and respawn. Knockback is a host correction with velocity.
Death-screen state is replicated with the host's drop-settlement flag, so the
client can request respawn without creating a second set of drops.
`pvp=false` and `max-players` in `server.properties` are enforced. Player identities,
operators, whitelist and bans use signed keys on both native and relay paths.
Public chat, whispers and commands share the same policy. Names precede chat,
including immediately after joining.

Combat audio is sent as bounded nearby events, once, rather than as replayed
sound history in gameplay images. Remote footsteps use the existing material
audio bank. Events stay local within 64 m (footsteps 32 m); player poses and
foreign spell effects use a 96 m interest radius. Empty dedicated hosts pause
world simulation while still polling connections. Bounded native control queues
keep chat and command replies ahead of bulk terrain during backpressure. Saves
build their snapshot directly instead of compressing and decoding it again.
Voice chat is intentionally
excluded. Direct native gameplay consumes no relay bandwidth and requires a
reachable UDP listener; Steamworks invitations, SDR and NAT traversal are not
provided by the standalone direct-IP library configuration.

`tools/Review-NativeMultiplayer.ps1` runs isolated native processes and writes
host/client logs and CPU measurements. `-NoPvp` exercises the disabled rule.
For native loss/latency runs, set `VOXEL_NET_LAG_MS` and
`VOXEL_NET_LOSS_PCT` in that review process. These opt-in settings use Valve's
packet simulation and have no default-game overhead beyond connection setup.

### Native acceptance, 2026-10-28

Candidate SHA-256: `F9EED3E5F9A54C33CECBBA2034552FE1B698CF0A2B57CF306EBFE3EDB3A3C6E5`.
The final candidate is `target/multiplayer-steam.exe`; start it with
`tools/Play-Multiplayer.ps1`.

- `target/native-final-pvp`: real native host plus two processes passed
  automatic world discovery, public/private chat, operator enforcement, damage,
  death, restored positive health after respawn, and combat-audio delivery to an
  initialized output device.
- `target/native-final-loss`: the same gameplay path passed with `pvp=false`,
  50 ms simulated send latency and 1 percent packet loss.
- `target/native-final-rendered`: a rendered client synchronized with the
  host and another peer. Inspected `captures/bench-001.png` shows the remote
  player body and nameplate in the loaded world.
- `target/native-final-rejoin`: an isolated copy of the edited saved world
  restored the signed player's inventory and operator rights. Creative and
  survival command replies reached the client during the terrain stream.
- `target/steam-final-core-tests.log`: 74 native networking library tests passed.

These are loopback acceptance runs. They do not establish cross-machine WAN
behavior, human sound-quality acceptance, or comparative Minecraft performance.
The final idle-host samples recorded 0–0.047 CPU seconds over three wall seconds;
they are small local samples, not hosting capacity measurements.

The earlier slice notes below are historical; they predate the survival, identity,
PvP, audio and current native build paths.

## Current compatibility update (2026-10-28)

The deployed public lobby speaks directory protocol 2. Clients prefer protocol 3 and
retry protocol 2 only after an explicit `unsupported lobby protocol` refusal. The
selected codec is retained for the connection, including hosting, listings and relay
threads. Protocol 2 omits MOTDs; gameplay packets and executable compatibility checks
are unchanged. Merely lowering the Hello version would send incompatible host/listing
fields and is insufficient.

Direct GameNetworkingSockets game sessions now use the same signed player keys,
access lists, operator checks, roster, chat, command and profile identity paths as
relay sessions. Admission of a keyed player has an absolute five-second deadline.
Native sends include the next packet in queue-limit accounting and disable the
Nagle batching delay. Authoritative frames are encoded once and share an owned
buffer across native sends. Reliable ordering remains required by frame and delta decoding.

The historical slice description below predates survival and profile replication;
it is not a current completeness or performance acceptance claim. Steamworks account
authentication, Steam invitations/SDR, WAN loss recovery and superiority over
Minecraft are not established by these source changes or type checks.

`engine_net` provides host-ordered action exchange, isolated from rendering and saves.
The game's optional `multiplayer` feature now uses this transport for a shared building
session: host-authoritative collision/movement, remote player models, initial terrain
transfer, acknowledged synchronization, and reliable block edit replication.
Its optional `steam` feature uses the open source Valve GameNetworkingSockets library
through `game-networking-sockets` **0.3.0**, pinned in Cargo.lock. No Steam account,
App ID or proprietary Steamworks SDK is needed for this direct-IP transport.

Sources and licenses:

- [Valve GameNetworkingSockets](https://github.com/ValveSoftware/GameNetworkingSockets), BSD-3-Clause.
- [Rust binding](https://github.com/hussein-aitlahcen/gns-rs), MIT / Apache-2.0.
- [Native Windows build requirements](https://github.com/ValveSoftware/GameNetworkingSockets/blob/master/BUILDING.md).

## Current boundary

Implemented: strict versioned binary packets, matching seed/generator/cell-size/content
identity, 16 connection slots including pending joins, host-assigned non-reused IDs,
5-second handshake and 15-second idle deadlines, 120 messages/second/peer limit,
1 KiB messages, 64 KiB native receive/send buffers, bounded polling and input lead,
sequence/replay checks, neutral missing inputs, sorted rosters, disconnect cleanup,
and reliable ordered host frames. Slow peers are disconnected rather than accumulating
an application retry queue. Admission slots include unverified connections.

The digest is a compatibility check, not authentication. Direct-IP encrypted transport
does not establish a trusted player or host identity. This is a private-session foundation;
Steam identity/tickets and a public-server admission policy are separate work. Lobbies and the
relay are in `lobby/` and `crates/lobby`, described below. A public listener also needs connection-flood handling beyond the
application's bounded polling; the binding owns its internal callback queue.

The `voxel-net` binary remains a standalone neutral-input session harness. The game
integration is in `crates/app/src/multiplayer.rs` and uses an authoritative server rather
than relying on cross-machine simulation determinism. Protocol version is now 4,
which narrows the per-tick roster payload and adds the social packets: presence is a
16-bit mask, the frame tick is sent once, and each present input carries a `u32`
sequence, four `i16` axes and a `u8` button byte. A full 16-player frame is 290 bytes
instead of 512, and an absent player costs 4 bytes. `crates/net/src/protocol.rs` pins
that size in a test. Names, chat and commands travel as `Introduce`, `Roster`, `Chat`
and `Command`; policy lives in `crates/net/src/social.rs` and
`crates/net/src/commands.rs`.

## Play a building session

The locally built candidate can be started with:

```powershell
.\tools\Play-Multiplayer.ps1 -Role Host
.\tools\Play-Multiplayer.ps1 -Role Join
# A dedicated host runs without a window; type quit to save and stop.
.\tools\Play-Multiplayer.ps1 -Role Server
# On another machine, add -Address <host-LAN-IP>:27020 and use the same build/seed.
```

Build the actual game with `cargo build -p minecraft_style_voxel_prototype --features multiplayer`.
Run the **same executable** on both peers, with the same `--multiplayer-seed` (default 74):

New worlds use generator 62, including surveyed bridges. Existing host saves retain
their stored generator. When joining an older world, pass `--multiplayer-generator=60`
(or its stored version); the launcher accepts `-Generator 60`. World identity is
checked before admission. Existing terrain is never silently regenerated.

```powershell
# Host a private LAN session; replace this address with the host's LAN IP.
.\target\debug\minecraft_style_voxel_prototype.exe --multiplayer-host=192.168.1.20:27020
# Join from another process or machine:
.\target\debug\minecraft_style_voxel_prototype.exe --multiplayer-join=192.168.1.20:27020
```

The host prints an invite code for its IPv4 address (`XXXX-XXXX-XXXX`) and shows it in the
expedition message. Guests join with `--multiplayer-join-code=<code>` instead of an address. The
code carries only the address and port; it is a typo check, not a credential.
For two processes on this machine use `127.0.0.1:27020`. `--low-spec` reduces the cost of
running two renderers. The host uses the same movement/edit authority path as guests.
WASD, jump, sprint, sneak, mouse look and normal block controls are retained. Reach is
six metres. Supported construction materials are dirt/grass, stone, sand/sandstone,
logs/planks, stone bricks/cobblestone, gravel, clay and basalt. Other selected blocks
are not placed. Host-owned motion is displayed at 30 Hz; remote bodies are interpolated.

This mode hosts a **separate Hills building world at half-metre voxel scale**. The host
loads and saves terrain, its position and the world calendar in the normal save root's
`multiplayer-building/seed-<seed>.ron` file. Changed terrain autosaves every 10 seconds;
Save and normal window/menu exit also save. Files are bounded, checksummed and replaced
after a flushed temporary write. Invalid saves are refused rather than overwritten.
Use `--multiplayer-save=<path>` to select a host save or `--multiplayer-ephemeral` for
a disposable session. Existing single-player sessions and preference files are untouched.
Survival panels show an unavailable message. Per-player inventories, crafting, machines, mobs,
combat, drops, fluids, campaign progression, reconnect recovery, Steam
lobbies/invites/authentication and dedicated servers are **not implemented** here.
This is an initial playable building slice, not full multiplayer survival or mod support.

The initial snapshot has a 16 MiB cap and at most 128 edited chunks. A peer loading a
snapshot has a 65,536-cell coalescing backlog and a 90-second acknowledgement deadline.
Exceeding a budget disconnects that peer; the host session keeps running. The checksum
detects transfer corruption; it is not a security signature. Exact executable matching
is deliberately stricter than a mod/content manifest and will need migration later.

## Online sessions through a lobby

Hosts that cannot accept inbound connections list themselves on a lobby, and clients join by
code or search the list. The lobby moves opaque bytes between the two ends over TCP, so no
port forwarding or native library is needed. This is the default `multiplayer` build.

- `voxel-lobby` (crate `crates/lobby`) keeps the directory, answers browse requests and relays
  frames. It runs on the public lobby `172.81.178.223:27021` (`lobby::DEFAULT_LOBBY`).
- Browse is a single filtered pass with a top-k selection. Search matches every typed word
  against the name or world. Sorts: players, newest, name. Hidden: full sessions (unless asked
  for), unlisted sessions, and other game builds.
- Join refuses a different build (executable digest) before any game bytes move. The joiner
  does not need the host's seed or settings: the lobby returns the seed, world kind, voxel
  scale and generator, and the joiner builds that world before it says hello. Version 0
  in a join means "whatever the host runs". Wire protocol is 2.
- Limits: 512 sessions, 4 per address, 256 connections, 8 browses per connection per second,
  64 KiB frames. A peer that stops reading is disconnected; its frames are never skipped.
- Session codes are eight characters (`XXXX-XXXX`), 40 bits drawn from the operating system's
  CSPRNG. Unlisted sessions stay out of the browser, but anyone who holds the code can join.
  Unknown-code joins are limited to 10 per address per minute, so guessing is impractical.
  Codes are not authentication; a host approval step is the next step if sessions must be private.
- Transport is plain TCP. Packets are not encrypted, and the content digest is a build check,
  not authentication.

Game flags:

```powershell
# Browse from a shell; prints code, players, mode, version and age.
.\minecraft_style_voxel_prototype.exe --multiplayer-browse="river"
# Host (listed, or --multiplayer-unlisted), then join by code.
.\minecraft_style_voxel_prototype.exe --multiplayer-online="Riverside" [--multiplayer-creative]
.\minecraft_style_voxel_prototype.exe --multiplayer-join-code=ABCD-EFGH
# Another lobby: add --multiplayer-lobby=HOST:PORT to any of the above.
```

The title menu's Multiplayer page searches the lobby on a worker thread and opens joins and
hosts in a new game window; the open world is not changed. Direct GameNetworkingSockets
addresses (`--multiplayer-host=`, `--multiplayer-join=`, 12-character invite codes) need the
`multiplayer-steam` feature and the native library.

Deploying the lobby (Ubuntu 24.04): build `engine_lobby` in release mode, install it as
`/usr/local/bin/voxel-lobby`, run it as the system user `voxellobby` under a systemd unit with
`--bind 0.0.0.0:27021`, and allow `27021/tcp` in ufw. The unit caps memory at 128 MiB and CPU
at 40%. The same host runs FastCast services on 80, 443 and UDP 45723; the lobby shares the
box and touches none of them.

## Build on Windows

Use a Visual Studio C++ developer shell, with CMake, Git and LLVM/libclang installed.
Set `LIBCLANG_PATH` to the directory containing `libclang.dll` if it is not discoverable.
The native crate bootstraps vcpkg and builds OpenSSL/Protobuf dependencies; first builds
require network access. Native build products live in the selected target directory.
The workspace forces the native dependency's development CMake configuration to Debug,
working around upstream 0.3.0's Debug/RelWithDebInfo library search mismatch. `engine_net`
also supplies the Windows system libraries required by the static native library.
For an existing separately built native library, `GNS_NATIVE_LIB_DIR` may point to its
directory containing `GameNetworkingSockets_s.lib`.
Use short paths for vcpkg build trees:

```powershell
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
$env:GNS_VCPKG_BUILDTREES_ROOT = "$PWD\target\gns-bt"
cargo build -p engine_net --features steam --bin voxel-net --target-dir target/net-foundation
```

For a private LAN, bind the host to a specific LAN IP and give peers that IP.
Loopback example, in two terminals from the repository root:

```powershell
# Placeholder compatibility digest for the harness only, never a real content identity.
$digest = '00' * 32
.\target\net-foundation\debug\voxel-net.exe host 127.0.0.1:27020 42 58 500 $digest
# Second terminal:
$digest = '00' * 32
.\target\net-foundation\debug\voxel-net.exe join 127.0.0.1:27020 42 58 500 $digest
```

Stop the harness with Ctrl+C. No game save is loaded. This example has not been run.

## Game integration contract

1. Derive `WorldIdentity` from the actual loaded world and a stable digest of all gameplay
   content, generator configuration and simulation rules. Never hardcode a production digest.
2. Poll `SteamHost`/`SteamClient` on one owner thread. Supply monotonic `Instant` values.
   The host must call `advance` exactly once per simulation step, at the agreed 30 Hz.
   It returns a full sorted roster and at most one input/player for that tick.
3. Clients call `submit` once per observed host tick, including neutral inputs when idle.
   The current three-tick lead is fixed; late input is dropped without rewinding the host.
   Buttons 0..3 are reserved intent bits; assign game meanings when the input adapter lands.
4. Validate movement, permissions, resource costs and interactions on the host before
   applying intents. Quantized inputs alone do not make the existing simulation deterministic.
   For actual lockstep, local host input must use the same queued path and simulation rules.
5. The game adapter implements bounded snapshots, acknowledgement, terrain catch-up,
   executable compatibility checks and remote interpolation. State digests, resync and
   survival-system replication remain future work; the separate harness has no world sync.
6. Keep position, inventory, combat and machine authority on the host as coverage grows.

## Validation recorded for this slice

The transport-independent core and the game with the multiplayer feature passed Rust
type checking. The native Valve library and multiplayer game also linked successfully
after installing workspace-local libclang and fixing the upstream development library
path. Due to C: capacity, the native build is under
`D:\CodexBuilds\voxel-v2-multiplayer-20261026\net-foundation\debug` on this machine.
No automated tests or CI were added or run. Two native processes completed handshake,
snapshot acknowledgement and terrain loading (27/27 chunks), with actual renderer captures
in `target/multiplayer-host-view.png` and `target/multiplayer-client-view.png`.
The exercised initial world was unedited. Movement feel, nonempty terrain transfers,
interactive block edits, WAN behavior and save/reload acceptance remain open.

Generator 62 and the revised game linked successfully. A native half-metre seed-74
capture at `target/bridge-final/bench-002.png` shows a generated bridge with railings,
supported deck, bank approaches and open water below. No human traversal was recorded.
The earlier host subsequently hit an egui RwLock timeout; long-session stability is
not accepted merely because the initial join succeeded.

Final native candidate: `target/multiplayer-final-20261026.exe`, SHA-256
`A2D75D37A789F9D9B086BF0E62AC2A3A383C11C8AAC2E51A29109E573FD7F3FE`.
The final dedicated host loaded its disposable save and a native client completed
snapshot acknowledgement and 27/27 terrain loading. Capture:
`target/dedicated-bridge-client.png`; logs: `target/dedicated-bridge-review.log` and
`target/dedicated-bridge-client.log`. These runs required network-enabled execution;
restricted shell networking had caused the intervening join timeouts.
This reload covered pristine terrain; nonempty saves/transfers remain unexercised.
The bounded dedicated run stayed connected until its scheduled shutdown, saved,
and exited successfully. The diagnostic client then reported transport closure.
Terrain surveying now completes before timed connection setup, and new-world route
search has bounded alternatives. Dedicated mode has no renderer or unused host avatar.
An unsafe nested egui context read in recipe shortcuts was removed; the earlier lock
panic's root cause is not conclusively established by this change.
