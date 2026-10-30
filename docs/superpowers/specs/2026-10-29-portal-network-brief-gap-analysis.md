# Portal network ("Webway") brief: gap analysis

Status: analysis only, no code changed. Owner questions in section 6.

Input: "AXIOM / MERIDIAN Webway Dimension ... Technical Design Document,
Revision 1.0", pasted 2026-10-29. The paste was cut at 50,000 characters,
inside section 15.5; sections 15.5 onward were not read. Not staged in
`docs/NEW/` yet.

Decision 0039 applies: briefs are backlog, not inputs of record. MASTER_PLAN M1
(`dam_break`) keeps priority.

## 1. Name

"Webway" is the name of the Aeldari portal network in Games Workshop's
Warhammer 40,000. 0039 §3 already declines designs that follow other works,
and new proper nouns need approval. Code uses `portal_network`; no player text
or RON row uses the brief's name. Navigator tier names (§14.2) are unapproved.

## 2. Already present

| Brief section | Code |
|---|---|
| §11.2 portal edge with rigid frame transform | `crates/app/src/world/portals.rs:90` (`Portal { source, destination, entrance, exit }`, `DQuat` frames, velocity mapped at `:306`) |
| §11.2 unit scale only | `portals.rs:105-106` rejects transforms the fixed-gravity controller cannot traverse |
| §12.2 bounded recursion | `portals/views.rs`: `MAX_TRAVERSALS = 4`, `MAX_VIEWS = 8`, `MAX_TARGET_PIXELS = 4_194_304`, resolution halves per depth |
| §12.1 render targets, aperture clip | `render/portals.rs`, clip plane in `views.rs:44` |
| Dimensions | `Realm::{Planet, Nether}` (`portals.rs:13`), `WorldType::Nether` |
| §6.3 retransmitted command runs once | `crates/netcode/src/operation.rs` (receipts, e83d7188) |
| §5.3 pristine chunks regenerate, edits ship | `netcode/src/chunk_stream.rs:96` (`delivery(version, pristine_hash, cached)`) |
| §8.1 revisioned state | `netcode/src/container.rs` (viewer revisions); acked delta baselines, stale-brick resends in `cells.rs` |
| §7.5 interest | chunk interest with view priority, cache-aware delivery (`chunk_stream.rs`) |
| §5.1 positional seeding | `engine_core::hash`, generator gates in `voxel/src/config.rs` |
| §3.1 headless authority | `crates/app/src/headless.rs`, lockstep digests |
| §11.3 type B wrapped tube | peer work, uncommitted: `WorldType::Manyfold`, `player/radial.rs` capsule controller, `engine_worldgen::manyfold` field |

The brief's §3.1 crate tree (`meridian_*`) is not adopted; its own text says to
adapt names to the repo.

## 3. Conflicts with recorded decisions

| Brief | Decision |
|---|---|
| §1.2 "rejects mandatory full-world lockstep" | Owner-approved lockstep plan: every peer runs `engine_sim` for integer matter. Players and bodies stay host-authoritative (0039 §2), which matches the brief's tiers B and C. Tier A matter differs: lockstep, not host deltas. |
| §7.2 QUIC via Quinn, STUN/TURN | 0039 §4: no Quinn or iroh; own keys and UDP relay, then Steam. Lobby/relay already on the VPS. |
| §9.1 30 Hz authoritative tick | 120 Hz fixed step (`simulation.rs:31`), 60 Hz CA region (ADR 0018). |
| §5.4 BLAKE3 chunk hash tree | New dependency, needs an ADR line. Current hashes are FNV (`engine_core::hash`) and u64 pristine hashes. |
| §4.1 `RegionId(u128)`, i64 chunk coords | Chunk coords are `IVec3`; a region key is a save change and a netcode wire change. |

## 4. Missing, and what each costs

- Region graph with topology epoch and drift (§13): new save section and
  generator gate. Gates collide across branches; check the registry first.
- Portal crossing that changes gravity basis (§11.1): needs the radial
  controller's `gravity_frame` (peer work) and `Portal::validate` relaxed.
- Atomic cross-portal transfer with one owner (§11.5): today a crossing is a
  client position change; host authority over the transfer is new netcode.
- Visual vs interaction interest for portal views (§7.5, §12.4): no portal
  interest class; a portal view streams nothing from the far realm over the
  network yet.
- Navigator and hierarchical route planning (§14): no region graph to plan on.
- Endpoint synchronisation and space compression (§15): depends on the graph.

## 5. Smallest slice that fits

Graph of fixed regions inside the existing `Nether` realm: authored portal
pairs between two or three region boxes, no drift, rigid transforms, gravity
unchanged. Reuses `portals.rs` and `views.rs`. Tests: crossing preserves
velocity through the frame transform; a portal view of an unloaded region
shows the fallback state and blocks traversal (§11.4). Needs a MASTER_PLAN row.

## 6. Owner questions

1. Route as backlog under 0039, or name a portal-network milestone?
2. Drop the brief's name; `portal_network` as the code placeholder?
3. Lockstep for matter stands over the brief's §1.2? (0039 assumes yes.)
4. Re-paste or stage sections 15.5 onward; they were cut off.
