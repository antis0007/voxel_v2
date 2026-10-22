"""Generates crates/app/src/egui_ui/icons_spells.rs and a preview sheet for the spell icons.
Half-art entries are mirrored left-to-right. '.' is empty. Glyphs use the shared
six-role palette from assets/pixel_palette.txt: 1 shade, 2 body, 3 outline,
4 highlight, 5 gold, 6 orange/brown. Runtime painting and this preview apply the
same one-pixel top-light and bottom-shadow rule."""
import sys, os
from pixel_palette import load_palette, glyph, top_light, bottom_shadow

PALETTE = load_palette()
S = {}  # id -> (name, colour rgb, rows, mirrored)
def sym(i, name, rgb, rows): S[i] = (name, rgb, rows, True)
def full(i, name, rgb, rows): S[i] = (name, rgb, rows, False)

sym(3, "Resonant burst", (196,112,255), ["...333",".33222",".32333","323244","323244",".32333",".33222","...333"])
sym(24, "Timberfall", (184,219,87), ["...333","..3222",".32222","322222",".32222","..3222","...322","....36","....36","...333"])
sym(27, "Siphon", (161,255,98), [".....3","....32","....32","...322","..3224",".32224",".32222",".32222","..3222","...333"])
sym(35, "Growth", (110,230,110), ["..33..",".3223.",".32223","..3223","....32","....32","...333"])
sym(37, "Widen", (120,200,255), ["..3...",".32...","322222",".32222","..3..."])
sym(38, "Seek ally", (255,150,170), [".....3",".....2","..3333",".32...",".3....","32..44",".3....",".32...","..3333",".....2",".....3"])
sym(39, "Shrink", (255,200,110), ["3.....","323...","32223.","323...","3....."])
sym(40, "Divide by 2", (200,200,230), ["33333.","32223.","32223.","33333."])
sym(41, "Divide by 4", (200,200,230), ["33333.","32223.","32223.","33333.","......","33333.","32223.","32223.","33333."])
sym(44, "Orbiting arc", (140,170,255), ["..3333",".32222","322...","32..44","32..44","322...",".32222","..3333"])
sym(36, "Conjure lanterncap", (255,170,90), ["..3333",".32222","322422","322222","333333","...322","...322","..3333"])
sym(45, "Blood siphon", (230,60,80), [".33...","3223..","322223","322222",".32222","..3222","...322","....33"])
sym(47, "If wounded", (255,110,110), ["32....",".322..","..322.","...322","...322","..322.",".322..","32...."])
sym(48, "If foe", (255,190,90), ["...333",".33222","322233","322344","322233",".33222","...333"])
sym(51, "Light shot", (255,236,150), [".....3","....32","...324","332244","332244","...324","....32",".....3"])
sym(52, "Long life", (120,230,190), ["333333",".32222","..3222","...322","....32","...322","..3222",".32222","333333"])
sym(53, "Piercing", (190,210,235), [".....3","....32","...324","..3224",".32224","....32","....32","....32","....32","...333"])
sym(54, "Accelerate", (255,240,120), [".....3","....32","...32.","..32..",".....3","....32","...32.","..32.."])
sym(55, "Damage plus", (255,120,90), ["...333","...322","...322","333322","322222","322222","333322","...322","...322","...333"])
sym(25, "Gale", (170,230,235), ["333333","322222",".33333","..3222","...333","...322","....33","....32",".....3"])
full(26, "Volatile edge", (255,150,80), ["        33  ","       3243 ","      32243 ","     32243  ","    32243   "," 3 32243    ","  33243     ","  3323      "," 3  33      ","3 3         "])
full(34, "Touch", (255,215,170), [
"    33      ","   3223     ","   3223     ","   3223 33  "," 33322232 3 ","3222222223  ","3222222223  "," 322222223  ","  3222223   ","   33333    "])
sym(42, "Curve", (140,255,200), ["..3333",".32222","32233.","3223..","3223..","3333.."])
full(43, "Wiggle", (140,230,255), ["   333333   ","  32222223  ","  3223333   ","   322223   ","    333223  ","  32222223  ","  3333333   "])
full(46, "Unstable", (255,120,230), [
"      3333  ","     32223  ","    32223   ","   322223   ","  3222233   ","    32223   ","   32223    ","  3223      "," 323        "," 33         "])
full(49, "Alpha", (255,235,150), [
"    3333    ","   322223   ","  32233223  ","  32233223  ","  32222223  "," 3222222223 "," 3223333223 "," 333    333 "])
full(50, "Chaotic path", (200,120,255), [
"33333333    ","32222223    ","33333223    ","    32233   ","   32233    ","  32233333  ","  32222223  ","  33333333  "])

sym(56, "Ignite", (255,115,30), [".....3","....32","...322","..3224",".32255",".32255",".32224","..3222","...333"])
full(57, "Static charge", (190,215,255), ["      333   ","     3243   ","    3243    ","   32243    ","  3222233   ","   3322243  ","     3243   ","    3243    ","   3243     ","   333      "])
sym(58, "Venom coat", (140,230,70), ["..3333","...322","...322","..3222",".32222","322244","322222","333333"])
sym(59, "Drench", (80,150,255), ["..3333",".32222","322222","322222","333333","..3..3","..2..2"])
sym(60, "Cold snap", (150,225,255), [".....3","3....2",".3...2","..3.32","...322","322222","322222","...322","..3.32",".3...2","3....2",".....3"])
sym(61, "Chain arc", (210,210,255), [".3333.","322223","32..23","32..23","322223",".3333."])
sym(62, "Shrapnel", (255,180,70), [".....3","3...32",".3.322","..3222","322244","..3222",".3.322","3...32",".....3"])
sym(63, "Overcharge", (255,90,150), ["..3333",".32222","322422","322422","322222","322422","322222","333333"])

sym(28, "Ember seed", (230,120,60), ["....33","...322","..3222",".32225",".32255",".32255",".32224","..3222","...333"])
sym(29, "Meteor", (240,100,40), ["....55","...556","..3222",".32244",".32222",".32222","..3222","...333"])
sym(30, "Time snag", (190,225,235), ["333333",".32222","..3222","...322","....33","...322","..3222",".32244","333333"])
sym(31, "Grapple", (240,225,200), [".....3",".....2",".....2",".....2","3....2","32...2",".322.2","..3222","...333"])
sym(32, "Leap", (160,200,100), [".....3","....32","...322","..3222",".32222","333322","...322","...322","...322","...333"])
full(64, "Reverse", (160,110,160), ["    33      ","   323      ","  3223333   "," 322222223  ","  3223333   ","   323      ","    33      "])
sym(65, "Gravity", (125,92,66), ["...322","...322","...322","333322",".32222","..3222","...322","....33"])
sym(66, "Phase", (140,190,210), ["33.333","32.322","32.322","32.322","32.322","33.333"])
sym(67, "Reckless", (220,107,92), ["...333","..3222",".32222","322244","322222",".32233","..3222","...333"])
sym(68, "Blood price", (184,41,56), [".....3","....32","...322","..3222",".32244",".32222","..3222","...333"])
sym(69, "Fork", (230,184,107), ["32....",".32...","..32..","...322","....22","....22","....33"])
sym(71, "Chainsaw", (215,215,225), ["3333333","3222222","3.3.3.3","3.3.3.3"])
sym(72, "Teleport bolt", (190,140,255), ["..3333",".32222","32.44.","32.44.",".32222","..3333"])
sym(73, "Flood bolt", (90,150,255), ["..3...","..32..",".322..","32222.","322244","322222",".33333","3.3.3."])
sym(74, "Venom nova", (115,215,75), ["3...3.","32.32.",".3223.","..322.","322222","..322.",".3223.","32.32.","3...3."])
sym(75, "Ice wall", (165,230,255), ["333333","322232","322232","333333","232222","232222","333333"])
sym(76, "Bounce orb", (245,165,215), ["..3333",".32222","322442","322442","322222",".32222","..3333"])
sym(77, "Mana leech", (140,189,209), ["...333","..3222",".32244","322222","322222",".32222","..3222","...333"])
sym(78, "Critical plus", (220,107,92), ["....3.","...32.","333322","322224","333322","...32.","....3."])
sym(79, "On expire", (140,92,140), ["..3333","..3222","...322","..3225","..3225",".32222","333333"])
sym(70, "On timer", (140,190,210), ["..3333",".32222","322232","322322","322332",".32222","..3333"])

def expand(i):
    name, rgb, rows, mir = S[i]
    out = []
    for r in rows:
        r = r.replace('.', ' ')
        out.append(r + r[::-1] if mir else r)
    return out

def validate():
    for i, (name, _, _, _) in S.items():
        rows = expand(i)
        solid = sum(1 for row in rows for ch in row if ch != ' ')
        assert solid >= 12, f"{name} ({i}) too sparse"
        assert len(rows) <= 12 and all(len(row) <= 14 for row in rows), f"{name} ({i}) oversized"
        assert all(ch in ' 123456' for row in rows for ch in row), f"{name} ({i}) bad glyph"
        assert any('3' in row for row in rows), f"{name} ({i}) has no outline"
        assert any('2' in row for row in rows), f"{name} ({i}) has no body role"

def rust():
    validate()
    o = ["//! Generated by tools/gen_spell_icons.py. Do not edit by hand.",
         "use egui::Color32 as C;", "",
         "/// Pixel art for spells without a bespoke illustration in icons.rs.",
         "pub fn art(id: u32) -> Option<(&'static [&'static str], C)> {", "    Some(match id {"]
    for i in sorted(S):
        name, rgb, _, _ = S[i]
        o.append(f"        {i} => (")
        o.append("            &[")
        for r in expand(i):
            o.append(f'                "{r}",')
        o.append("            ],")
        o.append(f"            C::from_rgb({rgb[0]}, {rgb[1]}, {rgb[2]}),")
        o.append("        ),")
    o += ["        _ => return None,", "    })", "}", ""]
    o += ["#[cfg(test)]", "mod tests {", "    use super::art;", "",
          "    #[test]", "    fn every_icon_is_readable() {",
          "        for id in 0..=crate::world::adventure::MAX_SPELL_ID {",
          "            let Some((rows, _)) = art(id) else { continue };",
          "            let solid: usize = rows.iter().map(|r| r.bytes().filter(|b| *b != b' ').count()).sum();",
          "            assert!(solid >= 12, \"spell {id} too sparse\");",
          "            assert!(rows.len() <= 12, \"spell {id} too tall\");",
          "            assert!(rows.iter().all(|r| r.len() <= 14), \"spell {id} too wide\");",
          "            assert!(rows.iter().any(|r| r.contains('3')), \"spell {id} lacks an outline\");",
          "            assert!(rows.iter().any(|r| r.contains('2')), \"spell {id} lacks a body\");",
          "            assert!(rows.iter().all(|r| r.bytes().all(|b| b\" 123456\".contains(&b))), \"spell {id} bad glyph\");",
          "        }", "    }", "}", ""]
    return "\n".join(o)

def sheet(path):
    from PIL import Image
    cell, scale = 16, 8
    ids = sorted(S)
    cols = 7
    rows_n = (len(ids) + cols - 1) // cols
    img = Image.new("RGB", (cols * cell * scale, rows_n * cell * scale), (20, 22, 26))
    px = img.load()
    for n, i in enumerate(ids):
        _, rgb, _, _ = S[i]
        ox, oy = (n % cols) * cell * scale, (n // cols) * cell * scale
        art = expand(i)
        for y, r in enumerate(art):
            for x, ch in enumerate(r):
                c = glyph(ch, rgb, PALETTE)
                if c is None:
                    continue
                for dy in range(scale):
                    for dx in range(scale):
                        px[ox + (x + 1) * scale + dx, oy + (y + 2) * scale + dy] = c
                if ch != '2':
                    continue
                above = art[y - 1][x] if y else ' '
                if above not in ('1', '2'):
                    c = top_light(rgb, PALETTE)
                    for dx in range(scale):
                        px[ox + (x + 1) * scale + dx, oy + (y + 2) * scale] = c
                below = art[y + 1][x] if y + 1 < len(art) else ' '
                if below not in ('1', '2'):
                    c = bottom_shadow(rgb, PALETTE)
                    for dx in range(scale):
                        px[ox + (x + 1) * scale + dx, oy + (y + 3) * scale - 1] = c
    img.save(path)

if __name__ == "__main__":
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    open(os.path.join(root, "crates/app/src/egui_ui/icons_spells.rs"), "w", newline="\n").write(rust())
    if len(sys.argv) > 1: sheet(sys.argv[1])
