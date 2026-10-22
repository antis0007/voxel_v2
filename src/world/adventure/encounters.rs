//! Extensible encounter recipes composed onto existing saved creature identities.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mechanic {
    None,
    RubbleAmbush,
    CacheAmbush,
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
];

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
    } else if id.z == 2 || id.z == 3 || id.y.abs() >= 500 {
        1
    } else {
        3
    }
}

/// True while the disguise or its audible emergence owns the creature's action.
pub(super) fn ambush(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    forward: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
) -> bool {
    e.encounter.exposed = (e.encounter.exposed - dt).max(0.0);
    if e.encounter.concealed {
        let offset = e.center() - eye;
        let distance = offset.length();
        let disturbed = e.health < e.max_health() - 0.1
            || e.status.any()
            || e.slow > 0.0
            || (e.pos - e.home).length() > 0.6;
        let opportunity =
            distance < 3.8 || (distance < 6.0 && forward.dot(offset.normalize_or_zero()) < 0.2);
        if disturbed || (opportunity && line(world, e.pos + Vec3::Y * 0.25, eye)) {
            e.encounter.concealed = false;
            e.encounter.emerge = 0.85;
            e.encounter.exposed = if disturbed { 2.5 } else { 1.25 };
            e.cooldown = 0.85;
            e.last_seen = feet;
            e.alert = 5.0;
            sound(sounds, Cue::Warning, Some(e.pos), 0.8);
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
    if !e.encounter.concealed {
        return false;
    }
    let cache = e.encounter.mechanic() == Mechanic::CacheAmbush;
    let color = if cache {
        [0.29, 0.21, 0.12]
    } else {
        [0.30, 0.29, 0.25]
    };
    if cache {
        m.cuboid(
            e.pos + Vec3::Y * 0.18,
            Vec3::new(0.70, 0.35, 0.50),
            Quat::IDENTITY,
            color,
            0.0,
        );
        m.cuboid(
            e.pos + Vec3::Y * 0.39,
            Vec3::new(0.74, 0.10, 0.54),
            Quat::IDENTITY,
            [0.36, 0.27, 0.15],
            0.0,
        );
        for x in [-0.23, 0.23] {
            m.cuboid(
                e.pos + Vec3::new(x, 0.23, -0.28),
                Vec3::new(0.05, 0.40, 0.05),
                Quat::IDENTITY,
                [0.39, 0.36, 0.27],
                0.0,
            );
        }
    } else {
        for (x, y, z, w) in [
            (-0.22, 0.12, 0.0, 0.4),
            (0.20, 0.15, 0.08, 0.42),
            (0.0, 0.24, -0.08, 0.45),
        ] {
            m.cuboid(
                e.pos + Vec3::new(x, y, z),
                Vec3::new(w, 0.25, 0.5),
                Quat::IDENTITY,
                color,
                0.0,
            );
        }
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
    if e.habitat.is_some() {
        return match e.kind { 0 => "Cistern spider", 1 => "Root nursery guardian", _ => "Orchard Matron" };
    }
    e.encounter
        .recipe
        .map_or_else(|| enemy_name(e.kind), |i| RECIPES[i].name)
}
