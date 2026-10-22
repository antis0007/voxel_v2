"""Measure local land boundaries in matched native water-depth diagnostics.

Production debug31 writes non-water geometry as exact black. Water has a
positive grayscale depth; daylight sky is not black. This measures visible
land/sky or land/water boundaries, including trees, not hidden feature edges.
"""
import argparse
import json
import math
from pathlib import Path
from PIL import Image

p = argparse.ArgumentParser()
p.add_argument("root", type=Path)
p.add_argument("--prefix", default="rock-geometry")
p.add_argument("--reference-span", type=int, choices=(64, 128), default=64)
p.add_argument("--targets", nargs="+", choices=("ridge", "shore", "forest"), default=("ridge", "shore", "forest"))
p.add_argument("--allow-static-duration-difference", action="store_true",
               help="Allow only capture-duration differences; record them explicitly. Never use for motion/physics.")
args = p.parse_args()
root, prefix = args.root, args.prefix
ignore = {"VOXEL_CAPTURE_DIR", "VOXEL_SAVE_DIR", "VOXEL_HORIZON_EXACT_PATCH",
          "VOXEL_HORIZON_REFERENCE_VOXELS"}
base = root / f"{prefix}-base"
manifest = json.loads((base / "manifest.json").read_text(encoding="utf-8-sig"))
assert "--benchmark-debug=water-depth" in manifest["arguments"]

def mask(path, bounds):
    image = Image.open(path).convert("RGB")
    assert image.size == (1280, 720)
    x0, y0, x1, y1 = bounds
    return {(x, y) for y in range(y0, y1) for x in range(x0, x1)
            if max(image.getpixel((x, y))) <= 2}

def edges(points, bounds):
    x0, y0, x1, y1 = bounds
    return {(x, y) for x, y in points if x0 < x < x1 - 1 and y0 < y < y1 - 1
            and any((x + dx, y + dy) not in points
                    for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1)])}

def nearest(a, b):
    return [math.sqrt(min((x - u) ** 2 + (y - v) ** 2 for u, v in b)) for x, y in a]

results = []
duration_differences = []
for name, center in {"ridge": (746, 265), "shore": (859, 285), "forest": (416, 271)}.items():
    if name not in args.targets:
        continue
    case = root / f"{prefix}-{name}{args.reference_span}"
    other = json.loads((case / "manifest.json").read_text(encoding="utf-8-sig"))
    assert other["sha256"] == manifest["sha256"]
    if other["arguments"] != manifest["arguments"] and args.allow_static_duration_difference:
        def without_duration(values):
            assert not any(v in values for v in ("--benchmark-terrain-sweep", "--benchmark-physics"))
            return [v for v in values if not v.startswith("--benchmark-duration=")]
        assert without_duration(other["arguments"]) == without_duration(manifest["arguments"])
        duration_differences.append({"target": name,
            "base": [v for v in manifest["arguments"] if v.startswith("--benchmark-duration=")],
            "reference": [v for v in other["arguments"] if v.startswith("--benchmark-duration=")]})
    else:
        assert other["arguments"] == manifest["arguments"]
    for key in manifest["environment"].keys() | other["environment"].keys():
        if key not in ignore:
            assert manifest["environment"].get(key) == other["environment"].get(key), key
    cx, cy = center
    bounds = (cx - 24, cy - 20, cx + 24, cy + 20)
    a, b = mask(base / "camera.png", bounds), mask(case / "camera.png", bounds)
    ea, eb = edges(a, bounds), edges(b, bounds)
    result = {"target": name, "roi_bounds": bounds,
              "land_mask_disagreement_pixels": len(a ^ b),
              "proxy_boundary_pixels": len(ea), "reference_boundary_pixels": len(eb)}
    if ea and eb:
        distances = sorted(nearest(ea, eb) + nearest(eb, ea))
        result.update({"maximum_bidirectional_boundary_distance_px": max(distances),
                       "p95_bidirectional_boundary_distance_px": distances[math.ceil(0.95 * len(distances)) - 1],
                       "within_two_pixels_in_this_roi": max(distances) <= 2})
    else:
        result["status"] = "No identifiable boundary in one or both crops; no silhouette pass claimed"
    results.append(result)
report = {"executable_sha256": manifest["sha256"], "settings_matched": True,
          "capture_duration_differences": duration_differences,
          "arguments_exactly_matched": not duration_differences,
          "reference_requested_span_m": args.reference_span,
          "measurements": results,
          "scope": "Native debug31 black land masks, small fixed ROIs. Includes ground/trees and possible occluders. Daylight, fixed exposure and no fog diagnostics required. Not whole-feature identity, hidden contours or active moving-handoff acceptance."}
suffix = "" if args.reference_span == 64 else f"-span{args.reference_span}"
output = root / f"{prefix}-silhouettes{suffix}.json"
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2))
