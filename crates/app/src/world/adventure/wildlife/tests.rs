use super::*;
use crate::world::WorldType;
use engine_core::VoxelScalePreset;

fn world(scale: VoxelScalePreset) -> World {
    World::with_seed(WorldType::Superflat, scale, 2738958700)
}
fn round_trip(a: &Adventure) -> State {
    let mut bytes = Vec::new();
    a.wildlife.encode(&a.enemies, &mut bytes);
    let mut words = bytes.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap()));
    let restored = State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("truncated"))).unwrap();
    assert!(words.next().is_none());
    restored
}

#[test]
fn wildlife_spawn_is_bounded_deterministic_and_clear_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let w = world(scale);
        let feet = Vec3::new(0.0, (w.terrain_height_at(0,0)+1) as f32 * scale.meters() + 0.02, 0.0);
        let mut a = Adventure::default();
        let mut b = Adventure::default();
        spawn(&mut a, &w, feet);
        spawn(&mut b, &w, feet);
        assert!(!a.enemies.is_empty(), "no wildlife at {scale:?}");
        assert!(a.enemies.len() <= ACTIVE_LIMIT);
        assert_eq!(a.enemies.len(), b.enemies.len());
        for (a,b) in a.enemies.iter().zip(&b.enemies) {
            assert_eq!((id(a), a.kind, a.pos), (id(b), b.kind, b.pos));
            assert!(free(&w, a.pos, a.kind));
            assert!(supported(&w, a.pos, 0.2));
        }
        for _ in 0..20 { spawn(&mut a, &w, feet); }
        assert!(a.enemies.len() <= ACTIVE_LIMIT);
        let mut ids: Vec<_> = a.enemies.iter().map(id).collect();
        ids.sort_unstable(); ids.dedup();
        assert_eq!(ids.len(), a.enemies.len());
    }
}

#[test]
fn wildlife_flees_without_attacking_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let w = world(scale);
        let floor = (w.terrain_height_at(0,0)+1) as f32 * scale.meters() + 0.02;
        for kind in [SHEEP, GOAT] {
            let mut e = Enemy::new(IVec3::ZERO, 0, kind, Vec3::new(0.0,floor,0.0), 0);
            let feet = e.pos + Vec3::Z * 2.0;
            for i in 0..240 {
                super::super::navigation::gravity(&mut e, &w, 1.0/120.0);
                tick(&mut e, &w, feet, i as f32 / 120.0, 1.0/120.0);
                assert!(free(&w, e.pos, kind));
                assert_eq!(e.tell, 0.0);
                assert!(!e.visible);
            }
            assert!(e.pos.distance(feet) > 3.0, "{kind} failed to flee at {scale:?}: {:?}", e.pos);
        }
    }
}

#[test]
fn wildlife_damage_position_and_death_survive_reload() {
    let w = world(VoxelScalePreset::OneMeter);
    let feet = Vec3::new(0.0, (w.terrain_height_at(0,0)+1) as f32 + 0.02, 0.0);
    let mut a = Adventure::default();
    spawn(&mut a, &w, feet);
    a.enemies[0].hurt(7.0);
    a.enemies[0].pos.x += 0.4;
    let key = id(&a.enemies[0]);
    let pos = a.enemies[0].pos;
    let health = a.enemies[0].health;
    let mut restored = Adventure { wildlife: round_trip(&a), ..Adventure::default() };
    spawn(&mut restored, &w, feet);
    let e = restored.enemies.iter_mut().find(|e| id(e)==key).unwrap();
    assert_eq!((e.pos,e.health),(pos,health));
    e.hurt(1000.0);
    let mut dead = Adventure { wildlife: round_trip(&restored), ..Adventure::default() };
    spawn(&mut dead, &w, feet);
    assert!(!dead.enemies.iter().any(|e| id(e)==key));
    assert!(dead.records.is_empty(), "wildlife must not clear guardian encounters");
}

#[test]
fn wildlife_decoder_rejects_oversized_and_nonfinite_records() {
    assert!(State::decode(&mut || Ok(RECORD_LIMIT as u32 + 1)).is_err());
    let mut words = vec![1, 0, 0, 0, SHEEP, f32::NAN.to_bits()];
    words.resize(13, 0);
    let mut words = words.into_iter();
    assert!(State::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("truncated"))).is_err());
}

#[test]
fn wildlife_support_rejects_cliffs_and_fluids_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = world(scale);
        let s = scale.meters();
        let y = (80.0 / s) as i32;
        for block in [BlockId::Grass, BlockId::Water, BlockId::Lava] {
            let mut cells = Vec::new();
            for x in (-2.0/s) as i32..(2.0/s) as i32 {
                for z in (-2.0/s) as i32..(2.0/s) as i32 { cells.push((IVec3::new(x,y,z),block)); }
            }
            w.set_many(&cells);
            let p = Vec3::new(0.0,80.0+s+0.02,0.0);
            assert_eq!(supported(&w,p,0.2), block==BlockId::Grass);
            assert_eq!(supported(&w,p+Vec3::Y,1.1), block==BlockId::Grass);
            assert!(!supported(&w,p+Vec3::Y*2.0,1.1));
        }
    }
}

#[test]
fn wildlife_populates_generated_outdoor_grass_without_editing_terrain() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let w = World::with_seed(WorldType::Hills, scale, 2738958700);
        let mut a = Adventure::default();
        let feet = Vec3::new(0.0,(w.terrain_height_at(0,0)+1) as f32*scale.meters()+0.02,0.0);
        spawn(&mut a,&w,feet);
        assert!(!a.enemies.is_empty(), "grass habitat absent at {scale:?}");
        assert!(a.enemies.iter().all(|e| free(&w,e.pos,e.kind)));
        assert!(a.records.is_empty());
    }
}
