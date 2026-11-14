//! Cosmetic response of each spell material. Gameplay never reads these profiles.
use super::{Projectile, Vec3, snapline, spells, status::Element};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub(crate) enum Family {
    #[default]
    Plain,
    Spark,
    Ember,
    Frost,
    Shock,
    Spore,
    Acid,
    Water,
    Rift,
    Thread,
    Lumen,
}

#[derive(Clone, Copy)]
pub(super) enum Contact {
    Actor,
    Glance,
    Surface(Vec3),
}

pub(super) struct Look {
    pub hot: [f32; 3],
    pub spent: [f32; 3],
    pub speed: f32,
    pub lift: f32,
    pub gravity: f32,
    pub drag: f32,
    pub rebound: f32,
    pub life: f32,
    pub stretch: f32,
    pub glow: f32,
}

const LOOKS: [Look; Family::Lumen as usize + 1] = [
    Look { hot: [1.0; 3], spent: [0.7; 3], speed: 2.0, lift: 0.0,
        gravity: 2.2, drag: 1.4, rebound: 0.42, life: 0.30, stretch: 1.0, glow: 1.35 },
    Look { hot: [1.0, 0.88, 0.59], spent: [0.69, 0.24, 0.09], speed: 2.8, lift: 0.5,
        gravity: 4.0, drag: 2.0, rebound: 0.32, life: 0.29, stretch: 1.9, glow: 1.65 },
    Look { hot: [1.0, 0.80, 0.35], spent: [0.70, 0.07, 0.025], speed: 1.7, lift: 1.35,
        gravity: -0.7, drag: 2.7, rebound: 0.16, life: 0.52, stretch: 1.35, glow: 1.70 },
    Look { hot: [0.82, 0.99, 1.0], spent: [0.15, 0.37, 0.78], speed: 2.2, lift: 0.5,
        gravity: 6.2, drag: 0.9, rebound: 0.48, life: 0.43, stretch: 2.7, glow: 1.30 },
    Look { hot: [0.84, 0.95, 1.0], spent: [0.25, 0.24, 0.87], speed: 5.6, lift: 0.1,
        gravity: 0.0, drag: 4.2, rebound: 0.22, life: 0.18, stretch: 3.7, glow: 2.10 },
    Look { hot: [0.75, 0.77, 0.30], spent: [0.17, 0.34, 0.15], speed: 0.8, lift: 0.37,
        gravity: -0.25, drag: 2.0, rebound: 0.14, life: 0.75, stretch: 1.0, glow: 0.55 },
    Look { hot: [0.80, 0.98, 0.28], spent: [0.24, 0.40, 0.04], speed: 1.6, lift: 0.55,
        gravity: 4.8, drag: 2.5, rebound: 0.08, life: 0.46, stretch: 1.5, glow: 0.95 },
    Look { hot: [0.74, 0.99, 0.97], spent: [0.06, 0.43, 0.55], speed: 2.6, lift: 1.0,
        gravity: 7.5, drag: 1.1, rebound: 0.13, life: 0.37, stretch: 1.0, glow: 0.80 },
    Look { hot: [0.86, 0.61, 0.98], spent: [0.26, 0.10, 0.51], speed: 1.45, lift: 0.1,
        gravity: 0.0, drag: 2.8, rebound: 0.18, life: 0.47, stretch: 1.9, glow: 1.40 },
    Look { hot: [1.0, 0.87, 0.68], spent: [0.0; 3], speed: 3.0, lift: 0.25,
        gravity: 1.4, drag: 4.6, rebound: 0.20, life: 0.28, stretch: 2.9, glow: 1.65 },
    Look { hot: [0.86, 1.0, 0.78], spent: [0.24, 0.63, 0.46], speed: 3.6, lift: 0.3,
        gravity: 3.0, drag: 3.6, rebound: 0.22, life: 0.23, stretch: 2.2, glow: 1.45 },
];

impl Family {
    pub(super) fn look(self) -> &'static Look {
        &LOOKS[self as usize]
    }

    pub(super) fn for_shot(shot: &Projectile) -> (Self, [f32; 3]) {
        Self::for_cast(shot.spell, shot.color(), shot.mods.element)
    }

    pub(super) fn for_cast(spell: u32, tint: [f32; 3], coat: Option<Element>) -> (Self, [f32; 3]) {
        let native = match spell {
            0 | 3 | 15 | 29 | 71 | 85 | 86 | 87 => Self::Spark,
            12 | 28 => Self::Ember,
            2 | 13 | 81 => Self::Frost,
            14 | 83 | spells::STORM_SURGE => Self::Shock,
            5 => Self::Spore,
            74 | 82 => Self::Acid,
            73 => Self::Water,
            6 | 10 | 72 | 84 => Self::Rift,
            snapline::SPELL => Self::Thread,
            9 | 51 => Self::Lumen,
            _ => Self::Plain,
        };
        let Some(coat) = coat else { return (native, tint); };
        let family = match coat {
            Element::Fire => Self::Ember,
            Element::Frost => Self::Frost,
            Element::Shock => Self::Shock,
            Element::Toxic => Self::Acid,
            Element::Wet => Self::Water,
        };
        (family, if family == native { tint } else { coat.color() })
    }

    /// Seed varies neighbouring sparks without introducing frame-to-frame noise.
    pub(crate) fn color(self, tint: [f32; 3], life: f32, seed: u32) -> [f32; 3] {
        if self == Self::Plain { return tint; }
        let look = self.look();
        let variation = (seed & 255) as f32 / 255.0;
        let fresh = life * life;
        let tint = Vec3::from_array(tint);
        let hot = tint.lerp(Vec3::from_array(look.hot), 0.48 + variation * 0.22);
        let spent = if self == Self::Thread {
            tint * 0.62
        } else {
            tint.lerp(Vec3::from_array(look.spent), 0.60 + variation * 0.20)
        };
        spent.lerp(hot, fresh).to_array()
    }

    pub(super) fn velocity(self, pos: Vec3, mut vel: Vec3, gravity: f32, time: f32, dt: f32) -> Vec3 {
        vel.y -= gravity * dt;
        if matches!(self, Self::Ember | Self::Spore) || self == Self::Plain && gravity < 0.5 {
            let curl = Vec3::new(
                (pos.y * 1.4 + time * 0.7).sin(), 0.2, (pos.x * 1.4 + time * 0.5).cos(),
            );
            vel += curl * dt * if self == Self::Spore { 1.1 } else { 0.35 };
        } else if self == Self::Rift {
            vel += Vec3::Y.cross(vel) * dt * 4.0;
        }
        vel * (1.0 - dt * self.look().drag).max(0.0)
    }

    pub(super) fn rebound(self, vel: Vec3, contact: Vec3) -> Vec3 {
        if contact != Vec3::ZERO {
            let normal_speed = vel.dot(contact).min(0.0);
            let tangent = vel - contact * normal_speed;
            let slip = if self == Self::Frost { 0.78 } else { 0.44 };
            return tangent * slip - contact * normal_speed * self.look().rebound;
        }
        let lift = if self == Self::Plain { 0.35 } else { self.look().rebound * 0.36 };
        -vel * self.look().rebound + Vec3::Y * lift
    }

    pub(crate) fn flash_strength(self) -> f32 {
        match self {
            Self::Shock => 4.5,
            Self::Ember | Self::Spark | Self::Thread => 3.8,
            Self::Spore | Self::Acid | Self::Water => 1.6,
            _ => 2.8,
        }
    }
}
