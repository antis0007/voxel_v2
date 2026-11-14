# Celestial bodies and the realm sky

Player-linked Hell planets (generators 143, 146 and 147) supersede the cold Nether palette
described below with a red volcanic atmosphere and 1–3 seeded moons. Historical
Nether saves retain this document's cold sky. See
[Hell planet and player-built portals](HELL_PLANET_AND_PORTALS.md).

2026-11-03. Sky shader pass for the moon, gas giant, sun, stars, the planet's
limb from orbit and the Nether's own sky. Shader code lives in
`shaders/celestial_bodies.wgsl` and `shaders/celestial_sky.wgsl`, composed in
front of `sky_shader.wgsl`.

## Bodies

- Moon: a lit sphere in direction space. Albedo sits on a 32-texel grid across
  the disc (maria from fbm, three crater sizes with rims, one rayed crater);
  lighting is Lommel-Seeliger plus a little Lambert, continuous across texels.
  Relief compares each texel with the next one toward the sun, so craters
  shade near the terminator. Planet mode adds earthshine on the night side.
  The moon is drawn at least 2.2 degrees in radius (`planet.rs`
  `MIN_MOON_RADIUS`, mirrored by `MOON_COS_RADIUS` on flat worlds) with an
  aerosol glow below the air. On flat worlds it is always full because
  `world.rs` places it opposite the sun.
- Gas giant (planet mode): banded sphere on a 32-texel grid, one storm oval,
  an icy ring with a Cassini-style gap, ring shadow on the body and body
  shadow on the ring, a faint fill on its night side. Bands follow true
  latitude about its spin axis, which comes from `star_systems.ron` (tilt
  26.7 degrees toward 240, so the ring stays 20-27 degrees open from home over
  the first 24 days) and rides in `cam.planet[2..4].w`. Its true size is under 0.1
  degree; `MIN_GIANT_RADIUS` draws it at 2.6 degrees (about 50 px at 1600x900).
- Moon and giant dim and redden toward the horizon through the air column
  (`extinction` in `sky_shader.wgsl`).
- Planet worlds build the lighting state from `planet::solar_time`, the flat
  clock time whose sun has the planet sun's height and direction of travel,
  so the sky gradient, fog, ambient and sun colour follow the planet sun.
  Before this the sky could be daytime blue with the sun 75 degrees down.
- Sun in vacuum: limb-darkened disc, corona streamers that fade in with
  `vacuum`, glare. The flat-world `sun_disk` is unchanged.

## Stars and galaxy

- Star magnitudes vary (most faint, a few bright); density rises along the
  galaxy band and above the air.
- Galaxy band: unresolved star clouds, dust lanes, a warm core, pink knots and
  a grain hash on the star grid. Drawn only where stars are visible.

## Planet limb from orbit

Single scattering along the eye ray (12 jittered samples, 4 toward the sun),
Rayleigh and Mie with coefficients scaled for a 20 km planet, plus a faint
green airglow layer on the night side. Rays deep inside the disc skip it.

Cover fixes found on the way (peer file `render/planet.rs` and the cover path
in `world_shader.wgsl`): border vertices built a zero-length tangent and a NaN
normal, which drew every cube-face edge as a black line; each face now also
hangs a 400 m skirt; the night side drops to a cool moonlit floor
(0.20, 0.25, 0.36) instead of 0.85 of day. A 0.05 floor was tried first and
turned half the planet black. The globe renderer (on by default since
2026-11-03) draws over the cover from orbit; `VOXEL_GLOBE=0` shows the cover.

## The Nether

The Nether used the overworld sky with clouds and skyhooks. `render.rs` now
writes `cam.planet[0].w = -1` in the Nether. The sky draws `cel_realm_sky`
(indigo dome, teal horizon haze, stepped violet and teal curtains, frost
sparks), the cloud composite returns nothing, `orbital::build` is skipped and
`world::portals::realm::lighting` sets a dim, cold, fixed light. Palette
follows the hell gate's vista.

## Evidence and cost

Captures in `target/visual-direction/cel-*` (`b0` before, `a1`..`a7` after)
and crops in `target/celestial-20261102/`. Sky pass on the 4090 at 1600x900:
0.05-0.09 ms before, 0.08-0.11 ms after. Nether benchmark runs often fail
with "distant terrain never became ready"; that predates this pass.
