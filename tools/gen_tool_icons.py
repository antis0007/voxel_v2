"""Regenerates the four 16x16 tool/wand sprites with the canonical palette.

Run from the repository root:
    python tools/gen_tool_icons.py

These are source sprites, not screenshots: transparent background, one-pixel
outline, native pixels, and the same top-left light / bottom-right shadow used
by the Rust icon painter.
"""
from pathlib import Path
from PIL import Image
from pixel_palette import load_palette, glyph, top_light, bottom_shadow

P = load_palette()
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets"

ART = {
    "brush": ((101, 183, 175), [
        "................",
        "...........33...",
        "..........3663..",
        ".........3663...",
        "........3663....",
        ".......3553.....",
        "......3553......",
        ".....3553.......",
        "....32223.......",
        "...3222223......",
        "..322222223.....",
        "..3223223223....",
        "...33.33.33.....",
        "................",
        "................",
        "................",
    ]),
    "area_tool": ((101, 183, 175), [
        "................",
        "..353333333353..",
        "..322222222223..",
        "..32........23..",
        "..32.333333.23..",
        "..32.322223.23..",
        "..32.322423.23..",
        "..32.322223.23..",
        "..32.322223.23..",
        "..32.333333.23..",
        "..32........23..",
        "..322222222223..",
        "..353333333353..",
        "................",
        "................",
        "................",
    ]),
    "builders_wand": ((101, 183, 175), [
        "................",
        "..........3333..",
        ".........355553.",
        "........3544453.",
        "........3544453.",
        ".........355553.",
        "..........3333..",
        ".........3223...",
        "........3223....",
        ".......3223.....",
        "......3223......",
        ".....3223.......",
        "....3223........",
        "...3223.........",
        "..3333..........",
        "................",
    ]),
    "destructor_wand": ((193, 102, 49), [
        "................",
        ".......3........",
        "......343.......",
        ".....34343......",
        "....3444443.....",
        "...344444443....",
        "....3444443.....",
        ".....34343......",
        "......343.......",
        ".....3223.......",
        "....3223........",
        "...3223.........",
        "..3223..........",
        "..3333..........",
        "................",
        "................",
    ]),
}


def render(name, accent, rows):
    assert len(rows) == 16 and all(len(row) == 16 for row in rows), name
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    px = img.load()
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            c = glyph(ch, accent, P)
            if c is not None:
                px[x, y] = (*c, 255)
    # Bake the same native-pixel edge light/shadow the UI painter adds.
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch != '2':
                continue
            above = rows[y - 1][x] if y else ' '
            if above not in ('1', '2'):
                px[x, y] = (*top_light(accent, P), 255)
            below = rows[y + 1][x] if y + 1 < len(rows) else ' '
            if below == ' ':
                px[x, y] = (*bottom_shadow(accent, P), 255)
    img.save(OUT / f"{name}.png", optimize=True)


if __name__ == "__main__":
    for name, (accent, rows) in ART.items():
        render(name, accent, rows)
    print("wrote", ", ".join(f"assets/{name}.png" for name in ART))
