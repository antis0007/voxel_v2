# Artificer tally wand

A 45 cm common wand with carved heartwood, a short leather grip, three copper
windings, a replaceable ceramic socket and an offset amber mineral. The two
short marks on the socket refer to the Artificers' signed-coil/tally tradition
in `docs/design/WORLD_LORE.md`. This is one equipment baseline, not the entire
wand ecosystem or an assertion of artistic acceptance.

## Editable sources and reproduction

- `artificer_tally.blend`: separate named functional components, UVs, packed
  authored pixel atlas, shared 19-joint rig, nine existing actions, cameras/lights.
- `fitted_hand.blend`: preserved-source derivative of `hand_v3`, with a lower
  opposing thumb and a quiet leather cuff with open rims. Earlier sources are untouched.
- `albedo.png`, `texture.json`: palette, strips, explicit pixel rectangles.
- `hand_albedo.png`, `hand_changes.json`: fitted-hand texture and changes.
- `material_masks.png`: authored material/metal/emission-eligibility reference.
  The current renderer uses the corresponding named GLB material classes;
  it does not sample this mask image.
- Runtime exports: `assets/runtime/viewmodels/artificer_tally/` contains the
  actual wand/hand GLBs and external RGBA atlases consumed by WGPU.

From the repository root, using Blender 5.0.1:

```powershell
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' --background --python tools/blender/finish_artificer_wand.py
& 'C:\Program Files\Blender Foundation\Blender 5.0\blender.exe' --background --python tools/blender/fit_artificer_hand.py
cargo build --profile release-fast -p minecraft_style_voxel_prototype
```

The first script regenerates this candidate only. Artist changes to these new
files should be preserved in a new revision before rerunning it. It does not
regenerate the older Copper study, the other wand families, or hand_v3.

## Runtime behavior

Normal play selects this wand and fitted hand without a review flag. The three
minor review items remain available separately. The atlases occupy two halves
of one nearest-sampled 512x256 texture; no texture work occurs per frame.

The mineral retains its painted color at rest. Casting drives its energy tint
from the spell that actually fired, rather than the next deck slot. The release
flash and local cast light follow the exported focus coordinate through the
animated grip and viewmodel FOV transformation. The former camera-offset spark
burst and old geometry-specific floating decorations are removed. Projectile
physics and collision origins remain unchanged; this is not a spell-system
rewrite or proof of projectile/muzzle convergence in combat.

The sleeve bends from the animated wrist to the camera elbow. Surface normals
include the derivative of that bend. The rig's nine existing animations are
retained; no newly authored animation set is claimed.

## Review

Blender front/side/three-quarter renders and genuine 96px and first-person black silhouettes:
`target/artificer-tally/`. The first render was rejected as too candle/fork-like;
the final socket uses a single swept ceramic cheek and a small rear contact.
The first native pass exposed the older hand's loose grip, prompting the fitted
hand derivative and animated cuff anchoring.

The final wand has 976 triangles; the fitted hand and sleeve have 866. Both
exports contain the same 19 joints and nine animation clips. GLB validation
logs are in `target/artificer-tally/`.

Native captures and their executable hashes are recorded separately in the
delivery manifest. They use the normal asset path in isolated benchmark worlds,
with diagnostic poses for repeatable comparison. This is rendered evidence,
not interactive playtesting or performance acceptance. The hand remains
deliberately faceted; finger spacing and expressive animation can still improve.
No existing save format, terrain generator or old asset was replaced on disk.

Final executable: `target/artificer-tally/finished-kit.exe` (SHA-256 recorded in
`artifacts.json`). Final native captures under `target/artificer-20261025/`:

- `tally-kit-day/camera.png`: normal asset selection, daylight.
- `tally-kit-inspect/camera.png`: corrected open cuff and wrist underlap.
- `tally-kit-frost/camera.png`: cool release tint at the socket.
- `tally-kit-motion/`: 68 frames over six seconds, pose cycle with a static
  camera. Sampled frames 000/020/040/060 show inspection, idle and casting.
- `tally-delivery-spark/camera.png`: underground warm release, before the final
  cloth entrance-cap correction. Wand geometry and shader are identical.

All four final runs exited zero. One earlier underground frost attempt exceeded
the harness timeout before capture; the subsequent studio frost run succeeded.
No walking/sprinting, sustained-fire combat, projectile convergence or isolated
performance acceptance is claimed. Local build and nineteen composed WGSL
validations passed; no unit test suite or CI was run.

A shared-project startup blocker was also corrected in `assets/descriptions.ron`:
the new `spell_95` description contained a semicolon and a spelled-out numeral
rejected by the existing content validator. Its interaction instructions remain
in the codex; spell behavior was not changed by that wording correction.
