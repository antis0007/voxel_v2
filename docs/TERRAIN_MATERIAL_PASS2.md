# Terrain material pass 2 review

Evidence and immutable source archives: `reviews/terrain-pass2-20261021/`.
The baseline is the dirty working tree at the start of this pass, not HEAD and
not an authentic pre-material-pass build. Its release executable was rebuilt.
The named historical `b_fix_noon` / `a3_fix_noon` and meadow pairs were not located
in the supplied attachment or searched review directories; no comparison against
those images is claimed. Hills fixture views are reproduced instead.

## Ownership and implementation

`owned-paths.txt` lists this pass's files. Preserve the separate world-generation,
factory, inventory, storage, audio and HUD work. Main/render hooks are limited to
debug selection and opt-in review controls. `implementation.patch` compares with
the frozen starting source; shared-file hunks belonging to other work must be
reviewed separately. No saves were opened for these generated-world benchmarks.

- Stable `BlockId` and save IDs are unchanged. `SurfaceDetail::shader_key` is
  `renderer_family + 64 * (BlockId + 1)`, stored as an exactly representable f32
  in the existing Float32x4 surface attribute. It has six family bits, not five.
- Named Rust families: gravel 33, tuft 34, pebble 35, capped turf 36.
  Shader decoding splits family/block at vertices. A flat u32 varying transports
  the categorical draw family. Horizon triangles retain interpolated material
  *coverage*, with draw family 0; they are not categorical voxel faces.
- Capped turf retains the grass block's material override and gameplay identity.
  Immediate Air above the support defines an exposed turf surface. Water,
  foliage, glass/ice and solid neighbors occupy that space; a roof separated by
  Air does not cap it. The greedy key includes this distinction, including the
  conforming boundary-face emitter. Bottom faces remain soil.
- Ground detail already required immediate Air and sky access. That rule remains;
  a separated roof may suppress tufts via skylight without suppressing fringe.
  The existing padded one-cell halo and diagonal edit invalidation cover the
  new neighbor read. Local dirty revisions now reject old mesh results before
  GPU upload, instead of displaying stale fringe and scheduling a repair after it.
  Unrelated global revisions do not reject useful work.
- Shape cells compare squared distances and take two final square roots. When
  detail attenuation reaches zero, grass/clod/aggregate/plane functions return
  their coarse color before nine neighbor-cell evaluations. Removing dead grain
  is cleanup; no separate performance gain is asserted for it.
- Grass's painted lean-direction highlight and one-sided fake cast shadow were
  removed. Dry pigment tips, mineral color, seams and root occlusion remain.
  Material height gradients still tilt the normalized lighting normal.
- Fine blade randomness converges to its density mean; narrow soil seams fade
  with the texel footprint. The existing continuous world-position derivatives
  and largest-singular-value footprint are computed before dispatch/discards.
  Feature-cell attenuation ends before multiple cells fit a pixel. Statistical
  coarse colors remain approximations; no exact analytic integration is claimed.
- Six-blade opaque tufts share root-anchored wind with the shadow shader through
  `terrain_detail_shared.wgsl` and normal shader-source composition. Maximum bend
  is 0.04 m, increasing quadratically with height. Roots have zero bend. Existing
  conservative chunk bounds exceed this; shadow bounds add the explicit margin.
  Tufts use near LODs 0/1 and only the two near shadow cascades. Both near maps
  refresh for wind. Distant cascades omit tuft triangles. Pebbles remain stationary.
  This renderer uses FXAA, not terrain TAA/motion-vector history.
- Held-light flicker is seeded by time, not its changing position. Stationary
  world torches retain positional seeds. Spell lights sharing the dynamic array
  still use the existing positional flicker seed; that remains a limitation.

## Diagnostics and physical controls

Existing renderer debug UI and `--benchmark-debug=` accept:

| Value | Meaning |
|---|---|
| `material-id` | Flat renderer family; unsupported 37–39 / 57–63 are magenta |
| `albedo` | Actual unlit linear base color after material/grime processing |
| `geometric-normal` | Input world normal, XYZ mapped to RGB |
| `shading-normal` | Final normalized world lighting normal, XYZ mapped to RGB |
| `roughness` | Actual BRDF roughness after filtering |
| `direct-light` | Sun/moon diffuse and specular, linear HDR |
| `ambient-diffuse` | Existing ambient/bounce diffuse; excludes environment specular |
| `local-light` | Actual accumulated local diffuse/fill/specular, linear HDR |
| `terrain-shadow` | Existing terrain-shadow visibility view |

Debug output bypasses exposure, fog and production grading. HDR values may clip
on display; these are contributions, not false-color photometric measurements.

The physical lattice remains 20 texels/m (0.05 m/texel). Feature cell pitches
remain 0.20 m grass/clods, 0.15 m gravel and 0.45 m rock. These are cell pitches,
not guaranteed object diameters. The jagged fringe remains 7–14 texels, clipped
to its exposed face; it does not paint through capped grass blocks below.
The old approximate 55–57% one-metre mean coverage remains an art choice.
`PATCH_METRES = 5` replaces the old five-block patch extent; nominal sampling
density remains 0.40 items/m² before patch modulation and the 384-item chunk cap.
Normal amplitudes remain gravel 0.34, clods 0.16 (stone inclusions 0.34), planes
0.10 and grass 0.05. Existing material detail strength controls their magnitude.
Filtering retains the existing 0.20–0.50 cells/pixel attenuation thresholds.

Opt-in environment settings are read once per process:

- `VOXEL_TERRAIN_EXPOSURE=<positive value>` fixes exposure and disables bloom.
- `VOXEL_TERRAIN_LIGHT=off|on|sweep`: disable all locals, force a warm held light,
  or replace locals with a neutral moving light. Sweep uses wall time, so cloud
  time can remain frozen. Empty uses production lights.
- `VOXEL_TERRAIN_WIND=<0..20>` overrides the existing wind-speed control.
- `VOXEL_TERRAIN_ROUTE=1` runs walking, sideways motion, pan, grazing traverse
  and ascent/descent after residency warmup.
- `VOXEL_TERRAIN_EDITS=1` clears an opt-in test volume at negative X / a vertical
  chunk boundary, then caps, uncovers, roofs across Air, digs and replaces.
  It leaves the resident render radius for twelve seconds while editing and
  returns. Allow at least 90 measured seconds for this complete sequence.

Production exposure is a time/weather curve, not camera-adaptive exposure.
This pass does not retune terrain colors to compensate for local lighting.

## Reproduction

Use local compiler temporary storage if the sandbox temp archive rename fails:

```powershell
$env:TMP = (Resolve-Path target/compiler-tmp).Path
$env:TEMP = $env:TMP
cargo build --manifest-path reviews/terrain-pass2-20261021/final/source-review3/Cargo.toml --bin terrain_material_review --release --locked --offline --target-dir C:/dev/voxel_v2_backup/target
cargo test --manifest-path reviews/terrain-pass2-20261021/final/source/Cargo.toml --release --locked --offline --target-dir C:/dev/voxel_v2_backup/target terrain_contract
cargo test --manifest-path reviews/terrain-pass2-20261021/final/source/Cargo.toml --release --locked --offline --target-dir C:/dev/voxel_v2_backup/target tuft_patch_field
```

The normal release output returned Access Denied when replaced. The immutable
`source-review3.zip` contains identical source plus a Cargo `[[bin]]` alias named
`terrain_material_review`, avoiding that output without copying dependency caches.
`source-review3.sha256`, `build-review3.log` and `binary-final.sha256` identify
this reviewed build. The original `source.zip` remains available with its focused
test receipts; it has the same code and original Cargo binary name.

Capture using the frozen executable, never a stale target copy:

```powershell
./tools/Review-TerrainPass2.ps1 -Executable reviews/terrain-pass2-20261021/final/terrain-final.exe -Label final/example-motion -Case motion -Seconds 46 -Width 1920 -Height 1080 -Animate -Wind 6 -Video
./tools/Review-TerrainPass2.ps1 -Executable reviews/terrain-pass2-20261021/final/terrain-edit-review.exe -Label final/example-edits -Case edits -Seconds 96 -Video
./tools/Review-TerrainPass2.ps1 -Executable reviews/terrain-pass2-20261021/final/terrain-final.exe -Label final/example-light -Case light -Seconds 26 -Exposure 0.8 -Light sweep -Video
```

The script records executable SHA-256, arguments, environment, 70° vertical FOV,
native render resolution, logs and run summaries. Video is a 10 Hz readback
recording with capture timestamp gaps preserved, not an external screen recording.
It introduces cost; timing runs use `-Case perf` without readbacks. That legacy
benchmark path does not execute the capture-only fixture, and its day/night clock
runs normally; match this path/settings between builds. It measures the natural
spawn view, not the fixture, despite the opt-in fixture environment variable.

GPU timestamps use the existing asynchronous ring/readback and actual queue
timestamp period. `terrain` covers the combined world pass, including resident
terrain/detail, horizon and other draws in that pass; it is not material-fragment
time alone. Shadow, indirect and cloud passes are separate. `passes` is a sum of
instrumented GPU passes, not a whole-frame fence duration. CPU submission/acquire
times overlap GPU work and must not be added to GPU durations. Detail triangles
are combined into chunk meshes, so separate live tuft triangle counts are not
available; six blades cost 24 triangles/tuft, loose pebbles 15 triangles each.

Results, baseline failure attribution and remaining acceptance limits are recorded
in `reviews/terrain-pass2-20261021/RESULTS.md` after the runs finish.

References: [WGSL](https://www.w3.org/TR/WGSL/),
[PBRT sampling](https://pbr-book.org/4ed/Textures_and_Materials/Texture_Sampling_and_Antialiasing),
[Filament](https://google.github.io/filament/main/filament.html),
[wgpu timestamps](https://wgpu.rs/doc/wgpu_examples/timestamp_queries/index.html).
The compiled wgpu 27 / Naga 27 validation remains authoritative for this build.

The canonical run/status index is reviews/terrain-pass2-20261021/RESULTS.md. The corrected edit harness is source-review4.zip / terrain-edit-review.exe (release-fast); its renderer shader is identical to source-review3. Earlier camera/edit recordings are explicitly superseded there. Gravel also has its own continuous far-proxy coverage, preserving the filtered aggregate mean across the handoff.

