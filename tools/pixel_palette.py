"""Shared loader for the canonical pixel-art palette.

Mirrors `assets/pixel_palette.txt` for every Python sprite generator. Keep all
authoring tools on this loader so the Rust UI painter and generated assets use
the same six-role ramp.
"""
from pathlib import Path


def load_palette(root=None):
    root = Path(root) if root else Path(__file__).resolve().parent.parent
    values = {}
    for line in (root / "assets" / "pixel_palette.txt").read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        key, value = key.strip(), value.strip()
        if key in {"shade", "bottom_shade", "top_light_mix"}:
            values[key] = float(value)
        else:
            values[key] = tuple(int(value[i : i + 2], 16) for i in (0, 2, 4))
    return values


def glyph(ch, accent, palette):
    """Return the canonical RGB for a sprite glyph."""
    if ch == "1":
        return tuple(int(v * palette["shade"]) for v in accent)
    if ch == "2":
        return accent
    if ch == "3":
        return palette["outline"]
    if ch == "4":
        return palette["highlight"]
    if ch == "5":
        return palette["gold"]
    if ch == "6":
        return palette["ember"]
    return None


def top_light(accent, palette):
    t = palette["top_light_mix"]
    paper = palette["paper"]
    return tuple(int(a * (1.0 - t) + b * t) for a, b in zip(accent, paper))


def bottom_shadow(accent, palette):
    return tuple(int(v * palette["bottom_shade"]) for v in accent)
