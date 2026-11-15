//! Extensible encounter recipes composed onto existing saved creature identities.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mechanic {
    None,
    RubbleAmbush,
    CacheAmbush,
    CeilingDrop,
    LightReveal,
    Suppressor,
    Interceptor,
    Hexer,
}

pub(super) struct Recipe {
    pub kind: u32,
    pub mechanic: Mechanic,
    pub name: &'static str,
    pub loot: &'static [(usize, u32)],
}

// Add recipes here; existing site/slot records still own death and reward persistence.
const CEILING_DROP: usize = 5;
const LIGHT_REVEAL: usize = 6;

pub(super) const RECIPES: &[Recipe] = &[
    Recipe {
        kind: 3,
        mechanic: Mechanic::RubbleAmbush,
        name: "Scree Lurker",
        loot: &[(3, 64), (13, 128)],
    },
    Recipe {
        kind: 10,
        mechanic: Mechanic::CacheAmbush,
        name: "Patient Cache Mimic",
        loot: &[(12, 64), (16, 32)],
    },
    Recipe {
        kind: 4,
        mechanic: Mechanic::Suppressor,
        name: "Aether Harrier",
        loot: &[(4, 64)],
    },
    Recipe {
        kind: 11,
        mechanic: Mechanic::Interceptor,
        name: "Cistern Stalker",
        loot: &[(7, 64)],
    },
    Recipe {
        kind: 12,
        mechanic: Mechanic::Hexer,
        name: "Kiln Trapper",
        loot: &[(3, 64), (5, 32)],
    },
    Recipe { kind: 3, mechanic: Mechanic::CeilingDrop, name: "Vault Droplet", loot: &[(3, 64), (13, 128)] },
    Recipe { kind: 11, mechanic: Mechanic::LightReveal, name: "Veil Stalker", loot: &[(7, 64)] },
];

#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct State {
    pub recipe: Option<usize>,
    pub concealed: bool,
    pub emerge: f32,
    pub exposed: f32,
    pub player_last: Vec3,
    pub motion: Vec3,
    pub stationary: f32,
    pub last_pattern: u32,
    pub boss_phase: u8,
    pub windup: f32,
}
impl State {
    pub fn new(site: IVec3, slot: u32, kind: u32, pos: Vec3) -> Self {
        let recipe = RECIPES.iter().position(|r| r.kind == kind).filter(|_| {
            site != IVec3::ZERO && (slot == 0 || (site.x ^ site.y ^ slot as i32) & 1 == 0)
        });
        let concealed = recipe.is_some_and(|i| {
            matches!(
                RECIPES[i].mechanic,
                Mechanic::RubbleAmbush | Mechanic::CacheAmbush
            )
        });
        Self {
            recipe,
            concealed,
            emerge: 0.0,
            exposed: 0.0,
            player_last: pos,
            motion: Vec3::ZERO,
            stationary: 0.0,
            last_pattern: u32::MAX,
            boss_phase: 0,
            windup: 1.1,
        }
    }
    pub fn specialize(&mut self, mechanic: Mechanic) {
        self.recipe = Some(match mechanic {
            Mechanic::CeilingDrop => CEILING_DROP,
            Mechanic::LightReveal => LIGHT_REVEAL,
            _ => return,
        });
        self.concealed = true;
    }

    pub fn mechanic(&self) -> Mechanic {
        self.recipe.map_or(Mechanic::None, |i| RECIPES[i].mechanic)
    }
    pub fn observe(&mut self, feet: Vec3, dt: f32) {
        let delta = feet - self.player_last;
        let velocity = (delta / dt.max(0.001)).clamp_length_max(7.0);
        self.motion = self.motion.lerp(velocity, 1.0 - (-dt * 5.0).exp());
        self.stationary = if delta.length_squared() < (dt * 0.7).powi(2) {
            self.stationary + dt
        } else {
            0.0
        };
        self.player_last = feet;
    }
}

pub(super) fn slots(id: IVec3) -> u32 {
    if id == IVec3::ZERO {
        2
    } else if super::sites::district_post(id) {
        3
    } else if id.z == 2 || id.z == 3 || id.y.abs() >= 500 {
        1
    } else {
        3
    }
}

pub(super) fn clear_mask(id: IVec3) -> u32 {
    // The authored Observatory has the guardian in the existing boss slot,
    // with no ambient guards. Its death, not visiting the room, earns clearance.
    if observatory::is_site(id) { 1 << 3 } else { (1 << slots(id)) - 1 }
}

/// True while the disguise or its audible emergence owns the creature's action.
fn wake(e: &mut Enemy, feet: Vec3, disturbed: bool, sounds: &mut Vec<Sound>) {
    e.encounter.concealed = false;
    e.encounter.emerge = 0.85;
    e.encounter.exposed = if disturbed { 2.5 } else { 1.25 };
    e.cooldown = 0.85;
    e.last_seen = feet;
    e.alert = 5.0;
    e.aware = 1.0;
    e.spotted_flash = 1.0;
    sound(sounds, Cue::Warning, Some(e.pos), 0.8);
}

pub(crate) fn focused_cache(world: &World, eye: Vec3, forward: Vec3, reach: f32) -> Option<(f32, IVec3, u32)> {
    let scale = world.voxel_scale.meters();
    world.adventure.enemies.iter().filter_map(|enemy| {
        if enemy.health <= 0.0 || !enemy.encounter.concealed || enemy.encounter.mechanic() != Mechanic::CacheAmbush {
            return None;
        }
        let center = enemy.pos + Vec3::Y * (scale * 0.45);
        let distance = (center - eye).dot(forward);
        ((0.0..=reach).contains(&distance) && (eye + forward * distance).distance_squared(center) < (scale * 0.56).powi(2)
            && line(world, eye, center)).then_some((distance, enemy.site, enemy.slot))
    }).min_by(|a,b| a.0.total_cmp(&b.0))
}

pub(crate) fn open_cache(world: &mut World, site: IVec3, slot: u32, feet: Vec3) {
    let adventure = &mut world.adventure;
    if let Some(enemy) = adventure.enemies.iter_mut().find(|enemy| enemy.site == site && enemy.slot == slot
        && enemy.health > 0.0 && enemy.encounter.concealed && enemy.encounter.mechanic() == Mechanic::CacheAmbush)
    {
        wake(enemy, feet, false, &mut adventure.sounds);
    }
}

pub(super) fn ambush(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    forward: Vec3,
    senses: &super::perception::Senses,
    dt: f32,
    sounds: &mut Vec<Sound>,
) -> bool {
    if !super::aberrations::is_kind(e.kind) {
        e.encounter.exposed = (e.encounter.exposed - dt).max(0.0);
    }
    if e.encounter.concealed {
        let offset = e.center() - eye;
        let distance = offset.length();
        let disturbed = e.health < e.max_health() - 0.1
            || e.status.any()
            || e.slow > 0.0
            || (e.pos - e.home).length() > 0.6;
        let woken = if e.encounter.mechanic() == Mechanic::CeilingDrop {
            let behind = e.pos - feet;
            let below = behind.y > 0.5 && behind.x.hypot(behind.z) < 2.6;
            let los = below && line(world, e.center(), eye);
            below && super::perception::watch(e, senses, distance, los, dt)
        } else if e.encounter.mechanic() == Mechanic::LightReveal {
            let cell = (e.center() / world.voxel_scale.meters()).floor().as_ivec3();
            senses.noticeable && distance < 10.0 && line(world, e.center(), eye)
                && crate::world::block_light::level_at(world, cell) >= 6
        } else if e.encounter.mechanic() == Mechanic::CacheAmbush {
            distance < 2.4 && forward.dot(offset.normalize_or_zero()) < -0.15
                && line(world, e.pos + Vec3::Y * 0.25, eye)
        } else {
            // Rubble lurkers watch all round, but a crouched player in the dark can
            // creep past at a few metres; noise nearby and close approach wake them.
            let los = distance < 6.0 && line(world, e.pos + Vec3::Y * 0.25, eye);
            super::perception::watch(e, senses, distance, los, dt)
        };
        if disturbed || woken {
            wake(e, feet, disturbed, sounds);
        } else {
            e.visible = false;
            e.alert = 0.0;
            return true;
        }
    }
    if e.encounter.emerge > 0.0 {
        e.encounter.emerge = (e.encounter.emerge - dt).max(0.0);
        e.cooldown = e.cooldown.max(e.encounter.emerge);
        return true;
    }
    false
}

pub(super) fn disguise(e: &Enemy, m: &mut Model, time: f32) -> bool {
    if e.encounter.mechanic() == Mechanic::CacheAmbush && (e.encounter.concealed || e.encounter.emerge > 0.0) {
        let scale = m.scale_meters();
        let opened = if e.encounter.concealed { 0.0 } else { 1.0 - e.encounter.emerge / 0.85 };
        crate::mesh::models::chest::draw(m, e.pos, scale, 0, 0, opened);
        if opened > 0.1 {
            for x in [-0.24, -0.08, 0.08, 0.24] {
                m.cuboid(e.pos + Vec3::new(x, 0.66, -0.37) * scale,
                    Vec3::new(0.053, 0.10, 0.066) * scale, Quat::IDENTITY,
                    [0.81, 0.76, 0.58], 0.0);
            }
        } else if e.encounter.concealed && (time * 2.0 + e.slot as f32).sin() > 0.96 {
            m.glow_voxel(e.pos + Vec3::new(0.16, 0.63, -0.451) * scale,
                Vec3::splat(0.019 * scale), Quat::IDENTITY, PALETTE_AMBER, 0.25, 0.7);
        }
        return true;
    }
    if !e.encounter.concealed { return false; }
    if e.encounter.mechanic() == Mechanic::LightReveal {
        return true;
    }
    if e.encounter.mechanic() == Mechanic::CeilingDrop {
        let shake = e.aware * 0.06 * (time * 23.0).sin();
        for (offset, size) in [
            (Vec3::new(0.0, 0.87, 0.0), Vec3::new(0.78, 0.23, 0.69)),
            (Vec3::new(-0.14, 0.64, 0.06), Vec3::new(0.36, 0.30, 0.43)),
            (Vec3::new(0.17, 0.44, -0.07), Vec3::new(0.22, 0.27, 0.25)),
        ] {
            m.cuboid(e.pos + offset + Vec3::X * shake, size, Quat::IDENTITY, [0.29, 0.28, 0.24], 0.0);
        }
        if e.aware >= super::perception::SUSPICIOUS {
            m.glow_voxel(e.pos + Vec3::Y * 0.4, Vec3::splat(0.065), Quat::IDENTITY, PALETTE_AMBER, 0.45, 0.8);
        }
        return true;
    }
    // A suspicious lurker shifts under its stones: the fair warning before it bursts.
    let stir = ((e.aware - super::perception::SUSPICIOUS) / (1.0 - super::perception::SUSPICIOUS)).clamp(0.0, 1.0);
    for (i, (x, y, z, w)) in [
        (-0.22, 0.12, 0.0, 0.4),
        (0.20, 0.15, 0.08, 0.42),
        (0.0, 0.24, -0.08, 0.45),
    ].into_iter().enumerate() {
        let shake = stir * 0.035 * (time * 23.0 + i as f32 * 2.3 + e.slot as f32).sin();
        m.cuboid(e.pos + Vec3::new(x + shake, y + shake.abs() * 0.6, z), Vec3::new(w, 0.25, 0.5),
            Quat::IDENTITY, [0.30, 0.29, 0.25], 0.0);
    }
    // A still seam and sparse glint let observant players spot and pre-empt the trap.
    m.cuboid(
        e.pos + Vec3::new(0.0, 0.18, -0.29),
        Vec3::new(0.20, 0.05, 0.05),
        Quat::IDENTITY,
        [0.12, 0.10, 0.08],
        0.0,
    );
    if ((time * 0.8) as u32 + e.slot).is_multiple_of(5) {
        m.glow_voxel(
            e.pos + Vec3::new(0.05, 0.18, -0.32),
            Vec3::splat(0.05),
            Quat::IDENTITY,
            PALETTE_AMBER,
            0.35,
            0.8,
        );
    }
    true
}

pub(super) fn name(e: &Enemy) -> &'static str {
    if vault_fight::lead(e) { return "Vault Charger"; }
    if let Some(id) = wildlife::vehicle::villager_id(e) {
        return crate::world::travel::trades::name(id);
    }
    if e.habitat.is_some() {
        return match e.kind { 0 => "Cistern spider", 1 => "Root nursery guardian", _ => "Orchard Matron" };
    }
    e.encounter
        .recipe
        .map_or_else(|| enemy_name(e.kind), |i| RECIPES[i].name)
}
