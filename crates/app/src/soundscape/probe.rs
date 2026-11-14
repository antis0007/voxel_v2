use super::{ambience, fauna};
use crate::audio::Fauna;
use crate::world::{World, WorldSnapshot};
use glam::{IVec3, Vec3};
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use crate::world::ambience::{Reading, SAMPLE_REACH_M};
use engine_worldgen::landscape::Biome;

#[derive(Default)]
pub(crate) struct Probe {
    pending: Option<Mutex<mpsc::Receiver<Result>>>,
}

struct Job {
    world: WorldSnapshot,
    eye: IVec3,
    reply: mpsc::SyncSender<Result>,
    started: std::time::Instant,
}

struct Result {
    root: Arc<std::path::PathBuf>,
    config: engine_worldgen::WorldSampleConfig,
    eye: IVec3,
    terrain: ambience::Sample,
    fauna: Fauna,
    biomes: [Biome; 5],
    ground_m: f32,
    label: &'static str,
    regional_air: [f32; 2],
    cosmos: f32,
    started: std::time::Instant,
}

fn lane() -> Option<&'static mpsc::SyncSender<Job>> {
    static LANE: OnceLock<Option<mpsc::SyncSender<Job>>> = OnceLock::new();
    LANE.get_or_init(|| {
        let (send, receive) = mpsc::sync_channel::<Job>(8);
        std::thread::Builder::new().name("ambience-probe".into()).spawn(move || {
            while let Ok(job) = receive.recv() {
                let _ = job.reply.try_send(sample(&job.world, job.eye, job.started));
            }
        }).ok()?;
        Some(send)
    }).as_ref()
}

impl Probe {
    pub(crate) fn request(&mut self, world: &mut World, eye: IVec3) {
        if self.pending.is_some() { return; }
        let Some(lane) = lane() else { return };
        let (reply, receive) = mpsc::sync_channel(1);
        if lane.try_send(Job { world: world.snapshot(), eye, reply, started: std::time::Instant::now() }).is_ok() {
            self.pending = Some(Mutex::new(receive));
        }
    }

    pub(crate) fn poll(&mut self, world: &World, eye_m: Vec3) -> Option<Reading> {
        let result = self.pending.as_ref()?.lock().ok()?.try_recv();
        if matches!(&result, Err(mpsc::TryRecvError::Empty)) { return None; }
        self.pending = None;
        let result = result.ok()?;
        let cfg = world.sample_config();
        let meters = cfg.voxel_meters();
        // Keep a late sample from a previous world or a distant listener
        // from replacing the local sound beds after a crossing or teleport.
        if result.started.elapsed().as_secs_f32() > crate::world::ambience::SAMPLE_LIFETIME_S
            || result.config != cfg
            || !Arc::ptr_eq(&result.root, &world.regions.view.root)
            || (result.eye.as_vec3() * meters).distance_squared(eye_m) > SAMPLE_REACH_M * SAMPLE_REACH_M
        {
            return None;
        }
        let mut around = result.terrain.with_flow(|p| world.water_velocity(p).length());
        around.altitude_m = eye_m.y - cfg.water_level() as f32 * meters;
        Some(Reading { position: result.eye.as_vec3() * meters,
            place: crate::world::ambience::place(&cfg, result.biomes, &around), around, voice: result.fauna,
            ground_m: result.ground_m, label: result.label, regional_air: result.regional_air, cosmos: result.cosmos })
    }
}

fn sample(world: &WorldSnapshot, eye: IVec3, started: std::time::Instant) -> Result {
    let mut trace = crate::gameplay_trace::Trace::new();
    let cfg = world.cfg;
    let meters = cfg.voxel_meters();
    let read = RefCell::new(world.voxel_reader());
    let get = |p| read.borrow_mut()(p);
    let terrain = ambience::sample(get, eye, meters, cfg.water_level() as f32 * meters);
    trace.mark("soundscape-worker-voxels");
    let reach = (fauna::BLEND_REACH_M / meters).round() as i32;
    let biomes = [(0, 0), (reach, 0), (-reach, 0), (0, reach), (0, -reach)]
        .map(|(dx, dz)| crate::world::landscape::sample(&cfg, eye.x + dx, eye.z + dz).biome);
    let names = biomes.map(|b| format!("{b:?}"));
    let fauna = fauna::blended([&names[0], &names[1], &names[2], &names[3], &names[4]]);
    trace.mark("soundscape-worker-biomes");
    // Labels, cave grading and regional air used to repeat these cold queries
    // on the UI/render thread. Share the same bounded, position-checked sample
    // as ambience rather than generating remote terrain during an orbital pass.
    let point_m = eye.as_vec3() * meters;
    let ground_m = crate::world::terrain_height(&cfg, eye.x, eye.z) as f32 * meters;
    let weights = crate::world::biome_weights(&cfg, eye.x, eye.z);
    let cosmos = crate::world::cosmos_at(&cfg, point_m);
    let label = engine_worldgen::civilization::location(&cfg, eye.as_vec3())
        .or_else(|| crate::world::sites::location(&cfg, eye.as_vec3()))
        .unwrap_or_else(|| {
            if cfg.generator_version < 2 { "Legacy landscape" }
            else if cfg.world_type == engine_worldgen::WorldType::Superflat { "Building plains" }
            else { engine_worldgen::basin::planet_label(&cfg, eye.x, eye.z).unwrap_or(biomes[0].label()) }
        });
    trace.mark("soundscape-worker-presentation");
    Result { root: world.regions.root.clone(), config: cfg, eye, terrain, fauna, biomes, started,
        ground_m, label, regional_air: [weights.desert, weights.snow], cosmos }
}
