//! One local reading for recovery, living air and sound. The existing voxel
//! probe runs off the frame thread; stale readings never follow a teleport.
use super::{LightingState, World, WorldSampleConfig, WorldType, adventure::affinity, lerp_color};
use crate::soundscape::{ambience::Surroundings, probe::Probe};
use engine_worldgen::landscape::Biome;
use glam::Vec3;
use std::time::Instant;

pub const NATURAL_DENSITY_MAX: f32 = 1.90;
pub const MANA_RECOVERY_MAX: f32 = 6.0;
pub const HEALTH_RECOVERY_MAX: f32 = 3.0;
pub const SAMPLE_LIFETIME_S: f32 = 3.0;
pub const SAMPLE_REACH_M: f32 = 16.0;
pub const TRANSITION_SPEED: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    pub natural_density_max: f32,
    pub mana_recovery_max: f32,
    pub health_recovery_max: f32,
    pub particles: f32,
    pub sparkles: bool,
    pub mist: f32,
    pub colour_grade: f32,
    pub transition_speed: f32,
}
impl Default for Tuning {
    fn default() -> Self {
        Self { natural_density_max: NATURAL_DENSITY_MAX, mana_recovery_max: MANA_RECOVERY_MAX,
            health_recovery_max: HEALTH_RECOVERY_MAX, particles: 1.0, sparkles: true,
            mist: 1.0, colour_grade: 1.0, transition_speed: TRANSITION_SPEED }
    }
}
static TUNING: std::sync::LazyLock<std::sync::RwLock<Tuning>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(Tuning::default()));
pub fn tuning() -> Tuning { *TUNING.read().unwrap() }
pub fn tune(value: Tuning) -> Result<(), String> {
    for (name, v, lo, hi) in [
        ("natural_density_max", value.natural_density_max, 1.05, 1.95),
        ("mana_recovery_max", value.mana_recovery_max, 1.0, 12.0),
        ("health_recovery_max", value.health_recovery_max, 1.0, 6.0),
        ("particles", value.particles, 0.0, 2.0), ("mist", value.mist, 0.0, 2.0),
        ("colour_grade", value.colour_grade, 0.0, 2.0),
        ("transition_speed", value.transition_speed, 0.1, 6.0),
    ] {
        if !v.is_finite() || !(lo..=hi).contains(&v) { return Err(format!("{name}: expected {lo}..{hi}")); }
    }
    *TUNING.write().unwrap() = value;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct Place {
    pub name: &'static str,
    /// Healthy abundance, not raw magical pressure. Always below poisoning.
    pub vitality: f32,
    /// Fireflies, pollen, ice crystals, embers.
    pub motes: [f32; 4],
    /// Petals, spores, water spray, dry dust. None of these glint.
    pub drift: [f32; 4],
    pub tint: [f32; 3],
    pub mist: f32,
    pub sparkles: f32,
}

impl Default for Place {
    fn default() -> Self {
        Self { name: "Still air", vitality: 0.0, motes: [0.0; 4], drift: [0.0; 4],
            tint: [1.0; 3], mist: 0.0, sparkles: 0.0 }
    }
}

fn biome_place(b: Biome) -> Place {
    use Biome::*;
    let (name, vitality, motes, drift, tint, mist, sparkles) = match b {
        Meadow => ("Meadow", 0.48, [0.40, 0.55, 0.0, 0.0], [0.30, 0.0, 0.0, 0.0], [1.04, 1.03, 0.94], 0.12, 0.12),
        Forest => ("Woodland", 0.64, [0.65, 0.30, 0.0, 0.0], [0.12, 0.08, 0.0, 0.0], [0.94, 1.04, 0.97], 0.28, 0.28),
        Taiga => ("Pine woods", 0.56, [0.22, 0.14, 0.15, 0.0], [0.0, 0.0, 0.0, 0.0], [0.94, 1.01, 1.07], 0.23, 0.15),
        Rainforest | CycadMarsh => ("Rainforest", 0.76, [0.75, 0.25, 0.0, 0.0], [0.20, 0.22, 0.0, 0.0], [0.91, 1.05, 0.99], 0.52, 0.30),
        CloudForest => ("Cloud forest", 0.84, [0.55, 0.20, 0.0, 0.0], [0.22, 0.16, 0.0, 0.0], [0.95, 1.04, 1.08], 0.65, 0.48),
        RedwoodGrove => ("Redwood grove", 0.80, [0.50, 0.25, 0.0, 0.0], [0.18, 0.12, 0.0, 0.0], [1.03, 1.02, 0.92], 0.40, 0.38),
        OvergrownGardens => ("Flower garden", 0.86, [0.65, 0.45, 0.0, 0.0], [0.70, 0.0, 0.0, 0.0], [1.05, 1.00, 0.97], 0.20, 0.55),
        Marsh | Fen => ("Wetland", 0.63, [0.90, 0.12, 0.0, 0.0], [0.0, 0.20, 0.0, 0.0], [0.93, 1.04, 1.01], 0.68, 0.24),
        FungalGrove => ("Fungal grove", 0.73, [0.28, 0.0, 0.0, 0.0], [0.0, 0.80, 0.0, 0.0], [0.98, 0.97, 1.08], 0.45, 0.38),
        Ocean | Coast | LastShore => ("Shore", 0.46, [0.12, 0.0, 0.0, 0.0], [0.0, 0.0, 0.30, 0.0], [0.96, 1.03, 1.08], 0.34, 0.12),
        Alpine | Tundra => ("Alpine air", 0.54, [0.0, 0.0, 0.70, 0.0], [0.0; 4], [0.96, 1.01, 1.10], 0.16, 0.25),
        LostPlateau | KarstUpland => ("Highland refuge", 0.66, [0.25, 0.25, 0.12, 0.0], [0.12, 0.0, 0.0, 0.0], [1.03, 1.02, 1.05], 0.18, 0.40),
        Heathland => ("Flower heath", 0.57, [0.30, 0.40, 0.0, 0.0], [0.45, 0.0, 0.0, 0.0], [1.05, 0.98, 1.03], 0.20, 0.20),
        Savanna => ("Dry grassland", 0.35, [0.18, 0.30, 0.0, 0.0], [0.0, 0.0, 0.0, 0.16], [1.06, 1.02, 0.92], 0.10, 0.0),
        Desert | Badlands => ("Dry air", 0.28, [0.0; 4], [0.0, 0.0, 0.0, 0.65], [1.06, 1.01, 0.93], 0.05, 0.0),
        VolcanicField => ("Ash air", 0.0, [0.0, 0.0, 0.0, 0.45], [0.0, 0.0, 0.0, 0.60], [1.04, 0.97, 0.94], 0.15, 0.0),
    };
    Place { name, vitality, motes, drift, tint, mist, sparkles }
}

/// Biome transitions blend in space. Nearby living materials add bounded
/// bonuses, so a planted refuge works without rewarding stacks of one flower.
pub fn place(cfg: &WorldSampleConfig, biomes: [Biome; 5], s: &Surroundings) -> Place {
    if matches!(cfg.world_type, WorldType::Hall | WorldType::Manyfold | WorldType::Realm(_)) {
        return Place::default();
    }
    if cfg.world_type == WorldType::Nether {
        return if cfg.is_hell() { biome_place(Biome::VolcanicField) } else { Place::default() };
    }
    if !s.grounded && s.underground < 0.1 && s.water < 0.1 { return Place::default(); }
    let canopy = s.canopy.iter().sum::<f32>() / 8.0;
    let [flowers, fungi, crystals, hot] = s.flora;
    let mut out = Place { name: biome_place(biomes[0]).name, tint: [0.0; 3], ..Place::default() };
    for (biome, w) in biomes.into_iter().zip([0.4, 0.15, 0.15, 0.15, 0.15]) {
        let p = biome_place(biome);
        out.vitality += p.vitality * w;
        out.mist += p.mist * w;
        out.sparkles += p.sparkles * w;
        for i in 0..4 { out.motes[i] += p.motes[i] * w; out.drift[i] += p.drift[i] * w; }
        for i in 0..3 { out.tint[i] += p.tint[i] * w; }
    }
    if s.underground > 0.55 {
        out = if crystals > 0.0 {
            Place { name: "Crystal grotto", vitality: 0.65 + crystals * 0.25,
                sparkles: 0.70, mist: 0.20, tint: [0.95, 1.02, 1.10], ..Place::default() }
        } else if fungi > 0.0 {
            Place { name: "Fungal hollow", vitality: 0.60 + fungi * 0.25,
                motes: [0.25, 0.0, 0.0, 0.0], drift: [0.0, 0.75, 0.0, 0.0],
                sparkles: 0.30, mist: 0.40, tint: [0.96, 1.04, 1.06] }
        } else {
            Place { name: "Stone cavern", ..Place::default() }
        };
    } else {
        // Real cover distinguishes a bare room from its regional biome, while
        // a canopy admits sheltered woodland ambience below the leaves.
        let outdoors = (s.sky + canopy * 1.4).clamp(0.0, 1.0);
        out.vitality *= outdoors;
        out.vitality += canopy * 0.16 + flowers * 0.30 + fungi * 0.16
            + s.water * 0.10 + s.water * s.water_motion * 0.10 + s.exposure * 0.10;
        out.drift[0] = (out.drift[0] + flowers * 0.65) * outdoors;
        out.drift[1] = (out.drift[1] + fungi * 0.55) * outdoors;
        // Spray comes from moving water or from surf on an open coast. Sea
        // water is static in the cell simulation, so the motion term alone left
        // every ocean shore without spray; the shore biomes' own weight keeps it,
        // stronger where water fills the view and the coast is exposed.
        let surf = out.drift[2] * (0.4 + 0.6 * s.water.min(1.0)) * (0.6 + 0.4 * s.exposure.min(1.0)) * 2.0;
        out.drift[2] = (s.water * s.water_motion * 0.80).max(surf).min(1.0) * outdoors;
        out.drift[3] *= outdoors;
        for mote in &mut out.motes { *mote *= outdoors; }
        out.mist *= outdoors;
        out.sparkles = (out.sparkles + flowers * 0.35 + canopy * s.water * 0.25) * outdoors;
        if flowers >= 0.35 { out.name = "Flower garden"; }
        else if s.water_motion > 0.65 && s.water > 0.35 { out.name = "Riverbank"; }
        else if out.vitality > 0.30 && biomes[0] == Biome::Desert { out.name = "Oasis"; }
    }
    // Hot material reduces the refuge bonus. Mere high mana never does.
    out.vitality = (out.vitality * (1.0 - hot)).clamp(0.0, 1.0);
    out.sparkles *= 1.0 - hot;
    out.motes[3] = out.motes[3].max(hot * 0.7);
    out
}

#[derive(Clone, Copy)]
pub struct Reading {
    pub position: Vec3,
    pub around: Surroundings,
    pub voice: crate::audio::Fauna,
    pub place: Place,
    pub ground_m: f32,
    pub label: &'static str,
    pub regional_air: [f32; 2],
    pub cosmos: f32,
}

#[derive(Default)]
pub struct State {
    probe: Probe,
    sample: Option<Reading>,
    config: Option<WorldSampleConfig>,
    position: Vec3,
    received: Option<Instant>,
    requested: Option<Instant>,
    pub shielded: bool,
}

impl State {
    pub fn update(&mut self, world: &mut World, eye_m: Vec3, shielded: bool) {
        self.shielded = shielded;
        let cfg = world.sample_config();
        if self.config != Some(cfg) || self.position.distance_squared(eye_m) > SAMPLE_REACH_M.powi(2)
            || self.received.is_none_or(|t| t.elapsed().as_secs_f32() > SAMPLE_LIFETIME_S) {
            self.sample = None;
        }
        if let Some(reading) = self.probe.poll(world, eye_m) {
            self.sample = Some(reading);
            self.config = Some(cfg);
            self.position = reading.position;
            self.received = Some(Instant::now());
        }
        if self.requested.is_none_or(|t| t.elapsed().as_secs_f32() >= crate::soundscape::ambience::PROBE_SECONDS) {
            self.requested = Some(Instant::now());
            self.probe.request(world, (eye_m / world.voxel_scale.meters()).floor().as_ivec3());
        }
    }

    pub fn reading(&self) -> Option<Reading> { self.sample }

    /// Presentation can run before this frame's gameplay refresh. Never carry
    /// a previous world's or a distant location's label/lighting across a jump.
    pub fn reading_at(&self, cfg: WorldSampleConfig, point_m: Vec3) -> Option<Reading> {
        if self.config != Some(cfg)
            || self.position.distance_squared(point_m) > SAMPLE_REACH_M.powi(2)
            || self.received.is_none_or(|t| t.elapsed().as_secs_f32() > SAMPLE_LIFETIME_S)
        { return None; }
        self.sample
    }

    pub fn density(&self, seed: u32, feet: Vec3) -> f32 {
        if self.shielded { return 1.0; }
        let Some(s) = self.sample else { return 1.0; };
        if s.place.name == "Still air" { return 1.0; }
        if s.place.vitality > 0.05 {
            1.0 + (tuning().natural_density_max - 1.0) * s.place.vitality
        } else {
            affinity::multiplier(seed, feet)
        }
    }
}

/// Presentation also runs in replicas and fixed-camera reviews. Refreshing
/// here never heals or damages a player, and wall-clock throttling shares the
/// gameplay probe rather than submitting a second terrain scan.
pub fn refresh(world: &mut World, player: &crate::player::Player) {
    let scale = world.voxel_scale.meters();
    let eye = player.precise_eye(world.voxel_scale) * f64::from(scale);
    let shielded = shielded(world, player, eye);
    let mut state = std::mem::take(&mut world.adventure.location);
    state.update(world, eye.as_vec3(), shielded);
    let density = state.density(world.seed, player.pos * scale);
    world.adventure.observe_mana_density(density);
    world.adventure.location = state;
}

pub fn shielded(world: &World, player: &crate::player::Player, eye: glam::DVec3) -> bool {
    crate::altitude::altitude_m(eye, crate::altitude::planet_world(world)) >= crate::altitude::ORBIT_M
        || super::orbital::habitat::readings(world, player).is_some_and(|(pressure, _)| {
            pressure >= super::orbital::habitat::MIN_BREATHING_PRESSURE_PA
        })
}

pub fn diagnostics(world: &World) -> String {
    let a = &world.adventure;
    let density = a.mana_density();
    let Some(s) = a.location.reading() else { return "Ambience: awaiting local sample · Mana regen: 1.0x".into(); };
    format!("{} · density {:.3} · mana regen {:.2}x · health regen {:.2}x · mana {:.2}/{:.2} · health {:.2}/{:.2} · poison {:.2}/s · vitality {:.3} · sky {:.2} · cave {:.2} · water {:.2}/{:.2} · flora {:?} · sparkles {:.2} · shielded {}",
        s.place.name, density, mana_recovery(density), health_recovery(density),
        a.mana, a.max_mana(), a.health, a.max_health(), affinity::poison_dps(density), s.place.vitality,
        s.around.sky, s.around.underground, s.around.water, s.around.water_motion, s.around.flora,
        s.place.sparkles, a.location.shielded)
}

/// Density and recovery deliberately use different scales. A rich forest is
/// below 2x pressure but can refill a wand six times as quickly.
pub fn mana_recovery(density: f32) -> f32 {
    let t = tuning();
    if density <= 1.0 { density.max(0.0) }
    else { 1.0 + (t.mana_recovery_max - 1.0) * ((density - 1.0) / (t.natural_density_max - 1.0)).min(1.4) }
}

pub fn health_recovery(density: f32) -> f32 {
    if density >= affinity::POISON_AT { return 0.0; }
    let t = tuning();
    1.0 + (t.health_recovery_max - 1.0) * ((density - 1.0) / (t.natural_density_max - 1.0)).clamp(0.0, 1.0)
}

#[derive(Default)]
pub struct Presentation {
    motes: [f32; 4],
    drift: [f32; 4],
    mana: [f32; 4],
    tint: [f32; 4],
}

impl Presentation {
    pub fn apply(&mut self, world: &World, lighting: &mut LightingState, underwater: bool, rain: f32,
        event: Option<super::mood::ActiveEvent>, dt: f32) {
        let state = &world.adventure.location;
        let tuning = tuning();
        let sample = state.reading();
        let realm_air = match world.world_type.realm() {
            Some(engine_voxel::RealmKind::Highfold) => Some(Place {
                name: "Highfold",
                motes: [0.0, 0.25, 0.32, 0.0],
                drift: [0.45, 0.0, 0.12, 0.0],
                mist: 0.12,
                ..Place::default()
            }),
            Some(engine_voxel::RealmKind::Gloaming) => Some(Place {
                name: "Gloaming",
                motes: [0.85, 0.0, 0.0, 0.0],
                drift: [0.0, 0.77, 0.0, 0.0],
                mist: 0.4,
                ..Place::default()
            }),
            _ => None,
        };
        let mut p = realm_air.unwrap_or_else(|| sample.map_or_else(Place::default, |s| s.place));
        let day = lighting.daylight;
        let night = 1.0 - day;
        let winter = ((lighting.foliage_tint[2] - lighting.foliage_tint[0] - 0.10) / 0.20).clamp(0.0, 1.0);
        let rain = ((rain - 0.10) / 0.65).clamp(0.0, 1.0);
        let cave = sample.is_some_and(|s| s.around.underground > 0.55);
        p.motes[0] *= if cave { 1.0 } else { night * night * (1.0 - winter) * (1.0 - rain) };
        p.motes[1] *= day * (1.0 - winter) * (1.0 - rain);
        p.motes[2] *= 0.3 + 0.7 * day;
        p.drift[0] *= (0.25 + 0.75 * day) * (1.0 - winter) * (1.0 - rain);
        p.drift[3] *= 1.0 - rain;
        if !cave && let Some(event) = event.filter(|e| matches!(e.kind,
            super::mood::SkyEvent::BloodMoon | super::mood::SkyEvent::Emberglow)) {
            p.motes[3] = p.motes[3].max(event.intensity * 0.8);
        }
        let density = world.adventure.mana_density();
        let mut mana = [p.sparkles * p.vitality, 0.0, 0.0, p.mist * (0.4 + p.vitality * 0.6)];
        if density < affinity::BLIGHT_BELOW {
            mana[1] = ((affinity::BLIGHT_BELOW - density) * 8.0).clamp(0.0, 1.0);
        } else if density >= affinity::POISON_AT {
            mana[2] = (0.4 + density - affinity::POISON_AT).min(1.0);
        }
        let suppressed = underwater || state.shielded || (sample.is_none() && realm_air.is_none());
        for weight in p.motes.iter_mut().chain(p.drift.iter_mut()) { *weight *= tuning.particles; }
        for weight in &mut mana[..3] { *weight *= tuning.particles; }
        if !tuning.sparkles { mana[0] = 0.0; p.motes[2] = 0.0; self.mana[0] = 0.0; self.motes[2] = 0.0; }
        mana[3] *= tuning.mist;
        let mut tint = [p.tint[0], p.tint[1], p.tint[2], p.vitality * 0.45 * tuning.colour_grade];
        if suppressed {
            p.motes = [0.0; 4]; p.drift = [0.0; 4]; mana = [0.0; 4]; tint = [1.0, 1.0, 1.0, 0.0];
            // Clear immediately on invalidation, submersion or realm travel.
            self.motes = [0.0; 4]; self.drift = [0.0; 4]; self.mana = [0.0; 4]; self.tint = tint;
        }
        let k = 1.0 - (-dt.max(0.0) * tuning.transition_speed).exp();
        for i in 0..4 {
            self.motes[i] += (p.motes[i] - self.motes[i]) * k;
            self.drift[i] += (p.drift[i] - self.drift[i]) * k;
            self.mana[i] += (mana[i] - self.mana[i]) * k;
            self.tint[i] += (tint[i] - self.tint[i]) * k;
        }
        lighting.ambience = self.motes;
        lighting.biome_ambience = self.drift;
        lighting.mana_ambience = self.mana;
        lighting.location_mood = self.tint;
        let grade = |c: [f32; 3]| lerp_color(c, [c[0] * self.tint[0], c[1] * self.tint[1], c[2] * self.tint[2]], self.tint[3]);
        lighting.ambient_color = grade(lighting.ambient_color);
        lighting.fog_color = grade(lighting.fog_color);
    }
}
