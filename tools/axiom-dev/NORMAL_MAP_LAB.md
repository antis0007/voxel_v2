# Pixel Art Lighter authoring laboratory

The Forge sprite editor's **Normal map lab** button sends the current painted
sprite to Pixel Art Lighter 3.5.22. It works on the same selected sprite as
**Export PNG**, including unsaved paint. Restart Forge's local server after
updating these files.

1. Open a sprite source in Forge and finish its palette and painted regions.
2. Select **Normal map lab**. Generate Normal with the Object preset for an
   inventory object, or the appropriate preset for a surface or character.
3. Adjust directional surface hints. Move the light above, below, left and right;
   check that highlights follow intended form rather than painted brightness.
4. Export Normal to a candidate directory. Keep the albedo, normal, settings and
   source identity together. Review at native resolution and at distance.

The lab is an authoring experiment. There is no runtime normal-texture import in
this change. Exported light masks do not represent Axiom's semantic material masks,
roughness, metalness, ambient occlusion or emission.

## Source and isolation

Official source: https://www.moonfell-rpg.com/Pixel-Art-Lighter-v3.5.22.html

The source contains the MIT licence, copyright 2026 Red Hat Sprite, retained in
the offline file. `vendor/pixel-art-lighter-provenance.json` records the URL and
SHA-256 hashes. The only offline-source change removes the hosting provider's
trailing analytics and challenge scripts. The server prepends a small bridge
and disables filesystem picker APIs so export uses PNG downloads.

The iframe has an opaque origin: scripts and downloads are allowed, same-origin
access is not. Its response CSP also blocks networking and external code.
Only a PNG Blob from the direct parent at the server's origin is accepted. No
Forge token, save, project path, write API or asset installation action is passed.
No additional service, dependency or external request is required to author maps.

## Rendering contract found in source

World relief currently comes from material-shaped procedural WGSL normals in
`world_shader.wgsl` and `terrain_surface.wgsl`. `art_refiner/harness.wgsl` invokes
those functions directly. Existing `target/art/kinds.png` shows albedo/relief
pairs; it is historical evidence and was not regenerated during this change.

Held pixel-kit colour is uploaded as `Rgba8UnormSrgb` in
`render/viewmodel_texture.rs` and read with `textureLoad(held_kit_atlas, ...)`
in `world_shader.wgsl`. That path has no authored normal-map texture binding.
Inventory icons are UI art; flat lighting there is not a rendering defect.

Pixel Art Lighter normals encode X right, Y up and Z toward the viewer, as
RGB values mapped from [-1,1] to [0,255]. Decode normal RGB directly without sRGB
conversion, then normalize. Albedo requires sRGB-to-linear conversion before
lighting, followed by output encoding. These rules were checked in the tool's
source and exercised with a positive-green upward surface hint.

A future runtime importer needs a typed normal asset reference, matching UVs,
a verified tangent frame per face/held pose, linear texture storage, neutral
fallback, filtering/LOD policy and native moving-light comparisons. Do not assume
an OpenGL-style image matches Axiom's face projection automatically.

## Template integration

Reusable original templates should retain outline, semantic region IDs, shade
indices, form hints and grip/placement metadata. Painting and material ramps
derive albedo; form hints can guide normal candidates. A generated normal is
optional surface detail, not a replacement for held or placed geometry. Keep one
asset identity across inventory, held and placed representations. Do not derive
geometry or normal-map height solely from a brightness ramp that already contains
a painted light direction. Do not trace existing mod artwork.

## Verification on 2026-11-06

- The local Forge server starts with the new route and sprite-editor module.
- All added/modified JavaScript modules and every offline inline script parse.
- The upstream generation functions ran on the current copper-ore icon source.
  Repeated output was identical; all 256 alpha values were preserved, including
  135 opaque pixels. Maximum decoded normal-length error was 0.002449.
- An upward 45-degree hint produced positive-green normals.
- `copper-light-sweep.png` compares flat baseline (top) and generated candidate
  (bottom) across eight light directions with linear-light colour handling.

Interactive import, editing, export and sandbox behavior still need a browser
smoke test. The in-app browser could not reach the local server and native
Firefox was minimized and unavailable through the active UI adapter. The sweep
is offline analysis, not an engine capture. Current assets were not replaced and
no native rendering improvement is claimed. Native before/after comparison
requires the runtime importer described above.
