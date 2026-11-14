"""Rank Axiom watershed candidates against held-out elevation distributions."""

import argparse
import csv
import struct
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont


SLOPE_BINS = np.r_[np.arange(0, 91, 5), np.inf]
CURVATURE_BINS = [-np.inf, -48, -24, -12, -6, -3, -1, 0, 1, 3, 6, 12, 24, 48, np.inf]
RELIEF_BINS = [0, 2, 5, 10, 16, 24, 36, 52, 75, 110, 160, 240, np.inf]
ALIGNMENT_BINS = np.r_[-3.0, np.linspace(-1.0, 1.0, 11)]
DESCRIPTORS = ("slope", "curvature", "relief")


def read_sweep(path):
    with path.open("rb") as stream:
        side, spacing, count = struct.unpack("<IdI", stream.read(16))
        parameters = np.fromfile(stream, "<f8", count * 3).reshape(count, 3)
        wet = np.fromfile(stream, "<f4", side * side).reshape(side, side) >= 0.5
        heights = np.fromfile(stream, "<f4").reshape(count, side, side)
    return parameters, spacing, wet, heights


def slope_alignment(dx, dz, land, step):
    """Measure continuity of steep-slope axes, regardless of which way they face."""
    magnitude = np.hypot(dx, dz)
    steep = land & (magnitude >= np.tan(np.deg2rad(15)))
    unit = np.stack((dx, dz), axis=-1) / np.maximum(magnitude[..., None], 1e-9)
    near = slice(step, None)
    far = slice(None, -step)
    full = slice(None)
    pairs = []
    for a, b in (((near, full), (far, full)), ((full, near), (full, far)),
                 ((near, near), (far, far)), ((near, far), (far, near))):
        cosine = np.sum(unit[a] * unit[b], axis=-1)
        pairs.append(np.clip(2 * cosine ** 2 - 1, -1, 1)[steep[a] & steep[b]])
    values = np.concatenate(pairs)
    # A window without steep neighbours occupies its own bin.
    return values if values.size else np.array([-2.0])


def histograms(height, land, spacing, spatial=False):
    def histogram(values, bins):
        counts = np.histogram(values, bins=bins)[0]
        if not counts.any():
            raise ValueError("No valid land in the terrain window")
        return counts / counts.sum()

    result = []
    for step in (1, 2, 4):
        size = height.shape[0] - 2 * step
        offsets = ((step, step), (0, step), (2 * step, step), (step, 0), (step, 2 * step))
        supports = [land[y:y + size, x:x + size] for y, x in offsets]
        valid = np.logical_and.reduce(supports)
        center, north, south, west, east = [height[y:y + size, x:x + size] for y, x in offsets]
        distance = 2 * step * spacing
        dx, dz = east - west, south - north
        slope = np.degrees(np.arctan(np.hypot(dx, dz) / distance))
        curvature = center - 0.25 * (north + south + west + east)
        samples = [center, north, south, west, east]
        relief = np.maximum.reduce(samples) - np.minimum.reduce(samples)
        metrics = tuple(histogram(data[valid], bins) for data, bins in (
            (slope, SLOPE_BINS), (curvature, CURVATURE_BINS), (relief, RELIEF_BINS),
        ))
        if spatial:
            metrics += (histogram(slope_alignment(dx / distance, dz / distance, valid, step), ALIGNMENT_BINS),)
        result.append(metrics)
    return result


def reference_histograms(path, side, spacing, source_spacing, spatial=False):
    ratio = spacing / source_spacing
    factor = round(ratio)
    if factor < 1 or not np.isclose(factor, ratio):
        raise ValueError("Reference spacing must divide the sweep spacing")
    height = np.asarray(Image.open(path), dtype=np.float32)
    if height.ndim != 2 or not np.isfinite(height).all():
        raise ValueError(f"Expected a finite single-band elevation raster at {path}")
    rows, cols = (size // factor for size in height.shape)
    if min(rows, cols) < side:
        raise ValueError(f"Reference must cover at least {side * spacing:g} metres on each axis")
    height = height[:rows * factor, :cols * factor].reshape(rows, factor, cols, factor).mean((1, 3))
    ys = sorted({0, (rows - side) // 2, rows - side})
    xs = sorted({0, (cols - side) // 2, cols - side})
    land = np.ones((side, side), dtype=bool)
    windows = [histograms(height[y:y + side, x:x + side], land, spacing, spatial) for y in ys for x in xs]
    return [tuple(np.mean([window[scale][metric] for window in windows], axis=0)
                  for metric in range(len(windows[0][scale])))
            for scale in range(3)]


def scores(heights, wet, spacing, reference, spatial=False):
    return np.array([
        np.mean([[np.sqrt(p * q).sum() for p, q in zip(p_scale, q_scale)]
                 for p_scale, q_scale in zip(histograms(height, ~wet, spacing, spatial), reference)], axis=0)
        for height in heights
    ])


def contact_sheet(path, parameters, heights, wet, spacing, fit, holdout, best):
    width, height = 138, 167
    columns = min(len(parameters), 9)
    rows = (len(parameters) + columns - 1) // columns
    sheet = Image.new("RGB", (columns * width + 28, rows * height + 80), "#142028")
    draw = ImageDraw.Draw(sheet)
    body = ImageFont.truetype("DejaVuSans.ttf", 11) if Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf").exists() else ImageFont.load_default()
    draw.text((14, 12), "AXIOM regional watershed parameter sweep", fill="#f3f7fa", font=body)
    draw.text((14, 33), "Uplift / incision / channel onset area   |   Bhattacharyya fit / holdout", fill="#b4c2cc", font=body)
    draw.text((14, 51), "Same terrain and light across the fit grid. Gold marks the highest fit score.", fill="#b4c2cc", font=body)
    for i, (terrain, factors) in enumerate(zip(heights, parameters)):
        x, y = 14 + i % columns * width, 74 + i // columns * height
        dz, dx = np.gradient(terrain, spacing)
        shade = np.clip((0.66 - 0.52 * dx - 0.49 * dz) / np.sqrt(1 + dx * dx + dz * dz), 0, 1)
        rgb = np.repeat((26 + 224 * shade).astype(np.uint8)[:, :, None], 3, axis=2)
        rgb[wet] = (62, 125, 149)
        sheet.paste(Image.fromarray(rgb).resize((126, 126), Image.Resampling.BICUBIC), (x, y))
        ink = "#ffcb78" if i == best else "#edf3f6"
        if i == best:
            draw.rectangle((x - 2, y - 2, x + 127, y + 127), outline=ink, width=2)
        draw.text((x, y + 129), " / ".join(f"{value:g}" for value in factors), fill=ink, font=body)
        draw.text((x, y + 144), f"{fit[i]:.3f} / {holdout[i]:.3f}", fill=ink, font=body)
    sheet.save(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fit_grid", type=Path)
    parser.add_argument("holdout_grid", type=Path)
    parser.add_argument("fit_reference", type=Path)
    parser.add_argument("holdout_reference", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--reference-spacing", type=float, default=4.0)
    args = parser.parse_args()

    parameters, spacing, wet, heights = read_sweep(args.fit_grid)
    check_parameters, check_spacing, check_wet, check_heights = read_sweep(args.holdout_grid)
    if not np.array_equal(parameters, check_parameters) or spacing != check_spacing or heights.shape != check_heights.shape:
        parser.error("Fit and holdout grids must contain the same parameters and resolution")
    fit_ref = reference_histograms(args.fit_reference, wet.shape[0], spacing, args.reference_spacing, spatial=True)
    holdout_ref = reference_histograms(args.holdout_reference, wet.shape[0], spacing, args.reference_spacing, spatial=True)
    fit_metrics = scores(heights, wet, spacing, fit_ref, spatial=True)
    holdout_metrics = scores(check_heights, check_wet, spacing, holdout_ref, spatial=True)
    # Report spatial continuity separately so historical sweep scores stay comparable.
    fit, holdout = fit_metrics[:, :3].mean(axis=1), holdout_metrics[:, :3].mean(axis=1)
    order = np.argsort(-fit, kind="stable")
    args.output.mkdir(parents=True, exist_ok=True)
    with (args.output / "scores.csv").open("w", newline="", encoding="utf-8") as output:
        writer = csv.writer(output)
        writer.writerow(["uplift", "incision", "onset_area", "fit", "holdout",
                         *(f"{site}_{metric}" for site in ("fit", "holdout")
                           for metric in (*DESCRIPTORS, "slope_alignment"))])
        for i in order:
            writer.writerow([*parameters[i], fit[i], holdout[i], *fit_metrics[i], *holdout_metrics[i]])
    contact_sheet(args.output / "parameter-grid.png", parameters, heights, wet, spacing, fit, holdout, order[0])
    for label, indices in (("best by fit", order[:1]), ("baseline", np.flatnonzero(np.all(parameters == 1.0, axis=1)))):
        for i in indices:
            print(f"{label}: {parameters[i]}  fit={fit[i]:.4f}  holdout={holdout[i]:.4f}")


if __name__ == "__main__":
    main()
