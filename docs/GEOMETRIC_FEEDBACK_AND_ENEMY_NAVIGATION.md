# Geometric feedback and enemy navigation — 2026-10-21

Implemented in the shared checkout:

- Look-at icons use native pixel stair outlines: triangle for hot material, pentagon for a machine, hexagon for ordinary material. A five-pixel acquisition glint appears once on target change. Panel entrance uses stepped opacity and respects the existing reduced UI motion setting.
- Ordinary enemy windups use a small triangle; wards use an open hexagon instead of a translucent sphere. Area hazards use circumscribed hexagons, keeping the entire circular damage area inside the warning.
- Non-boss navigation caches bounded best-first routes through live voxel collisions: full body clearance, diagonal sweeps, one-metre upward candidates, conservative downward candidates, lava rejection, and route refresh after goal movement. One actor may search per frame; each search has at most 96 expansions, 384 nodes and an 18-metre local radius. Routes are transient and require no save-format change.
- Existing group alerts, last-seen pursuit, separation, flanking, ranged spacing and projectile evasion feed tactical destinations into navigation. Sight updates every 0.12–0.195 seconds. Ranged enemies lead moving targets with a bounded prediction committed at windup, retreat farther when wounded, change flank side after two attacks, and steer away from active or pending area hazards.
- Warden and porter legs use opposing cycles with foot lift instead of multiplied sine waves. Movement amplitude settles at rest; skitterer and kiln limbs also settle. Walk phase uses horizontal distance, excluding falling. Turning is bounded, attack poses ease into windup and out through recovery, and gravity continues during stagger and windup. Collision side steps use only the remaining movement allowance.

Validation: `cargo check --offline --target-dir target` succeeded. Log: `target/geometric-ai-check.log`. Existing warnings remain. No tests, hosted CI or game session were run for this pass.

Limits: navigation is local and bounded, not a global dungeon navigation mesh. It does not destroy terrain, open doors or provide general vertical flight planning. Boss AI remains in its existing encounter controllers. Adaptive behavior here means reactions to current health, player motion, attack cycles, projectiles and hazards; there is no learned player model. Actual animation feel, route success and frame cost require gameplay review. Other active chats may subsequently extend the enemy presentation wrapper and creature bodies.
