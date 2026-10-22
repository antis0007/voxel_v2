"""Compare matched native proxy/full-voxel captures across the rock default fix.

Read-only image analysis; RGB errors are not physical radiance or silhouette error.
"""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops

p = argparse.ArgumentParser()
p.add_argument("root", type=Path)
p.add_argument("--include-facet", action="store_true")
p.add_argument("--include-directional", action="store_true")
p.add_argument("--include-final", action="store_true")
args = p.parse_args()
root = args.root
ignore = {"VOXEL_CAPTURE_DIR", "VOXEL_SAVE_DIR", "VOXEL_HORIZON_EXACT_PATCH",
          "VOXEL_HORIZON_REFERENCE_VOXELS"}
bounds = (250, 245, 425, 310)
baseline_settings = None
reports = []
variants = ["proxy", "rock"]
if args.include_facet or args.include_directional or args.include_final:
    variants.append("facet")
if args.include_directional or args.include_final:
    variants.append("directional")
if args.include_final:
    variants.append("final")
for prefix in variants:
    manifests = []
    images = []
    for suffix in ("beauty", "voxel128", "direct-light"):
        case = root / f"{prefix}-hill-{suffix}"
        m = json.loads((case / "manifest.json").read_text(encoding="utf-8-sig"))
        settings = {"arguments": [a for a in m["arguments"] if not a.startswith("--benchmark-debug=")],
                    "environment": {k: v for k, v in m["environment"].items() if k not in ignore},
                    "fov": m["fov_vertical_degrees"]}
        if baseline_settings is None:
            baseline_settings = settings
        assert settings == baseline_settings, f"Unmatched capture: {case}"
        manifests.append(m)
        im = Image.open(case / "camera.png").convert("RGB")
        assert im.size == (1280, 720)
        images.append(im.crop(bounds))
    assert len({m["sha256"] for m in manifests}) == 1
    delta = ImageChops.difference(images[0], images[1]).tobytes()
    direct = images[2].tobytes()
    luma = [0.2126 * direct[i] + 0.7152 * direct[i + 1] + 0.0722 * direct[i + 2]
            for i in range(0, len(direct), 3)]
    reports.append({"variant": prefix, "executable_sha256": manifests[0]["sha256"],
                    "proxy_vs_full_voxel_mean_absolute_channel_error": sum(delta) / len(delta),
                    "direct_display_luma_mean": sum(luma) / len(luma),
                    "direct_display_pixels_over_200": sum(v > 200 for v in luma)})
old = json.loads((root / "priority-water-source-3DD4B8EF.json").read_text(encoding="utf-8-sig"))
new = json.loads((root / "rock-source-F066057F.json").read_text(encoding="utf-8-sig"))
def hashes(m):
    return {f["path"].replace("\\", "/"): f["sha256"] for f in m["files"]}
a, b = hashes(old), hashes(new)
changes = sorted(k for k in a.keys() | b.keys() if a.get(k) != b.get(k))
assert set(changes) == {"src/shaders/world_shader.wgsl", "src/render.rs",
                        "src/render/material_finish_contract.rs", "src/horizon_review.rs"}, changes
report = {"camera_settings_matched": True, "roi_bounds": bounds, "legacy_to_rock_source_changes": changes,
          "variants": reports,
          "scope": "Actual native captures. Legacy-to-rock source comparison verifies the VS finish fallback plus test-only diagnostics. The optional facet variant is a separate directional-lighting candidate identified by its executable and review patch. ROI includes background/trees; no isolated silhouette, pixel-handoff or physical-radiance guarantee."}
out = root / ("rock-final-comparison.json" if args.include_final else
              "rock-directional-comparison.json" if args.include_directional else
              "rock-finish-and-facet-comparison.json" if args.include_facet else "rock-finish-comparison.json")
out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
