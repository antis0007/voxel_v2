"""Bounded pixel comparison of real, matched native reference captures.

This measures appearance error inside fixed ROIs; it is not a silhouette gate.
"""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops

parser = argparse.ArgumentParser()
parser.add_argument("root", type=Path)
parser.add_argument("--prefix", default="support")
args = parser.parse_args()
root, prefix = args.root, args.prefix
base_dir = root / f"{prefix}-unfogged"
manifest = json.loads((base_dir / "manifest.json").read_text(encoding="utf-8-sig"))
baseline = Image.open(base_dir / "camera.png").convert("RGB")
ignored = {"VOXEL_CAPTURE_DIR", "VOXEL_SAVE_DIR", "VOXEL_HORIZON_EXACT_PATCH", "VOXEL_HORIZON_REFERENCE_VOXELS"}
results = []
for target, (cx, cy) in {"ridge": (746, 265), "shore": (859, 285), "forest": (416, 271)}.items():
    case = root / f"{prefix}-{target}64"
    other = json.loads((case / "manifest.json").read_text(encoding="utf-8-sig"))
    assert other["sha256"] == manifest["sha256"], "Executable differs"
    assert other["arguments"] == manifest["arguments"], "Camera/render arguments differ"
    for name in manifest["environment"].keys() | other["environment"].keys():
        if name not in ignored:
            assert manifest["environment"].get(name) == other["environment"].get(name), name
    reference = Image.open(case / "camera.png").convert("RGB")
    assert reference.size == baseline.size
    bounds = (cx - 15, cy - 14, cx + 15, cy + 15)
    difference = ImageChops.difference(baseline.crop(bounds), reference.crop(bounds))
    rgb = difference.tobytes()
    errors = [sum(rgb[i:i + 3]) for i in range(0, len(rgb), 3)]
    results.append({"target": target, "center_px": [cx, cy], "roi_bounds": bounds,
                    "pixels": len(errors), "mean_absolute_channel_error": sum(errors) / (len(errors) * 3),
                    "pixels_rgb_sum_error_over30": sum(e > 30 for e in errors),
                    "maximum_rgb_sum_error": max(errors)})
report = {"executable_sha256": manifest["sha256"], "arguments_and_environment_matched": True,
          "references": results,
          "interpretation": "Native fixed-camera ROI differences include background and water/effects. No silhouette isolation, 1-2px handoff guarantee or motion acceptance is claimed."}
output = root / f"{prefix}-reference-pixels.json"
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
