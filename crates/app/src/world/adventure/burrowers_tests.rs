use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{World, WorldType, CHUNK_SIZE};

/// A world with the chunks around `feet` resident, so the spawner sees rock.
fn resident(kind: WorldType, seed: u32, feet_of: impl Fn(&World) -> Vec3) -> (World, Vec3) {
    let mut w = World::with_seed(kind, VoxelScalePreset::OneMeter, seed);
    let feet = feet_of(&w);
    let c = (feet / CHUNK_SIZE as f32).floor().as_ivec3();
    for dz in -1..=1 {
        for dy in -1..=0 {
            for dx in -1..=1 {
                assert!(w.admit_edit_chunk(c + IVec3::new(dx, dy, dz)));
            }
        }
    }
    (w, feet)
}

fn surface(seed: u32) -> (World, Vec3) {
    resident(WorldType::Hills, seed, |w| Vec3::new(0.5, (w.terrain_height_at(0, 0) + 1) as f32, 0.5))
}

/// Runs `seconds` of spawn passes with a standing player; returns bodies
/// that rose, bodies that rose wary, and damage taken.
fn run(w: &World, feet: Vec3, seconds: u32) -> (u32, u32, f32) {
    let mut a = Adventure::default();
    let (mut risen, mut wary, mut damage) = (0, 0, 0.0);
    let eye = feet + Vec3::Y * 1.6;
    for s in 0..seconds {
        let before: Vec<u32> = a.enemies.iter().filter(|e| is_burrow_body(e)).map(|e| e.slot).collect();
        spawn(&mut a, w, feet);
        for e in a.enemies.iter().filter(|e| is_burrow_body(e) && !before.contains(&e.slot)) {
            risen += 1;
            wary += u32::from(e.burrow.wary);
        }
        let mut fx = Vec::new();
        for step in 0..20 {
            let time = s as f32 + step as f32 * 0.05;
            for e in a.enemies.iter_mut().filter(|e| is_burrow_body(e)) {
                damage += tick(e, w, feet, eye, 0.05, time, true, 0.0, &mut fx).0;
            }
        }
    }
    (risen, wary, damage)
}

/// A session starts quiet: nothing rises in the first spawn passes.
#[test]
fn nothing_rises_during_the_spawn_grace() {
    let (w, feet) = resident(WorldType::Hills, 3, |w| Vec3::new(0.5, (w.terrain_height_at(0, 0) - 12) as f32, 0.5));
    let mut a = Adventure::default();
    for _ in 0..SPAWN_GRACE_PASSES {
        spawn(&mut a, &w, feet);
    }
    assert!(!a.enemies.iter().any(is_burrow_body), "a body rose during the grace");
}

/// On open ground worms still turn up, but most are wary and a standing
/// player is rarely hunted.
#[test]
fn surface_worms_are_seen_but_rarely_hunt() {
    let (mut risen, mut wary, mut damage) = (0, 0, 0.0);
    for seed in [1, 2, 3, 4, 5, 6] {
        let (w, feet) = surface(seed);
        let (r, wr, d) = run(&w, feet, 900);
        risen += r;
        wary += wr;
        damage += d;
    }
    println!("surface: risen {risen} wary {wary} damage {damage:.1}");
    assert!(risen >= 12, "worms are not rare on open ground: {risen} in 90 minutes");
    assert!(wary * 10 >= risen * 6, "most open-ground bodies are wary: {wary}/{risen}");
    assert!(damage <= 120.0, "six 15-minute stands lost {damage} health on the surface");
}
