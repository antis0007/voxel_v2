//! Gameplay side of site modifiers (generator 32+). The modifier itself is a
//! pure worldgen roll (`sites::modifiers`), so nothing here is saved: entering a
//! ruin re-derives it from the seed. Every effect is bounded and deterministic.
use super::{Adventure, Enemy, MAX_ENEMIES, World, flying, free, status};
use crate::player::Player;
use engine_worldgen::sites::modifiers::{SiteModifier, site_modifier};
use engine_worldgen::{WorldSampleConfig, sites};
use glam::{IVec3, Quat, Vec3};

/// Site id `z` reserved for Haunted wisps; they never touch site records.
pub(super) const WISP_Z: i32 = -31;
const WISP_KIND: u32 = 2;
const WISP_PERIOD: f32 = 20.0;
const MAX_WISPS: usize = 2;

/// The modifier of an encounter id: ruins (`z == 0`) and their cairn and cache
/// (`z == 2, 3`) share the ruin's region key.
pub(super) fn for_site(cfg: &WorldSampleConfig, id: IVec3) -> SiteModifier {
    if matches!(id.z, 0 | 2 | 3) {
        site_modifier(cfg, (id.x, id.y))
    } else {
        SiteModifier::None
    }
}

/// The modifier of the nearest modified site whose encounter range holds `feet`.
pub(super) fn current(cfg: &WorldSampleConfig, feet: Vec3) -> SiteModifier {
    sites::encounters(cfg, feet)
        .into_iter()
        .filter_map(|(id, at, _)| {
            let m = for_site(cfg, id);
            (m != SiteModifier::None).then_some(((at - feet).length_squared(), m))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map_or(SiteModifier::None, |(_, m)| m)
}

/// The affix a modifier grants every foe of the site, bosses included.
pub(super) fn affix(m: SiteModifier) -> Option<status::Affix> {
    use status::Affix as A;
    match m {
        SiteModifier::Volatile => Some(A::Volatile),
        SiteModifier::Overgrown => Some(A::Regenerating),
        SiteModifier::Smouldering => Some(A::Fiery),
        SiteModifier::Frostbound => Some(A::Frostbound),
        SiteModifier::Flooded | SiteModifier::Crystalline => Some(A::Armored),
        SiteModifier::Haunted => Some(A::Warded),
        SiteModifier::Echoing | SiteModifier::Unstable => Some(A::Swift),
        SiteModifier::Gloom | SiteModifier::None => None,
    }
}

pub(super) fn affixes(m: SiteModifier, base: status::Affixes) -> status::Affixes {
    affix(m).map_or(base, |a| base.with(a))
}

/// Mana cost multiplier for casts made inside the site.
pub(super) fn mana_mul(m: SiteModifier) -> f32 {
    if m == SiteModifier::Crystalline { 0.7 } else { 1.0 }
}

/// Unstable sites bend each cast by up to 4 degrees, following a fixed
/// time-driven wobble (deterministic for a given adventure clock).
pub(super) fn wobble(m: SiteModifier, forward: Vec3, time: f32) -> Vec3 {
    if m != SiteModifier::Unstable {
        return forward;
    }
    let yaw = (time * 2.3).sin() * 4f32.to_radians();
    let pitch = (time * 3.7).cos() * 2.5f32.to_radians();
    let side = forward.cross(Vec3::Y).normalize_or_zero();
    let q = Quat::from_axis_angle(Vec3::Y, yaw)
        * if side == Vec3::ZERO { Quat::IDENTITY } else { Quat::from_axis_angle(side, pitch) };
    (q * forward).normalize_or(forward)
}

/// Squared radius over which a foe that sees the player alerts its site-mates.
pub(super) fn alert_radius_sq(m: SiteModifier) -> f32 {
    if m == SiteModifier::Echoing { 576.0 } else { 144.0 }
}

/// How far foes see in this site; Gloom hides everyone until close.
pub(super) fn sight_range(m: SiteModifier) -> f32 {
    if m == SiteModifier::Gloom { 16.0 } else { 27.0 }
}

/// Per-step effects. Runs inside `Adventure::update`.
pub(super) fn step(adv: &mut Adventure, world: &World, player: &mut Player, feet: Vec3, dt: f32) {
    let cfg = world.sample_config();
    let m = current(&cfg, feet);
    adv.site_modifier = m;
    if m == SiteModifier::Frostbound && player.on_ground && !player.flying {
        // Rime underfoot: bleed a share of horizontal speed every step.
        let keep = 1.0 - (dt * 4.0).min(0.35);
        player.vel.x *= keep;
        player.vel.z *= keep;
    }
    if m != SiteModifier::Haunted {
        adv.haunt_clock = 0.0;
        return;
    }
    adv.haunt_clock += dt;
    if adv.haunt_clock < WISP_PERIOD {
        return;
    }
    adv.haunt_clock = 0.0;
    let wisps = adv.enemies.iter().filter(|e| e.site.z == WISP_Z).count();
    let wild = adv.enemies.iter().filter(|e| e.habitat.is_none()).count();
    if wisps >= MAX_WISPS || wild >= MAX_ENEMIES {
        return;
    }
    debug_assert!(flying(WISP_KIND));
    let turn = (adv.time / WISP_PERIOD) as i32;
    for attempt in 0..6 {
        let a = turn as f32 * 2.4 + attempt as f32 * 1.05;
        let p = feet + Vec3::new(a.cos() * 9.0, 3.0, a.sin() * 9.0);
        if free(world, p, WISP_KIND) {
            let site = IVec3::new(feet.x as i32, feet.z as i32, WISP_Z);
            adv.enemies
                .push(Enemy::new(site, wisps as u32, WISP_KIND, p, 1).with_affixes(affixes(m, Default::default())));
            break;
        }
    }
}

/// Dead wisps leave quietly: no record, loot or site completion.
pub(super) fn reap(enemies: &mut Vec<Enemy>) {
    enemies.retain(|e| e.site.z != WISP_Z || e.health > 0.0);
}

/// HUD text, Noita perk style.
pub(super) fn hud(m: SiteModifier) -> Option<(&'static str, &'static str)> {
    (m != SiteModifier::None).then(|| (m.name(), m.description()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use engine_worldgen::WorldType;

    #[test]
    fn modifier_effects_are_bounded() {
        for m in SiteModifier::ALL {
            let f = Vec3::new(0.3, -0.2, 0.9).normalize();
            for t in 0..200 {
                let w = wobble(m, f, t as f32 * 0.37);
                assert!(w.angle_between(f) <= 5f32.to_radians());
            }
            assert!(mana_mul(m) > 0.5 && mana_mul(m) <= 1.0);
            assert!(hud(m).is_some());
        }
        assert!(hud(SiteModifier::None).is_none());
        assert!(affix(SiteModifier::Volatile) == Some(status::Affix::Volatile));
    }

    #[test]
    fn modifier_site_lookup_is_v32_only() {
        let cfg = |v| WorldSampleConfig {
            generator_version: v,
            seed: 77,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::QuarterMeter,
        };
        let mut hit = 0;
        for x in -30..30 {
            let id = IVec3::new(x, 4, 0);
            assert_eq!(for_site(&cfg(31), id), SiteModifier::None);
            assert_eq!(for_site(&cfg(32), IVec3::new(x, 4, 1)), SiteModifier::None);
            hit += (for_site(&cfg(32), id) != SiteModifier::None) as i32;
        }
        assert!(hit > 20);
    }
}
