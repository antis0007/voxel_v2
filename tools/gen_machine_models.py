"""Write authored voxel casings for single-block factory machines.

Each machine is one solid block of 10 x 10 x 10 voxels at 0.1 block units per
voxel, which is two texels of the 20 texel/m world grid per voxel. The front
faces +Z. Output uses the vertex-list format read by `wand_asset::Mesh::load`;
the Rust side owns no shapes.

Run from the repository root:

    python -I tools/gen_machine_models.py

Palette roles mirror `assets/pixel_palette.txt`: frame and steel are the
shade and body roles, trim is the family accent, ink is the status colour
(tint 1, multiplied by the live machine state at draw time).
"""

from pathlib import Path

N = 10
PIXEL = 0.1
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "models" / "machines"

FRAME = (0.29, 0.34, 0.37)
STEEL = (0.416, 0.451, 0.482)
BOLT = (0.541, 0.573, 0.604)
INSET = (0.04, 0.045, 0.05)
INK = (1.0, 1.0, 1.0)

# Palette key -> (colour, tint). Tint 1 takes the live status colour at draw time,
# tint 2 the family trim colour. The trim key is added per machine family; the
# "trim" entries below only colour previews unless the machine bakes them.
PALETTE = {
    "f": (FRAME, 0),
    "s": (STEEL, 0),
    "b": (BOLT, 0),
    "i": (INSET, 0),
    "g": (INK, 1),
}

# Lit cells of a 5 x 5 glyph as (column, row) from the top left. Matches
# `machine_casing::pixels` in crates/app/src/mesh/models/machine_casing.rs.
FLAME = [(2, 0), (2, 1), (1, 2), (2, 2), (3, 2), (0, 3), (1, 3), (2, 3), (3, 3),
         (4, 3), (1, 4), (2, 4), (3, 4)]

COPPER = (0.72, 0.45, 0.25)


def boundary(v):
    return v == 0 or v == N - 1


def block(glyph):
    """Return {(x, y, z): key} for one closed casing; the front is +Z."""
    lit = {(2 + col, 7 - row) for col, row in glyph}
    grid = {}
    for x in range(N):
        for y in range(N):
            for z in range(N):
                edges = sum(boundary(v) for v in (x, y, z))
                c = "f" if edges >= 2 else "s"
                if y == 0:
                    c = "f"
                elif y == N - 1 and (boundary(x) or boundary(z)):
                    c = "t"
                elif y == N - 1 and z in (3, 6) and 3 <= x <= 6:
                    c = "i"
                if z == N - 1 and 1 <= y <= N - 2:
                    if (x in (1, 7) and 2 <= y <= 8) or (y in (2, 8) and 1 <= x <= 7):
                        c = "t"
                    if 2 <= x <= 6 and 3 <= y <= 7:
                        c = "i"
                    if (x, y) in lit:
                        c = "g"
                    if (x, y) in ((1, 1), (8, 1)):
                        c = "b"
                if z == 0 and 3 <= x <= 6 and y in (3, 5, 7):
                    c = "i"
                if x in (0, N - 1) and 1 <= y <= 8 and 1 <= z <= 8:
                    ring = (3 <= y <= 6 and z in (3, 6)) or (3 <= z <= 6 and y in (3, 6))
                    bore = 4 <= y <= 5 and 4 <= z <= 5
                    if ring:
                        c = "f"
                    if bore:
                        c = "i"
                grid[(x, y, z)] = c
    return grid


# Shaped machines. A grid holds only solid voxels; a missing cell is air, so
# these blocks have real silhouettes. Each builder returns its animation
# frames: frame 0 is the idle pose, frame 1 the alternate stepped pose.


def fill(grid, xs, ys, zs, key):
    for x in xs:
        for y in ys:
            for z in zs:
                if 0 <= x < N and 0 <= y < N and 0 <= z < N:
                    grid[(x, y, z)] = key


def carve(grid, xs, ys, zs):
    for x in xs:
        for y in ys:
            for z in zs:
                grid.pop((x, y, z), None)


def span(a, b):
    """Inclusive integer range."""
    return range(a, b + 1)


def plinth(grid, height=1):
    """Dark base slab over the whole footprint with corner bolts."""
    fill(grid, span(0, 9), span(0, height - 1), span(0, 9), "f")
    for x, z in ((0, 9), (9, 9)):
        grid[(x, height - 1, z)] = "b"


def crusher(frame):
    """Jaw crusher: an open hopper over toothed jaws, flywheels on both sides
    and a discharge chute at the front foot. The moving jaw and the flywheel
    spokes step between the frames."""
    g = {}
    plinth(g)
    fill(g, span(1, 8), span(1, 6), span(1, 8), "s")
    fill(g, span(1, 8), [6], span(1, 8), "t")
    # Hopper: flares from the body to the full footprint, open on top.
    for y, lo in ((7, 1), (8, 0), (9, 0)):
        hi = 9 - lo
        fill(g, span(lo, hi), [y], span(lo, hi), "t" if y == 9 else "f")
        carve(g, span(lo + 1, hi - 1), [y], span(lo + 1, hi - 1))
    carve(g, span(2, 7), [6], span(2, 7))
    fill(g, span(2, 7), [5], span(2, 7), "i")
    # Fixed jaw on the back wall, moving jaw swinging toward it.
    fill(g, span(2, 7), span(6, 8), [2], "s")
    for x in (2, 4, 6):
        fill(g, [x], span(6, 7), [3], "b")
    jaw = 6 if frame == 0 else 5
    fill(g, span(2, 7), span(6, 8), [jaw], "s")
    for x in (3, 5, 7):
        fill(g, [x], span(6, 7), [jaw - 1], "b")
    # Discharge chute under the front panel.
    carve(g, span(3, 6), span(1, 2), [8])
    fill(g, span(3, 6), span(1, 2), [7], "i")
    fill(g, span(2, 7), [3], [8], "f")
    # Status lamp on the front panel.
    fill(g, span(5, 7), [5], [8], "g")
    # Flywheels: discs on both side faces with a stepped spoke pattern.
    for x in (0, 9):
        for y in span(1, 8):
            for z in span(1, 8):
                dy, dz = y - 4.5, z - 4.5
                r2 = dy * dy + dz * dz
                if r2 > 13.0:
                    continue
                if r2 > 7.0:
                    key = "f"
                elif r2 < 1.0:
                    key = "b"
                elif frame == 0 and (abs(dy) < 1.0 or abs(dz) < 1.0):
                    key = "s"
                elif frame == 1 and (abs(dy - dz) < 1.0 or abs(dy + dz) < 1.0):
                    key = "s"
                else:
                    continue
                g[(x, y, z)] = key
    return g


def generator(frame):
    """Combustion generator: a finned firebox with a barred grate on the
    front, a dynamo drum lying across the top and an exhaust stack behind.
    The grate glow steps between the frames."""
    g = {}
    plinth(g)
    fill(g, span(0, 9), span(1, 6), span(1, 9), "s")
    fill(g, span(0, 9), [6], span(1, 9), "f")
    fill(g, span(0, 9), [1], [9], "t")
    # Cooling fins: every other column of the side walls is cut back.
    for x in (0, 9):
        for z in span(2, 8):
            if z % 2 == 1:
                carve(g, [x], span(2, 5), [z])
    # Grate: bars over a lit firebox.
    carve(g, span(2, 7), span(2, 4), [9])
    fill(g, span(2, 7), span(2, 4), [8], "g")
    bars = (2, 4, 6) if frame == 0 else (3, 5, 7)
    for x in bars:
        fill(g, [x], span(2, 4), [9], "f")
    fill(g, span(1, 8), [5], [9], "t")
    # Dynamo drum across the top, banded at both ends with a copper winding.
    for y in span(7, 9):
        for z in span(3, 7):
            dy, dz = y - 6.5, z - 5.0
            if dy * dy + dz * dz <= 7.0:
                fill(g, span(1, 8), [y], [z], "s")
                fill(g, [1, 8], [y], [z], "t")
                fill(g, span(3, 6), [y], [z], "c")
    # Exhaust stack in the back corner, open at the top.
    fill(g, span(7, 8), span(7, 9), span(0, 1), "f")
    g[(7, 9, 0)] = "i"
    g[(8, 9, 1)] = "i"
    return g


def pump(frame):
    """Centrifugal pump: a round volute on a plinth, flanged ports on both
    sides and a finned motor on top. The motor fins step between frames."""
    import math

    g = {}
    plinth(g, 2)
    fill(g, span(0, 9), [1], [9], "t")
    fill(g, span(3, 6), [1], [9], "g")
    for y in span(2, 9):
        for x in range(N):
            for z in range(N):
                dx, dz = x - 4.5, z - 4.5
                r2 = dx * dx + dz * dz
                if y <= 4:
                    if r2 <= 17.0:
                        g[(x, y, z)] = "f" if y == 4 else "s"
                elif y <= 6:
                    if r2 <= 9.0:
                        g[(x, y, z)] = "t" if y == 6 else "s"
                else:
                    if r2 <= 6.5:
                        angle = math.atan2(dz, dx)
                        step = int(math.floor((angle / (2 * math.pi)) * 8 + 0.5 * frame)) % 2
                        key = "b" if y == 9 and r2 < 1.0 else ("f" if step == 0 else "s")
                        if y == 9 and r2 > 2.5:
                            key = "f"
                        g[(x, y, z)] = key
    # Flanged inlet and outlet ports on the side faces.
    for x in (0, 9):
        fill(g, [x], span(2, 4), span(3, 6), "f")
        fill(g, [x], [3], span(4, 5), "i")
    return g


def press(frame):
    """Hydraulic press: a bed and work plate, two side columns, a crown beam
    with the cylinder, and a platen that lowers in the second frame. The
    front is open between the columns."""
    g = {}
    fill(g, span(0, 9), span(0, 1), span(0, 9), "f")
    fill(g, span(0, 9), [2], span(1, 8), "s")
    fill(g, span(0, 9), [1], [9], "t")
    fill(g, span(2, 7), [2], span(2, 7), "b")
    fill(g, span(3, 6), [2], span(3, 6), "i")
    for x0 in (0, 8):
        fill(g, span(x0, x0 + 1), span(3, 7), span(3, 6), "s")
        fill(g, span(x0, x0 + 1), span(3, 7), [6], "f")
        fill(g, span(x0, x0 + 1), [6], [6], "g")
    fill(g, span(0, 9), span(8, 9), span(2, 7), "f")
    fill(g, span(0, 9), [9], [2, 7], "t")
    fill(g, span(3, 6), [9], span(3, 6), "s")
    fill(g, span(4, 5), [9], span(4, 5), "b")
    platen = 5 if frame == 0 else 4
    fill(g, span(4, 5), span(platen + 1, 7), span(4, 5), "b")
    fill(g, span(2, 7), [platen], span(2, 7), "s")
    fill(g, span(2, 7), [platen], [7], "t")
    return g


MACHINES = {
    "furnace": {"trim": (0.55, 0.39, 0.19), "baked": True, "frames": lambda: [block(FLAME)]},
    "crusher": {"trim": (0.30, 0.55, 0.95), "frames": lambda: [crusher(0), crusher(1)]},
    "generator": {"trim": (0.95, 0.42, 0.18), "frames": lambda: [generator(0), generator(1)]},
    "pump": {"trim": (0.25, 0.70, 0.85), "frames": lambda: [pump(0), pump(1)]},
    "fabricator": {"trim": (0.80, 0.45, 0.90), "frames": lambda: [press(0), press(1)]},
}


def faces(grid):
    """Greedy-merged quads on every solid face open to air: (key, axis, sign, corners).

    Corners are integer voxel-corner coordinates, counter-clockwise about the
    outward normal after `triangles` winds them.
    """
    out = []
    for axis in range(3):
        u, v = (axis + 1) % 3, (axis + 2) % 3
        for sign in (1, -1):
            for d in range(N):
                mask = {}
                for a in range(N):
                    for b in range(N):
                        p = [0, 0, 0]
                        p[axis], p[u], p[v] = d, a, b
                        q = list(p)
                        q[axis] += sign
                        if tuple(p) not in grid or tuple(q) in grid:
                            continue
                        mask[(a, b)] = grid[tuple(p)]
                plane = d + (1 if sign > 0 else 0)
                while mask:
                    a, b = min(mask)
                    c = mask[(a, b)]
                    w = 1
                    while mask.get((a + w, b)) == c:
                        w += 1
                    h = 1
                    while all(mask.get((a + k, b + h)) == c for k in range(w)):
                        h += 1
                    for k in range(w):
                        for j in range(h):
                            del mask[(a + k, b + j)]
                    corners = []
                    for da, db in ((0, 0), (w, 0), (w, h), (0, h)):
                        p = [0, 0, 0]
                        p[axis], p[u], p[v] = plane, a + da, b + db
                        corners.append(tuple(p))
                    out.append((c, axis, sign, corners))
    return out


def triangles(axis, sign, corners):
    """Two triangles wound counter-clockwise about the outward normal."""
    normal = [0.0, 0.0, 0.0]
    normal[axis] = float(sign)
    fixed = []
    for t in ((corners[0], corners[1], corners[2]), (corners[0], corners[2], corners[3])):
        e1 = [t[1][i] - t[0][i] for i in range(3)]
        e2 = [t[2][i] - t[0][i] for i in range(3)]
        cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ]
        if sum(cross[i] * normal[i] for i in range(3)) < 0:
            t = (t[0], t[2], t[1])
        fixed.append(t)
    return fixed


def vertex_line(pos, normal, color, tint):
    p = ", ".join(f"{v * PIXEL:.4f}" for v in pos)
    n = ", ".join(f"{v:.1f}" for v in normal)
    c = ", ".join(f"{v:.3f}" for v in color)
    return (
        f"    (p:({p}), n:({n}), c:({c}), uv:(0.0, 0.0), kind:41.0, finish:0.00, "
        f"tint:{tint}, bone:0, weight:0.00),"
    )


def ron_text(name, trim, grid):
    """`trim` is a baked colour, or None for a white trim on tint 2 that takes
    the live family colour (it changes with the machine mode) at draw time."""
    palette = dict(PALETTE)
    palette["t"] = (trim, 0) if trim is not None else (INK, 2)
    palette["c"] = (COPPER, 0)
    lines = []
    for key, axis, sign, corners in faces(grid):
        colour, tint = palette[key]
        normal = [0.0, 0.0, 0.0]
        normal[axis] = float(sign)
        for tri in triangles(axis, sign, corners):
            for pos in tri:
                lines.append(vertex_line(pos, normal, colour, tint))
    header = [
        f"// Generated by tools/gen_machine_models.py ({name}). Edit the generator.",
        "// One solid block in unit coordinates (0 to 1), 10 voxels per metre, front +Z.",
        "(vertices: [",
    ]
    return "\n".join(header + lines + ["])", ""])


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name, spec in MACHINES.items():
        for index, grid in enumerate(spec["frames"]()):
            assert grid and all(0 <= v < N for cell in grid for v in cell)
            label = name if index == 0 else f"{name}_{index}"
            path = OUT / f"{label}.ron"
            text = ron_text(label, spec["trim"] if spec.get("baked") else None, grid)
            path.write_text(text, encoding="utf-8")
            print(f"wrote {path.relative_to(ROOT)} vertices={text.count('(p:')}")


if __name__ == "__main__":
    main()
