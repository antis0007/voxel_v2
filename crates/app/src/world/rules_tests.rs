use super::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("voxel-rules-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn every_rule_flipped() -> Rules {
    Rules {
        mode: GameMode::Survival,
        keep_inventory: true,
        mob_spawning: false,
        daylight_cycle: false,
        pvp: false,
        spawn_protection_m: 48,
    }
}

#[test]
fn rules_round_trip_through_bytes_and_disk() {
    for rules in [Rules::default(), every_rule_flipped(), Rules::new_world(GameMode::Survival)] {
        assert_eq!(Rules::decode(&rules.encode()).unwrap(), rules);
    }
    let dir = scratch("round-trip");
    every_rule_flipped().save(&dir).unwrap();
    assert_eq!(Rules::load(&dir).unwrap(), every_rule_flipped());
    assert!(!dir.join(format!("{FILE}.tmp")).exists());
    Rules::default().save(&dir).unwrap();
    assert_eq!(Rules::load(&dir).unwrap(), Rules::default());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rules_missing_file_is_creative() {
    let dir = scratch("missing");
    let rules = Rules::load(&dir).unwrap();
    assert_eq!(rules, Rules::default());
    assert_eq!(rules.mode, GameMode::Creative);
    assert!(rules.mode.creative_tools() && rules.mode.flight());
    assert!(rules.mob_spawning && rules.daylight_cycle && !rules.keep_inventory);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rules_reject_corrupt_files() {
    let good = every_rule_flipped().encode();
    // Every single-bit flip must fail the checksum or a field bound.
    for i in 0..good.len() * 8 {
        let mut bad = good.clone();
        bad[i / 8] ^= 1 << (i % 8);
        assert!(Rules::decode(&bad).is_err(), "bit {i} flip accepted");
    }
    assert!(Rules::decode(&good[..good.len() - 1]).is_err());
    let mut long = good.clone();
    long.push(0);
    assert!(Rules::decode(&long).is_err());
    assert!(Rules::decode(&[]).is_err());

    let reseal = |mutate: &dyn Fn(&mut Vec<u8>)| {
        let mut body = good[..good.len() - 8].to_vec();
        mutate(&mut body);
        let sum = checksum(&body);
        body.extend_from_slice(&sum.to_le_bytes());
        body
    };
    assert!(Rules::decode(&reseal(&|b| b[8] = 9)).is_err(), "unknown mode");
    assert!(Rules::decode(&reseal(&|b| b[9] |= 0x80)).is_err(), "unknown flag");
    assert!(Rules::decode(&reseal(&|b| b[10..12].copy_from_slice(&u16::MAX.to_le_bytes()))).is_err());
    assert!(Rules::decode(&reseal(&|b| b[..8].copy_from_slice(b"VXRUL999"))).is_err());

    let dir = scratch("corrupt");
    std::fs::write(dir.join(FILE), b"garbage").unwrap();
    assert!(Rules::load(&dir).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rules_spawn_protection_is_horizontal() {
    let rules = Rules { spawn_protection_m: 16, ..Rules::default() };
    let spawn = glam::Vec3::new(10.0, 64.0, 10.0);
    assert!(rules.protects(spawn, spawn + glam::Vec3::new(15.0, -40.0, 0.0)));
    assert!(!rules.protects(spawn, spawn + glam::Vec3::new(16.0, 0.0, 0.0)));
    assert!(!Rules::default().protects(spawn, spawn));
}

#[test]
fn survival_world_never_toggles_flight() {
    use crate::simulation::fixed;
    for mode in GameMode::ALL {
        let mut w = crate::world::World::new(crate::world::WorldType::Superflat, engine_core::VoxelScalePreset::OneMeter);
        w.rules.mode = mode;
        let mut p = crate::player::Player::new();
        p.pos = w.suggested_spawn();
        let mut input = crate::player::Input::default();
        let mut flew = false;
        // Two jump taps 3 ticks apart: well inside the 0.25 s double-tap window.
        for tick in 0..40 {
            input.jump = matches!(tick, 10 | 13);
            fixed::gameplay(&mut w, &mut p, &mut input, true, |_, _, _, _| fixed::Actions::default());
            flew |= p.flying;
        }
        assert_eq!(flew, mode.flight(), "{mode:?}");
    }
}
