# Multiplayer social layer: names, chat and server commands

Scope: player names, chat, operator commands and how joiners move, from the wire to
the HUD. Everything below is wired into the game and was run end to end through a
local lobby (see Verification).

## What landed in `crates/net`

| File | Contents |
|---|---|
| `crates/net/src/social.rs` | Name and chat policy: visible-character alphabet, 16 character cap, counted duplicate suffixes, control-character stripping, 240 byte chat cap, sliding chat budget of 5 lines per 5 seconds |
| `crates/net/src/commands.rs` | Command parsing, operator gate and user-facing wording for `/help`, `/list`, `/msg`, `/me`, `/name`, `/seed`, `/time`, `/gamemode`, `/kick`, `/tp`, `/op`, `/deop` |
| `crates/net/src/protocol.rs` | Protocol 4: `Introduce`, `Roster`, `Chat` and `Command` packets with `u16` length-prefixed UTF-8 |
| `crates/net/src/session.rs` | The host accepts `Introduce`, assigns a unique name, answers with `Roster`, and collects `Command` lines for the application |

Two design choices worth keeping:

- Commands return an `Effect` instead of touching the world, so the command layer stays
  independent of the renderer, the save format and the simulation. The application
  supplies a `ServerView` and applies each effect where it already owns that state.
- `Roster` is sent when names change, not every tick. A full 16 player roster is
  2 + 16 * (4 + up to 34) bytes and stays inside `MAX_PACKET`.

## Names

A name is shown to other players and typed back into commands, so it is limited to
letters, digits, underscore, hyphen and full stop, with single spaces only in the
middle. Duplicates gain a counted suffix, `Ana`, `Ana (2)`, `Ana (3)`, which keeps the
same person recognisable after a reconnect. Unnamed players appear as `player<id>` so
the roster is never empty or ambiguous.

## Chat

`Chat` carries an explicit `ChatKind`: say, action, whisper or system. Keeping the kind
on the wire means a client cannot present a server notice as player chat, and the
whisper case keeps its two-sided wording, `to Bo: ...` for the sender and
`from Ana to Bo: ...` for everyone else.

Ordering is the transport's job: the relay is reliable and ordered, so chat and roster
updates cannot arrive out of sequence on the current transport.

## Commands

Channel rules follow what players expect from a hosted world:

- Everyone: `/list`, `/msg`, `/me`, `/name`, `/seed`, `/time` with no argument.
- Operators only: `/gamemode`, `/kick`, `/tp`, `/op`, `/deop`, `/time set`, `/time add`.

`/list` reports the count against the 16 slot limit and each player's gamemode, which
is the one line that answers "who is here and what are they doing".

## Application wiring

| Piece | Where |
|---|---|
| Chat history, player list, typed lines | `crates/app/src/multiplayer/social.rs` (`Social`, `Row`) |
| Host chat and command loop, every effect | `crates/app/src/multiplayer/host_commands.rs` |
| Chat overlay, player list, name tags | `crates/app/src/egui_ui/chat.rs` |
| Name and key files | `multiplayer/gameplay.rs`: `player_name`, `player_key` |
| Joiner movement check | `multiplayer.rs`: `owner_move`, `correct`; tests in `multiplayer/owner_tests.rs` |

Session messages added on the data channel:

| Tag | Direction | Body |
|---|---|---|
| 16 | host to joiner | player list, RON `Vec<(id, name, flags)>`, bit 0 operator, bit 1 creative, once a second |
| 17 | host to joiner | position correction, RON `(id: u16, position: [f32; 3])` |
| 18 | joiner to host | one typed line, raw UTF-8, at most 240 bytes |

Typed lines travel as data rather than `Packet::Command` so both transports carry
them the same way. The host treats a line without a leading `/` as chat, and every
line, chat or command, spends from the speaker's `ChatBudget` (5 lines per 5 s).

Controls (tag 3) now carry the joiner's own feet position and velocity, the newest
correction it applied, and the fall or hazard damage its own step produced.

### In game

- Enter opens chat, `/` opens it with a command started, Up and Down recall sent
  lines, Enter sends, Escape closes. T stays the spell key.
- Closed chat shows the last 10 lines for 10 s each; open chat shows 20 lines and the
  player list with mode and operator markers.
- Each remote player has a name label above the head, faded out past 64 m.
- The multiplayer page has a player name field (saved to `multiplayer-name`) and a
  creative switch for hosting; listings show the MOTD.

### Joiner movement

A joiner steps its own body at the fixed 1/120 s step with the same `update_player`
the host uses, so its own movement has no network delay. The host does not re-run
that movement. It checks each reported position against the last accepted one:

- horizontal travel within 1.25 times the momentum cap plus one dash, per elapsed time,
- climb within 20 m/s plus 2 m, unless the player is in creative,
- the body swept in steps under half a voxel never overlaps solid cells, with a 0.2 m skin.

A refused report gets a numbered correction (tag 17); the host ignores that player's
reports until one echoes the number. Teleports and respawns use the same correction.
The check is deliberately lenient: it stops teleporting, flying in survival and
walking through walls, not small speed gains. Knockback the host applies to a joiner
is overwritten by the joiner's next report; damage still applies.

The host keeps the authoritative copy of everything else: inventory, edits, spells,
machines and health. The gameplay image no longer overwrites the joiner's movement
state (`movement`, `on_ground`, `swimming`, `underwater`).

## Server administration

`crates/net/src/admin.rs` adds the two files a Minecraft operator expects, in the same
`key=value` shape so they can be hand edited and version controlled:

- `ServerSettings`: `motd`, `max-players`, `gamemode`, `difficulty`, `pvp`,
  `whitelist`, `spawn-protection`, `view-distance`. A bad line is reported and the
  previous value stands, so one typo cannot lock everyone out. Values are clamped to
  what the protocol can actually carry, for example `max-players` is 1 to 16.
- `AccessList`: operators, whitelist and bans keyed by lower-case name, written back
  in sorted order. Ban reasons survive a save and load.

`HostSession` now takes an optional `AccessList` plus the whitelist switch, and refuses
a banned or non-whitelisted name at the point where the name is introduced. A session
without an access list admits every name, which keeps private play unchanged.

Bans win over the whitelist, and both are checked before a name is assigned, so a
refused player never appears in the roster.

## Admin commands

`crates/net/src/commands.rs` now covers the whole admin surface, operators only:

`/whitelist <add|remove|on|off|list>`, `/ban <player> [reason]`, `/pardon <player>`,
`/banlist`, `/save-all`, `/stop`, on top of the earlier `/gamemode`, `/kick`, `/tp`,
`/op` and `/deop`. `/whitelist list` and `/banlist` read the live lists through two
new `ServerView` defaults, so a private session with no access list still works.

Effects stay declarative: `Whitelist`, `SetWhitelist`, `Ban`, `Pardon`, `Save` and
`Stop` are applied by the host where it already owns those files.

## Server browser line

The listing now carries a MOTD. `HostInfo` and `Entry` in
`crates/net/src/lobby/wire.rs` gained a `motd` field, encoded with the same bounded
text helper, so a browse reply shows a server message next to the name, world, mode,
player count and age. The lobby wire protocol moved to 3.

## Persistence and the join gate

The server files are now real on disk. `crates/app/src/multiplayer/server_files.rs` reads
and writes `server.properties` and `server.access` in the world's save directory:

- a missing file means defaults, so a fresh world starts without ceremony,
- an unreadable or partly invalid file is reported and the valid parts are kept,
- both files are written back in sorted, hand-editable form.

The host installs the lists through `RelayHost::set_access` immediately after the
session is listed and before it admits anyone, so a banned or non-whitelisted name is
refused at the point the name is introduced and never appears in the roster.
`RelayHost` also exposes the live lists, the roster and the queued command lines for
the command layer to use.

## The command loop

`host_commands::run` runs on the host's 30 Hz network tick. Lines come from the
session queue, from joiners' typed lines, from the host's own chat and from a
dedicated host's console. Every effect is applied:

| Effect | Applied as |
|---|---|
| `Ban`, `Pardon`, `Whitelist`, `SetWhitelist`, `SetOp` | access lists changed and saved to `server.access` and `server.properties` on the same tick; a ban or whitelist entry on an online player is also bound to the key that player proved; an online banned player is disconnected |
| `Kick` | peer disconnected; the host itself cannot be kicked |
| `Gamemode` | per player; survival also ends flight; the joiner learns its mode from the player list |
| `Teleport` | body moved beside the target, then a correction to the joiner |
| `SetTime` | day clock set; joiners follow through the existing time sync |
| `Rename` | through `HostSession::rename`, or the host's own name |
| `Save` | the building save is written; a world without a save file says so instead of claiming a save |
| `Stop` | the world is saved, joiners are disconnected and a dedicated host exits with code 0 |

Rights follow keys: the host is always an operator, and a joiner is one only when
`HostSession::holds_op` finds an operator entry bound to its proven key. A dedicated
host's console speaks as "Server" and is not listed as a player. The player
registry (inventories and positions saved per player) is keyed by the proven key's
fingerprint, so a returning player gets the same inventory back.

## Verification (2026-10-27)

Run through a local `voxel-lobby` with a dedicated host and windowless joiners
(`--multiplayer-headless` now also joins; `--multiplayer-bot-walk` makes a joiner
sprint in a circle and hop):

- Chat both ways; operator gate refusal then `/op`; `/gamemode`, `/list`, `/time set`,
  `/me`, `/name`, `/msg` (whisper reaches only its target), `/save-all`, `/kick`.
- Persistent host: `/tp` between two joiners with a correction, `/save-all`, `/stop`
  from an operator joiner (host exits 0, save written), restart from the save, and the
  same joiner is still an operator through its key.
- Two walking joiners for 30 and 60 s on generated terrain: 0 corrections, and each
  joiner sees the other's moving position through the host.
- Unit tests: `multiplayer::social`, `multiplayer::owner_tests`, `multiplayer::server_files`.

## Still missing for server parity

1. Prediction of the joiner's own block edits. Breaking and placing still wait one
   round trip for the host's answer.
2. Ping in the player list. The relay is TCP, so this needs an application echo.
3. Host-side knockback and conveyors on joiners (needs an impulse allowance, as the
   `engine_netcode` validator has).
4. Content parity: scoreboard, teams, gamerules, world border, resource packs and a
   mod handshake.
