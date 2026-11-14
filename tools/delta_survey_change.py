"""Compare co-registered, metre-based USGS ASCII elevation surveys.

Restrict the comparison to common 8-by-8 blocks so every resolution uses the
same measured area. Average signed bed change before counting erosion or gain.
"""

import argparse
from array import array
from pathlib import Path


def read_grid(path):
    with path.open() as stream:
        header = dict(line.lower().split() for line in (next(stream) for _ in range(6)))
        cols, rows = (int(header[key]) for key in ("ncols", "nrows"))
        data = array("f", (float(value) for line in stream for value in line.split()))
    if len(data) != cols * rows:
        raise ValueError(f"unexpected sample count in {path}")
    return header, cols, rows, data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("earlier", type=Path)
    parser.add_argument("later", type=Path)
    args = parser.parse_args()
    header, cols, rows, earlier = read_grid(args.earlier)
    later_header, _, _, later = read_grid(args.later)
    nodata = float(header.pop("nodata_value"))
    later_nodata = float(later_header.pop("nodata_value"))
    if header != later_header or float(header["cellsize"]) != 1.0:
        parser.error("both surveys must use the same aligned 1 m grid")

    totals = {size: [0.0, 0.0] for size in (1, 2, 4, 8)}
    area = 0
    for y in range(0, rows - 7, 8):
        for x in range(0, cols - 7, 8):
            indices = [(y + row) * cols + x + col for row in range(8) for col in range(8)]
            if any(earlier[i] == nodata or later[i] == later_nodata for i in indices):
                continue
            area += 64
            changes = [later[i] - earlier[i] for i in indices]
            for size, volumes in totals.items():
                for row in range(0, 8, size):
                    for col in range(0, 8, size):
                        volume = sum(changes[(row + dy) * 8 + col + dx]
                                     for dy in range(size) for dx in range(size))
                        volumes[0] += max(0.0, volume)
                        volumes[1] += max(0.0, -volume)
    if not area:
        parser.error("the surveys have no complete common blocks")

    print(f"Common surveyed area: {area:,} m²")
    for size, (gain, erosion) in totals.items():
        print(f"{size} m grid: gain={gain:,.0f} m³ erosion={erosion:,.0f} m³ "
              f"net={gain - erosion:+,.0f} m³")


if __name__ == "__main__":
    main()
