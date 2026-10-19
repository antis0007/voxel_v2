use super::*;

// Quantities are fixed-point inventory units (64 per whole item), not voxels.
pub(super) fn guardian(kind: u32) -> &'static [(usize, u32)] {
    match kind {
        0 => &[(13, 128)],                     // Cave grub: biomass.
        1 => &[(12, 64), (16, 32)],            // Sentinel: circuitry and coil parts.
        2 => &[(5, 64), (12, 32)],             // Wisp: crystal and arcane circuitry.
        3 => &[(13, 256), (4, 64)],            // Brute: richer fungal materials.
        4 => &[(4, 128), (5, 32)],             // Spitter: luminous fibre and crystal.
        _ => &[(12, 128), (15, 64), (17, 16)], // Warden: steel and a core fragment.
    }
}

pub(super) fn sanctum(cfg: &super::super::WorldSampleConfig, id: IVec3) -> Reward {
    let (name, bonus): (&str, &[(usize, u32)]) = match sites::reward_kind(cfg, id) {
        0 => ("Workshop reclaimed", &[(14, 192), (16, 64)]),
        1 => ("Watchtower reclaimed", &[(5, 192), (16, 128)]),
        2 => ("Waterworks reclaimed", &[(7, 256), (14, 192)]),
        3 => ("Foundry reclaimed", &[(15, 256), (16, 128)]),
        4 => ("Conservatory restored", &[(13, 512), (4, 256)]),
        5 => ("Archive unsealed", &[(17, 64), (5, 192)]),
        6 => ("Luminous grove restored", &[(4, 256), (13, 384)]),
        7 => ("Ember hollow cleansed", &[(3, 512), (15, 128)]),
        8 => ("Crystal cathedral unsealed", &[(5, 256), (17, 64)]),
        _ => ("Drowned grotto reclaimed", &[(7, 256), (16, 128)]),
    };
    let mut items = vec![(18, 128), (12, 64)];
    items.extend_from_slice(bonus);
    Reward {
        name,
        items,
        seconds: 7.0,
        discovery: None,
    }
}
