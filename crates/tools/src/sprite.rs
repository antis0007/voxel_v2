//! Composes 16x16 tool sprites from part masks and material ramps.
//!
//! Every tool of a schema shares one silhouette, so a gold pickaxe and a flint
//! pickaxe read as the same tool. Masks, ramps and layer order live in
//! `assets/tools/sprites16.txt`.
use std::collections::HashMap;
use std::sync::LazyLock;

pub const SIZE: usize = 16;
pub const SPRITES_TXT: &str = include_str!("../../../assets/tools/sprites16.txt");
const OUTLINE: [u8; 3] = [26, 18, 14];

/// One sprite texel: shade index (0 outline, 1 dark to 4 light) and colour.
pub type Texel = Option<(u8, [u8; 3])>;

#[derive(Default)]
pub struct Sheet {
    masks: HashMap<String, Vec<u8>>,
    palettes: HashMap<String, [[u8; 3]; 4]>,
    tools: HashMap<String, Vec<(String, usize)>>,
}

fn hex(s: &str) -> Option<[u8; 3]> {
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let mut sheet = Sheet::default();
    let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.starts_with("//"));
    while let Some((n, line)) = lines.next() {
        let err = |what: &str| format!("sprites16.txt:{}: {what}", n + 1);
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["mask", name] => {
                let mut cells = Vec::with_capacity(SIZE * SIZE);
                for _ in 0..SIZE {
                    let (_, row) = lines.next().ok_or_else(|| err("mask ends early"))?;
                    if row.len() != SIZE {
                        return Err(err("mask rows are 16 cells"));
                    }
                    for c in row.bytes() {
                        cells.push(match c {
                            b'.' => 0,
                            b'1'..=b'4' => c - b'0',
                            _ => return Err(err("mask cells are . or 1-4")),
                        });
                    }
                }
                if lines.next().map(|(_, l)| l.trim()) != Some("end") {
                    return Err(err("mask needs end"));
                }
                sheet.masks.insert((*name).into(), cells);
            }
            ["palette", material, ramp @ ..] if ramp.len() == 4 => {
                let mut colours = [[0; 3]; 4];
                for (slot, h) in colours.iter_mut().zip(ramp) {
                    *slot = hex(h).ok_or_else(|| err("palette colours are rrggbb"))?;
                }
                sheet.palettes.insert((*material).into(), colours);
            }
            ["tool", schema, layers @ ..] => {
                let mut parsed = Vec::new();
                for layer in layers {
                    let (mask, slot) = layer.split_once(':').ok_or_else(|| err("layers are mask:slot"))?;
                    parsed.push((mask.into(), slot.parse().map_err(|_| err("slot is a number"))?));
                }
                sheet.tools.insert((*schema).into(), parsed);
            }
            _ => return Err(err("unknown line")),
        }
    }
    Ok(sheet)
}

pub static SHEET: LazyLock<Sheet> = LazyLock::new(|| parse(SPRITES_TXT).unwrap_or_default());

impl Sheet {
    pub fn has_tool(&self, schema: &str) -> bool {
        self.tools.contains_key(schema)
    }

    /// Four-step colour ramp of a material, dark to light; unknown materials use stone.
    pub fn ramp(&self, material: &str) -> Option<[[u8; 3]; 4]> {
        self.palettes.get(material).or_else(|| self.palettes.get("stone")).copied()
    }

    /// Renders `schema` with one material per part slot; unknown materials fall back to stone.
    pub fn render(&self, schema: &str, materials: &[&str]) -> Option<Vec<Texel>> {
        let px = self.paint_layers(schema, materials, true)?;
        let filled = |x: i32, y: i32| (0..SIZE as i32).contains(&x) && (0..SIZE as i32).contains(&y) && px[y as usize * SIZE + x as usize].is_some();
        let mut out = px.clone();
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                if !filled(x, y) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| filled(x + dx, y + dy)) {
                    out[y as usize * SIZE + x as usize] = Some((0, OUTLINE));
                }
            }
        }
        Some(out)
    }

    /// Metalwork and fittings for a held tool with a separate haft mesh.
    pub fn render_head(&self, schema: &str, materials: &[&str]) -> Option<Vec<Texel>> {
        self.paint_layers(schema, materials, false)
    }

    fn paint_layers(&self, schema: &str, materials: &[&str], include_haft: bool) -> Option<Vec<Texel>> {
        let layers = self.tools.get(schema)?;
        let mut px: Vec<Texel> = vec![None; SIZE * SIZE];
        for (mask, slot) in layers {
            if !include_haft && matches!(mask.as_str(), "handle" | "handle_long" | "handle_axe") {
                continue;
            }
            let cells = self.masks.get(mask)?;
            let material = materials.get(*slot).copied().unwrap_or("stone");
            let ramp = self.palettes.get(material).or_else(|| self.palettes.get("stone"))?;
            for (dst, &shade) in px.iter_mut().zip(cells) {
                if shade > 0 {
                    *dst = Some((shade, ramp[shade as usize - 1]));
                }
            }
        }
        Some(px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_sprite_sheet_parses_and_covers_every_material() {
        let sheet = parse(SPRITES_TXT).unwrap();
        for m in crate::MATERIALS.iter() {
            assert!(sheet.palettes.contains_key(m.id.as_str()), "no palette for {}", m.id.as_str());
        }
        for (schema, layers) in &sheet.tools {
            assert!(crate::PARTS.schemas.iter().any(|s| s.id == *schema), "unknown schema {schema}");
            for (mask, _) in layers {
                assert!(sheet.masks.contains_key(mask), "{schema} names missing mask {mask}");
            }
        }
    }

    #[test]
    fn tool_sprites_are_outlined_and_share_a_silhouette_across_materials() {
        let a = SHEET.render("pickaxe", &["gold", "copper", "wood"]).unwrap();
        let b = SHEET.render("pickaxe", &["flint", "sinew", "bone"]).unwrap();
        assert!(a.iter().zip(&b).all(|(a, b)| a.is_some() == b.is_some()));
        assert!(a.iter().any(|t| matches!(t, Some((0, _)))));
        assert!(a[0].is_none() && a[SIZE * SIZE - 1].is_none());
    }
}
