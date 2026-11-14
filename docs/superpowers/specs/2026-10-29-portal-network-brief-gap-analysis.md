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
Warhammer 40,000. The owner approved the name on 2026-10-30 (0039 addendum);
designs, art and text stay original to this game. Code keeps `portal_network` as
the module name. Navigator tier names (§14.2) remain unapproved.

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

## 7. Update, same day: crystal gates landed (uncommitted at writing)

Correction to section 4: there was no crossing at all before this work, not a
client position change; the portal modules were declared and never called.

Implemented, same-realm only:
- Gates: a closed `Crystal` ring around a rectangular opening in a vertical
  plane (`world/portals/gates.rs`). F on the ring attunes, links, releases or
  closes. Matching sizes only (no scaling). Both faces link, four portals per pair.
- Crossing: the eye triggers it, the body follows, velocity and yaw turn with
  the portal; an obstructed exit holds the walker at the plane
  (`world/portals/traverse.rs`). Host or solo only: links live in the host's
  `gates.ron` sidecar, so the save version is unchanged.
- Windows: each destination renders from its virtual eye into a pooled target
  (terrain, horizon, models, sky; no clouds or indirect light), clipped at the
  exit plane through `Camera::portal_clip`, then composited with the veil
  (`render/portals*`, `shaders/portal_composite.wgsl`). Depth two, four
  destinations, resolution follows screen coverage.
- Openings without a window get a mist membrane; openings into a realm that
  is not loaded (the surface hell gate) get a raymarched underworld stand-in
  drawn from the owner's concept art. It is a stand-in until a second world
  can be resident and rendered.

Still missing: cross-realm windows and crossings, mobs and projectiles through
portals, multiplayer replication of links, drift and the region graph.

## 8. Update, 2026-10-30: gate visuals

- Every window and veil now carries light in the inside of its ring (violet
  for crystal gates, cold blue for realm gates) and a ripple that runs inward
  from the ring and dies within about a metre (`portal_composite.wgsl`).
- Realm gates report whether they show a vista (`realm::Gate::vista`). The
  underworld side of the hell gate gets a dark mist membrane instead of a bare
  opening.
- The hell gate's red threads and backing plate are gone; the vista fills the
  arch. Surface approach ruins are generator 89 (`docs/design/BURIED_HELL_GATES.md`).

## 9. Update, 2026-11-02: pearl membrane and crystal film

- Membrane and window share a nacre film (`pearl()` in `portal_composite.wgsl`):
  interference colour that drifts across terraces and walks with the viewing
  angle, one-texel growth lines and rare stepped sparks, all on the world's
  20 texels/m grid and faded out once a texel is under a pixel. Window tint
  keeps the destination's luminance. Rim bands carry the same spectral colours.
- Within 1.5 m of the plane, inside the outline, refraction, colour split and
  film strength swell: stepping through reads as pushing through a film.
- Windows under 0.12 % of the screen (`views::MIN_WINDOW_SHARE`) get no terrain
  pass; the shader fades a window into the membrane up to twice that share, so
  the hand-over does not pop. Test: `a_distant_gate_gets_the_membrane_instead_of_a_window`.
- Underworld vista march 110 to 72 steps with distance-relaxed tolerance. A/B
  with `VOXEL_PORTALS=0` on the underworld pose: frame GPU time within noise.
- Resonant crystal (`world_shader.wgsl`, kind 22): per-shard thin-film
  iridescence that re-hues the received light at equal luminance, faint
  face-on, stronger at grazing angles. Applies to all crystal, not only gates.
- Evidence: `target/portals-20261029/pearl-before-*` and `pearl4-*`
  (`pearl3-far`, `pearl4-front-r2`), review pose `threshold` added.
