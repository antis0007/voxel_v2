# Glassweaver presentation

Implemented 2026-10-29:

- Simulation-owned world-space foot contacts, paired stepping, short terrain probes,
  support loss recovery, and two-segment leg presentation. Action damage stays in
  the existing action controller. Contacts are derived state, not save data.
- Intact lattice nodes contribute local lights through `spell_lights`; destruction
  removes their light, damage dims it, and a travelling bolt charges its current node.
  At most two living Glassweavers contribute, with at most six lights each.
- Selected body optics and lattice nodes use crystal material 56, separately from
  matte body material 58. Crystal receivers use alpha -2 in the existing indirect
  buffer; water retains its existing -1..0 encoding. The compositor separates them.
- Refraction is a twelve-query half-resolution screen-space approximation with a
  fixed index of 1.46. It rejects foreground, other crystals, offscreen and missing
  hits. Misses preserve normal shading. No hidden geometry, true thickness,
  multiple interfaces, dispersion, or caustics are reconstructed. It depends on
  the existing indirect pass being enabled.
- Six rotating shards per broken node, capped at 24 per creature, expire after
  eight seconds. Bounded substeps, approximate spherical collision, restitution,
  friction, sleeping and support-loss wakeup drive their motion. Impact speed
  drives existing spatial clang sounds, capped at two per simulation update.
  This is cosmetic debris, not a general rigid-body solver or replicated gameplay.

Validation: the first Rust check passed. Following the shader integration, all
22 composed WGSL programs validated, but the full check encountered duplicate
`Item` declarations in unrelated `world/inventory.rs`. No tests were run.

Still requires runtime review: silhouette and contact stability across turns and
broken terrain; refraction at silhouettes and under camera motion; light costs;
impact sound quality; distant/remote-client fallback. Remote clients without
simulation-owned contacts use the model's rest stance. No visual, acoustic or
performance acceptance is claimed.

Remaining broader work: world-space GI, geometric acoustics, general rigid bodies,
additional creatures, and gameplay-authoritative vision impairment.
