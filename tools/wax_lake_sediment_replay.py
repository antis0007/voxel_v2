"""Fit bed stress on the first half of the open 2014 Wax Lake record.

Compare predictions on the later half and match published SSC by timestamp.
Input files and units are documented at https://doi.org/10.17632/sb4xnz5ncv.2.
"""

import argparse
import csv
import math
from datetime import datetime
from pathlib import Path
from statistics import fmean


def read(path, columns):
    with path.open(newline="", encoding="utf-8-sig") as stream:
        for row in csv.DictReader(stream):
            values = tuple(float(row[column]) for column in columns)
            if all(map(math.isfinite, values)):
                yield datetime.strptime(row["Time"], "%d-%b-%Y %H:%M:%S"), values


def fit_stress(samples):
    xx = sum(x * x for x, _, _ in samples)
    xy = sum(x * y for x, y, _ in samples)
    yy = sum(y * y for _, y, _ in samples)
    xt = sum(x * stress for x, _, stress in samples)
    yt = sum(y * stress for _, y, stress in samples)
    determinant = xx * yy - xy * xy
    current = xt / xx
    with_waves = ((xt * yy - yt * xy) / determinant, (yt * xx - xt * xy) / determinant)
    if min(with_waves) < 0:
        # The nonnegative two-variable optimum then lies on one of the axes.
        with_waves = min(
            [(current, 0.0), (0.0, yt / yy)],
            key=lambda weights: sum((weights[0] * x + weights[1] * y - stress) ** 2
                                    for x, y, stress in samples),
        )
    return (current, 0.0), with_waves


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("data", type=Path, help="directory containing the two published CSV files")
    args = parser.parse_args()
    observations = {}
    for date, (depth_cm, wave_cm, east_cms, north_cms, shear_cms) in read(
        args.data / "water_level_wave_velocity_ustar.csv", ("depth", "hs", "u", "v", "ustar"),
    ):
        depth = depth_cm / 100
        if depth <= 0 or wave_cm < 0 or shear_cms < 0:
            continue
        current_squared = (east_cms**2 + north_cms**2) / 10_000
        amplitude = min(wave_cm / 200, 0.39 * depth)
        wave_squared = 9.81 * amplitude**2 / depth
        # Approximate water density. The source estimates shear velocity from the measurements.
        stress = 1000 * (shear_cms / 100) ** 2
        observations[date] = current_squared, wave_squared, stress

    dates = sorted(observations)
    if len(dates) < 6:
        parser.error("too few valid stress observations")
    split = len(dates) // 2
    calibration = [observations[date] for date in dates[:split]]
    evaluation = [observations[date] for date in dates[split:]]
    print(f"Calibrated on {len(calibration)} observations through {dates[split - 1]}")
    print(f"Held out {len(evaluation)} observations starting {dates[split]}")
    for label, (current, waves) in zip(("current only", "current and waves"), fit_stress(calibration)):
        errors = [current * x + waves * y - stress for x, y, stress in evaluation]
        print(f"{label}: coefficients {current:.4f}, {waves:.4f}, "
              f"MAE {fmean(map(abs, errors)):.5f} Pa, bias {fmean(errors):+.5f} Pa")

    groups = ([], [], [])
    for date, (concentration,) in read(args.data / "turbidity_ssc.csv", ("SSC_calib",)):
        if date in observations and date >= dates[split] and concentration >= 0:
            stress = observations[date][2]
            index = 0 if stress < 0.01 else 1 if stress < 0.02 else 2
            groups[index].append(concentration)
    for label, values in zip(("<0.01", "0.01–0.02", ">=0.02"), groups):
        if values:
            print(f"Stress {label} Pa: measured SSC mean {fmean(values):.2f} mg/L, n={len(values)}")


if __name__ == "__main__":
    main()
