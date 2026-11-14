# First-person equipment revision

Historical 2026-10-25 pass. The owner rejected this hand and sleeve; the current
replacement and evidence are in `VIEWMODEL_RESTART_20261026.md`.

The current direction is Minecraft modded fantasy: coarse, deliberate silhouettes,
restrained material regions and workshop construction consistent with the lore.
The owner's pasted quality brief adds anatomy, attachment, motion, lighting and
native capture requirements. It does not call for photorealistic skin or a noisy
increase in surface detail.

## Findings and implementation

The previous lofted hand used parallel finger paths and a narrow thumb base.
Voxelizing it at high resolution made the stair edges dominate. The Blender
source now has a palm heel, thumb pad, staggered finger joints and an opposed
thumb. Five weighted finger groups allow small grip adjustments without
separating the palm. Geometry uses coarse voxel silhouettes and narrow bevels;
skin shading follows the underlying anatomy to suppress dark stair stripes.
The sleeve has three broad compression folds, a turned cuff and a wrist anchor.

The default wand is shorter and wider above the grip. A ceramic collar and
forged jaws support a jade focusing stone. The small channel carries spell
colour independently of the stone. The mana gauge has a physical backing and
the grip has three leather turns. Six interchangeable shafts and six heads
retain separate silhouettes. Their materials come from the existing lore's
timber, bone, copper, brass, crystal and workshop instruments; no new faction
or story premise was added.

The old renderer added a neutral camera-only fill and assumed full sky exposure
for the held kit. Authored equipment now uses shared linear world lighting,
sun shadows, local lights and tone mapping, with a bounded, smoothed roof query
for sky access. Skin and cloth have no procedural micro-noise. Wood detail uses
stable local coordinates, and metal/glass retain distinct roughness.

The vertex shader gives camera-relative equipment a fixed 72-degree vertical
projection while retaining aspect ratio, camera roll and monotonic reverse-Z
depth ordering. Sprint FOV no longer changes its size. Hand, shaft, focusing
head and indicators share one attachment transform; the sleeve follows the
wrist instead of being independently placed against the screen.

## Motion and controls

`player/viewmodel.rs` owns presentation state updated at the existing fixed
step. A closed-form damped spring handles turn inertia. Ground speed drives
gait; breathing is small. Health loss adds recoil, equipment changes ease into
the hand, and casting uses separate preparation, release and recovery phases.
The animation does not delay gameplay casts or change movement physics.

Hold **I** to inspect equipment. Movement or casting cancels inspection.
Holding the existing cast key brings the wand toward the aiming pose; casting
still follows existing gameplay timing. The existing interaction input lowers
the tool and loosens the grip. A brief, infrequent idle grip adjustment occurs
after sustained inactivity. All key poses and attachment offsets are in
`assets/viewmodel/rig.ron`.

## Mesh and allocation ownership

Blender runs volume union, block remeshing, coplanar simplification and a narrow
edge bevel offline. Export drops degenerate triangles after coordinate rounding.
The engine caches RON meshes once. A five-joint weighted deformation affects
the hand, while rigid pieces use compact transforms. The scene vector and GPU
streaming buffer are reused; the former per-frame spell-deck Vec is now a fixed
array. The current engine still transforms and streams the held vertices each
frame. This revision does not introduce a separate persistent GPU skinning pass.

The rig and separate equipment pieces allow additional poses or attachment
sets without replacing gameplay state. Dual wielding and two-handed equipment
are future content, not features claimed by this revision.

## Review evidence

Builds, Blender receipts, focused local tests and performance receipts are under
`target/artificer-blender-20261025`. Native captures are under
`target/artificer-20261025/overhaul-*`. The executable hash and source manifest
identify the delivered candidate. The comparison uses native daylight, aiming,
windup, release, inspection, interior light, night and reduced-resolution views.

The focused tests cover finite geometry and normal lengths across poses and
equipment variants, wrist attachment, camera-relative bounds, rejection of
non-finite transforms, spring partition independence and warm-path allocations.
Full-game frame times are reported separately from the viewmodel microbenchmark.
No hosted CI is used. Camera-route playback does not establish player-controlled
traversal feel or performance parity with reference games.

## Measured result

The delivered executable is `target/artificer-blender-20261025/overhaul-final.exe`,
SHA-256 `82C3137C5FC8E5FCB95C500A388643DAEBCEF7D78217D96580733C1AFE1E2108`.
Its release build passed and validated 19 composed WGSL programs. Five focused
tests passed. The default authored assembly contains 10,993 triangles, excluding
runtime indicators and motes. The editable Blender source includes weighted
finger groups and an armature.

Two alternating runs per executable used an RTX 4090, Vulkan, 1280 x 720,
the same static camera and seed, six view chunks, clear daylight, clouds disabled,
10 seconds of warmup and 8 seconds of measurement. Frame captures were disabled
during these timing runs.

| Whole-game timing | Before A | Before B | After A | After B |
| --- | ---: | ---: | ---: | ---: |
| Average frame, ms | 3.546 | 3.792 | 3.981 | 3.924 |
| 95th percentile, ms | 5.073 | 5.147 | 5.253 | 5.231 |
| 99th percentile, ms | 5.826 | 5.696 | 5.818 | 5.762 |

The mean of the run averages increased from 3.669 to 3.953 ms, or 7.7%.
This is a measured regression in this scene, not a performance improvement.
Terrain draw counts varied between 233 and 246, and the executables include
the shared working tree; the difference cannot be attributed entirely to the
viewmodel. These short runs do not establish general performance acceptance.

The separate warm CPU draw measurement transformed 33,627 vertices in about
0.190 ms per iteration over 1,000 iterations. It recorded zero heap allocations
after cache and scene-buffer warmup. Replaying the removed deck-copy operation
recorded 1,000 allocations over 1,000 iterations. That comparison concerns the
deck copy, not all allocations in the old renderer. No cold-load or whole-game
allocation claim is made.

Native evidence includes rest, aim, windup, release, inspection, torchlit hall,
night, 640 x 360 and a 50-frame camera-route sequence. The before image uses the
same camera and world configuration. Run manifests retain the exact executable
hash, settings and successful process exit. The final receipt lists all paths.

Remaining limitations: vertices still transform and stream on the CPU each
frame; ambient sky access uses a smoothed overhead query rather than a full
environment probe. The 12.5 fps captured sequence supports pose continuity
review, not high-refresh or player-controlled movement acceptance. Inspection
and the default grip were visually reviewed; the other five part pairs have
geometry checks but have not each received the same full lighting review.
