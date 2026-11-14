# Enemy perception and stealth (design)

Status: approved 2026-11-14. Owner: "keep improving hidden enemies and enemy AI capabilities … make
sure it makes for the MOST FUN GAMEPLAY". Slice 1 of a hidden-enemy / secrets track; slice 2 is new
hidden-enemy kinds, slice 3 is secret rooms, traps and triggers.

## Intent

Today a hostile sees the player instantly, in every direction, whenever range and line of sight
allow (`adventure.rs` sight check). Light, crouching and facing do nothing; noise only reaches
burrowers; ambushers wake on a fixed distance. Sneaking is impossible, so hidden enemies are only
surprises, never a game.

Make detection a readable contest the player can win: creep through the dark behind an enemy,
lure it with noise, strike first for a big bonus, or break line of sight and hide while it searches.

## Fun rules (the tuning contract)

1. **Never spotted without warning.** Outside point-blank range, detection always passes through a
   visible *suspicious* stage (amber marker, head turn, slow approach) before *spotted* (red marker,
   warning sting, chase).
2. **Every player choice matters and is legible.** Darkness, crouching, standing still and staying
   behind an enemy each slow detection; sprinting, casting and light speed it up.
3. **Noise is a tool, not only a risk.** A spell impact on a wall pulls enemies to the impact,
   so players can distract and split groups. Hearing alone never fully spots the player.
4. **Losing them is possible and tense.** Breaking line of sight sends the enemy to the last-seen
   spot, then through a short search of nearby points, then home.
5. **Stealth pays.** Hits on an unaware enemy deal double melee / 1.5x spell damage and stagger.
6. **Point-blank is honest.** Within about 2 m an enemy notices at once; ambushers 1.6 m.

## Non-goals and anti-goals

- No change to bosses (always aware in their arenas), wildlife, settlement guards, burrowers'
  own hearing model, or field foes beyond what the shared sight path already gives them.
- No save change: perception state is transient and enemies are not saved (wildlife encode only).
- No wall-clock time, no RNG: search points use a positional hash of site, slot and step.
- Anti-goal: a stealth system the player cannot read. If they cannot see why they were spotted,
  it has failed even if it works.

## Approaches

1. **Awareness meter on the existing hostile path** (chosen). One `perception.rs` module owns
   gain, hearing, search and the marker; `adventure.rs` gets small hooks. Shared by every generic
   hostile and by rubble ambushers.
2. Per-species perception rows in `assets/bestiary.ron`. Better long term, but changes a content
   struct (dump test, peers' rows) before the feel is proven. Deferred: global constants first,
   species columns once tuned.
3. A full behaviour-tree rewrite of `Adventure::update`. Rejected: 1,500-line peer-shared loop,
   high regression risk, no extra fun in this slice.

## Design

- `Enemy` gains transient fields (serde default under `multiplayer`): `aware: f32` (0..1),
  `investigate: Option<Vec3>`, `search: f32`, `search_step: u8`, `spotted_flash: f32`.
- **Sight gain** each perception check, when within sight range and line of sight:
  `rate = 1.7/s * cone * distance * light * posture`.
  - cone: front 1.0, side 0.55, behind 0.2 (enemy yaw vs direction to player eye).
  - distance: point-blank (< 2.2 m) sets aware = 1; otherwise `((R - d) / (R - 2.2))^1.3 + 0.15`.
  - light at the player's eye: `max(block_light/15, sky)` where sky is 1.0 outdoors by day,
    0.35 outdoors at night, 0 under a roof/cave; factor `0.25 + 0.75 * L`.
  - posture: crouched 0.45, still 0.6, sprinting 1.6, casting 1.5 (multiplicative).
  - Already-spotted enemies keep tracking while line of sight holds.
- **Decay** 0.22/s without stimulus, so suspicion lingers long enough to read.
- **States**: unaware < 0.3 ≤ suspicious < 1 = spotted. Spotted sets `visible`, `last_seen`,
  `alert` exactly as today and plays a warning sting on the transition.
- **Suspicious behaviour**: stop wandering, face the stimulus, walk to `investigate` at reduced pace.
- **Hearing**: `Adventure::noise(pos, radius)` events this tick. Sources: sprint footsteps
  (7 m), plain walking (3 m), landings (8 m), casting (10 m), block breaking (8 m), melee
  impacts (8 m), projectile impacts (9 m at the impact). Heard when distance < radius (x0.55
  through walls); gain `0.25 + 0.6 * (1 - d/r)`, capped at 0.95; sets `investigate`.
  Crouched movement makes no footstep noise.
- **Search**: when a spotted enemy loses sight and reaches `last_seen` (or alert runs out),
  visit 3 hashed points ~3 m around it over ~7 s, then go home.
- **Ambushers**: rubble lurkers accumulate awareness with cone = 1 and wake at 1; loud noise
  nearby or any disturbance still wakes them. Cache mimics keep their chest trigger.
- **Sneak attack**: target with aware < 0.3 and not visible: melee x2, spell x1.5, extra stagger,
  a distinct cue and spark.
- **Marker**: small glowing diamond over the head. Amber growing with awareness while suspicious,
  dim pulse while searching, red flash for ~1 s on spotted. Hidden while concealed.

## Left to the builder

Exact constants after playtest, marker size/height per body, which existing cue plays for the
sneak hit, crouch detection source (`player.crouch_t`).

## Verification

`cargo check` / clippy for the app crate; playtest night pose: crouch past a Scree Lurker vs
sprint past; lure test with a spell impact.
