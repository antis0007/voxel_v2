//! Read-only coverage diagnostic, not a test or native rendering acceptance.
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, landscape};
use std::{collections::BTreeMap, fmt::Write};

fn color(b: landscape::Biome) -> &'static str {
    use landscape::Biome::*;
    match b {
        Ocean => "#294760",
        Coast => "#c1ac78",
        Marsh | Fen => "#556e56",
        Meadow => "#a0ae62",
        Forest => "#406343",
        Taiga => "#426568",
        Desert => "#d1b67b",
        Badlands => "#a76c49",
        Tundra => "#a7aaa0",
        Alpine => "#c1c5be",
        Savanna => "#b0a35e",
        Rainforest => "#25563d",
        Heathland => "#897c72",
        LostPlateau => "#bd9956",
        CycadMarsh => "#649e88",
        CloudForest => "#6e9b9b",
        RedwoodGrove => "#794b38",
        OvergrownGardens => "#91bd72",
        FungalGrove => "#b08ba1",
        LastShore => "#bb7169",
        VolcanicField => "#625451",
        KarstUpland => "#b4baa1",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let seed = args
        .get(1)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(424242);
    let version = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(95);
    let out = args
        .get(3)
        .map(String::as_str)
        .unwrap_or("habitat-survey.svg");
    let cfg = WorldSampleConfig {
        planet: None,
        seed,
        generator_version: version,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let mut counts: BTreeMap<&str, (usize, i32, i32, i32)> = BTreeMap::new();
    let mut canopy_sites: BTreeMap<&str, Vec<(i32, i32, i32)>> = BTreeMap::new();
    let mut svg = String::from(
        "<svg xmlns='http://www.w3.org/2000/svg' width='900' height='720' viewBox='0 0 900 720'><rect width='900' height='720' fill='#172026'/><g font-family='sans-serif' fill='#e0e6df'>",
    );
    writeln!(
        svg,
        "<text x='24' y='28' font-size='18'>Habitat survey · seed {seed} · generator {version}</text>"
    )?;
    let (mut transition, mut land, mut total) = (0, 0, 0);
    for iz in 0..80 {
        for ix in 0..80 {
            let (x, z) = ((ix - 40) * 64, (iz - 40) * 64);
            let s = landscape::preview_landscape_lod(&cfg, x, z, 32.0);
            let distance = x * x + z * z;
            if s.ecotone > 0.6 {
                canopy_sites
                    .entry(s.biome.label())
                    .or_default()
                    .push((distance, x, z));
            }
            let entry = counts.entry(s.biome.label()).or_insert((0, i32::MAX, 0, 0));
            entry.0 += 1;
            if distance < entry.1 {
                entry.1 = distance;
                entry.2 = x;
                entry.3 = z;
            }
            total += 1;
            if s.biome != landscape::Biome::Ocean {
                land += 1;
            }
            if s.ecotone > 0.05 && s.ecotone < 0.95 {
                transition += 1;
            }
            writeln!(
                svg,
                "<rect x='{}' y='{}' width='8' height='8' fill='{}'><title>{}: ({x}, {z}), {:.0} m, blend {:.2}</title></rect>",
                24 + ix * 8,
                48 + iz * 8,
                color(s.biome),
                s.biome.label(),
                s.height,
                s.ecotone
            )?;
        }
    }
    println!("seed={seed} version={version} samples={total} land={land} transitions={transition}");
    for (i, (label, (count, d, x, z))) in counts.iter().enumerate() {
        println!(
            "{label:22} {:5.2}% nearest={:4.0}m at=({x},{z})",
            *count as f32 / total as f32 * 100.0,
            (*d as f32).sqrt()
        );
        let b = landscape::preview_landscape_lod(&cfg, *x, *z, 32.0).biome;
        writeln!(
            svg,
            "<rect x='684' y='{}' width='10' height='10' fill='{}'/><text x='702' y='{}' font-size='11'>{label} {:.1}%</text>",
            54 + i * 24,
            color(b),
            63 + i * 24,
            *count as f32 / total as f32 * 100.0
        )?;
    }
    svg.push_str("<text x='24' y='711' font-size='12'>5.12 km square · 64 m sample grid · offline classification, not a game capture</text></g></svg>");
    std::fs::write(out, svg)?;
    if args.iter().any(|s| s == "--trees") {
        for (name, (_, _, x, z)) in &counts {
            if !matches!(
                *name,
                "Redwood grove" | "Fungal grove" | "Cycad marsh" | "Lost plateau" | "Cloud forest"
            ) {
                continue;
            }
            let sites = canopy_sites.entry(name).or_default();
            sites.sort_unstable();
            if sites.is_empty() {
                sites.push((0, *x, *z));
            }
            let mut cells = Vec::new();
            for (d, px, pz) in sites.iter().take(12) {
                let (cx, cz) = (px.div_euclid(12), pz.div_euclid(12));
                cells.extend((-8i32..=8).flat_map(|dz| {
                    (-8i32..=8).map(move |dx| (*d + dx * dx + dz * dz, cx + dx, cz + dz))
                }));
            }
            cells.sort_unstable();
            let found = cells.into_iter().find_map(|(_, a, b)| {
                let tree = engine_worldgen::tree_visual(&cfg, a, b)?;
                let s =
                    landscape::sample(&cfg, tree.base.x.floor() as i32, tree.base.z.floor() as i32);
                if s.biome.label() != *name || s.ecotone < 0.6 {
                    return None;
                }
                if *name == "Fungal grove" && tree.canopy != engine_worldgen::TreeCanopy::Fungal {
                    return None;
                }
                if *name == "Cloud forest" && tree.conifer {
                    return None;
                }
                if *name == "Redwood grove" && (!tree.conifer || tree.height < 20.0) {
                    return None;
                }
                Some(tree)
            });
            match found {
                Some(t) => println!(
                    "[canopy] {name}: base=({:.1},{:.1},{:.1}) height={:.1} radius={:.1} shape={:?}",
                    t.base.x, t.base.y, t.base.z, t.height, t.radius, t.canopy
                ),
                None => {
                    println!("[canopy] {name}: no qualifying anchor near 12 surveyed core points")
                }
            }
        }
    }
    Ok(())
}
