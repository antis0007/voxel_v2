//! Versioned preferences with bounded decoding and a debounced, atomic background writer.

use crate::{
    egui_ui::EguiUi,
    fluid::VoxelScalePreset,
    render::{self, AtmosphereMode, RenderDistanceSettings},
    world::{WorldType, storage},
};
use anyhow::{Result, ensure};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAGIC: &[u8; 8] = b"VXPRF008";
const WORDS: usize = 26 + crate::egui_ui::magic_style::WORDS;
const CLOUDS_OFF_BIT: u32 = 8;
/// Bit 9 of the same word: automatic quality switched off (so old files keep it on).
const AUTO_QUALITY_OFF_BIT: u32 = 9;
/// Bits 10-17 of the same word: render resolution in percent (0 = unset = 100).
const RENDER_SCALE_SHIFT: u32 = 10;
fn path() -> PathBuf {
    storage::save_directory().join("preferences.vxp")
}
fn hash(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf29ce484222325, |h, &v| {
        (h ^ v as u64).wrapping_mul(0x100000001b3)
    })
}
/// UI scale bounds and default. The old default of 1.0 was too small for the
/// recipe, production and factory screens on a 720p window, so 1.0 saved by an
/// older build is treated as unchosen and migrates to the new default.
pub const UI_SCALE_MIN: f32 = 0.75;
pub const UI_SCALE_MAX: f32 = 2.5;
pub const UI_SCALE_DEFAULT: f32 = 1.5;

#[derive(Clone, Copy, PartialEq)]
pub struct Preferences {
    pub fov: f32,
    pub sensitivity: f32,
    pub invert: bool,
    pub hud: bool,
    pub vsync: bool,
    pub fxaa: bool,
    pub atmosphere: u32,
    pub view: i32,
    pub lod: i32,
    pub levels: u8,
    pub scale: u32,
    pub world: u32,
    pub cpu: usize,
    pub gpu: usize,
    pub mesh: usize,
    pub edited: usize,
    pub autosave: bool,
    pub interval: usize,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub effects_volume: f32,
    pub cloud_scale: u32,
    /// Stored as bit 8 of the cloud-scale word; older builds clamp that word to Balanced.
    pub clouds: bool,
    pub indirect_lighting: bool,
    /// Step settings down on its own when frames run slow.
    pub auto_quality: bool,
    /// Internal render resolution as a percentage of the window (50 to 100).
    pub render_scale: u32,
    /// Menu and HUD zoom on top of the native DPI correction.
    pub ui_scale: f32,
    pub hotbar_scale: f32,
    pub magic: crate::egui_ui::magic_style::Settings,
}
impl Default for Preferences {
    fn default() -> Self {
        let d = render::render_distance_settings();
        let (cpu, _) = render::world_residency_budgets_mib();
        let gpu = 0; // Automatic, resolved after selecting the adapter.
        Self {
            fov: 70.0,
            sensitivity: 0.0025,
            invert: false,
            hud: true,
            vsync: true,
            fxaa: true,
            atmosphere: 2,
            view: d.view_distance_chunks,
            lod: d.lod_start_chunks,
            levels: d.lod_levels,
            scale: 0,
            world: 1,
            cpu,
            gpu,
            mesh: render::mesh_budget_mib(),
            edited: crate::world::regions::edited_budget_mib(),
            autosave: true,
            interval: 30,
            width: 1280,
            height: 720,
            fullscreen: false,
            effects_volume: 0.65,
            cloud_scale: 3,
            clouds: true,
            indirect_lighting: true,
            auto_quality: true,
            render_scale: 100,
            ui_scale: UI_SCALE_DEFAULT,
            hotbar_scale: 1.0,
            magic: Default::default(),
        }
    }
}
impl Preferences {
    pub fn kind(self) -> WorldType {
        match self.world {
            0 => WorldType::Superflat,
            2 => WorldType::Ridged,
            _ => WorldType::Hills,
        }
    }
    pub fn voxel_scale(self) -> VoxelScalePreset {
        match self.scale {
            1 => VoxelScalePreset::HalfMeter,
            2 => VoxelScalePreset::QuarterMeter,
            _ => VoxelScalePreset::OneMeter,
        }
    }
    pub fn atmosphere(self) -> AtmosphereMode {
        AtmosphereMode::all()
            .get(self.atmosphere as usize)
            .copied()
            .unwrap_or_default()
    }
    pub fn apply_globals(self) {
        crate::audio::set_volume(self.effects_volume);
        render::set_cloud_render_scale(self.cloud_scale);
        render::set_clouds_enabled(self.clouds);
        render::set_indirect_lighting_enabled(self.indirect_lighting);
        crate::quality::set_auto_enabled(self.auto_quality);
        render::set_scene_scale_percent(self.render_scale);
        // The old stock setting kept full blocks for three chunks and then
        // threw the rest away at 256 m. Promote that exact preset once.
        let (view, lod, levels) = if self.view == 16 && self.lod == 3 && self.levels == 3 {
            (28, 10, 4)
        } else {
            (self.view, self.lod, self.levels)
        };
        render::set_render_distance_settings(RenderDistanceSettings {
            view_distance_chunks: view,
            lod_start_chunks: lod,
            lod_levels: levels,
        });
        render::set_world_residency_budgets_mib(self.cpu, self.gpu);
        render::set_mesh_budget_mib(self.mesh);
        crate::world::regions::set_edited_budget_mib(self.edited);
        storage::set_autosave_enabled(self.autosave);
        storage::set_autosave_seconds(self.interval);
    }
    pub fn apply_ui(self, ui: &mut EguiUi) {
        ui.fov_degrees = self.fov;
        ui.mouse_sensitivity = self.sensitivity;
        ui.invert_mouse = self.invert;
        ui.show_hud = self.hud;
        ui.ui_scale = self.ui_scale;
        ui.hotbar_scale = self.hotbar_scale;
        crate::egui_ui::magic_style::set(&ui.ctx, self.magic);
    }
    pub fn capture(
        ui: &EguiUi,
        vsync: bool,
        fxaa: bool,
        atmosphere: AtmosphereMode,
        kind: WorldType,
        scale: VoxelScalePreset,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Self {
        let d = render::render_distance_settings();
        let (cpu, _) = render::world_residency_budgets_mib();
        let gpu = render::gpu_residency_limit_mib();
        Self {
            fov: ui.fov_degrees,
            sensitivity: ui.mouse_sensitivity,
            invert: ui.invert_mouse,
            hud: ui.show_hud,
            vsync,
            fxaa,
            atmosphere: atmosphere.as_u32(),
            view: d.view_distance_chunks,
            lod: d.lod_start_chunks,
            levels: d.lod_levels,
            world: match kind {
                WorldType::Superflat => 0,
                WorldType::Hills => 1,
                WorldType::Ridged => 2,
            },
            scale: match scale {
                VoxelScalePreset::OneMeter => 0,
                VoxelScalePreset::HalfMeter => 1,
                VoxelScalePreset::QuarterMeter => 2,
            },
            cpu,
            gpu,
            mesh: render::mesh_budget_mib(),
            edited: crate::world::regions::edited_budget_mib(),
            autosave: storage::autosave_enabled(),
            interval: storage::autosave_seconds(),
            width,
            height,
            fullscreen,
            effects_volume: crate::audio::volume(),
            cloud_scale: render::cloud_render_scale(),
            clouds: render::clouds_enabled(),
            indirect_lighting: render::indirect_lighting_enabled(),
            auto_quality: crate::quality::auto_enabled(),
            render_scale: render::scene_scale_percent(),
            ui_scale: ui.ui_scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX),
            hotbar_scale: ui.hotbar_scale.clamp(0.7, 1.6),
            magic: crate::egui_ui::magic_style::get(&ui.ctx),
        }
    }
    fn words(self) -> [u32; WORDS] {
        let base = [
            self.fov.to_bits(),
            self.sensitivity.to_bits(),
            self.invert as u32,
            self.hud as u32,
            self.vsync as u32,
            self.fxaa as u32,
            self.atmosphere,
            self.view as u32,
            self.lod as u32,
            self.levels as u32,
            self.scale,
            self.world,
            self.cpu as u32,
            self.gpu as u32,
            self.mesh as u32,
            self.edited as u32,
            self.autosave as u32,
            self.interval as u32,
            self.width,
            self.height,
            self.fullscreen as u32,
            self.effects_volume.to_bits(),
            self.cloud_scale
                | u32::from(!self.clouds) << CLOUDS_OFF_BIT
                | u32::from(!self.auto_quality) << AUTO_QUALITY_OFF_BIT
                | self.render_scale.clamp(render::MIN_SCENE_SCALE_PERCENT, 100) << RENDER_SCALE_SHIFT,
            self.indirect_lighting as u32,
            self.ui_scale.to_bits(),
            self.hotbar_scale.to_bits(),
        ];
        let mut words = [0; WORDS];
        words[..26].copy_from_slice(&base);
        words[26..].copy_from_slice(&self.magic.words());
        words
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            (bytes.len() == 8 + WORDS * 4 + 8 && bytes.get(..8) == Some(MAGIC.as_slice()))
                || (bytes.len() == 8 + 26 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF007".as_slice()))
                || (bytes.len() == 8 + 24 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF005".as_slice()))
                || (bytes.len() == 8 + 25 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF006".as_slice()))
                || (bytes.len() == 8 + 24 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF004".as_slice()))
                || (bytes.len() == 8 + 23 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF003".as_slice()))
                || (bytes.len() == 8 + 22 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF002".as_slice()))
                || (bytes.len() == 8 + 21 * 4 + 8
                    && bytes.get(..8) == Some(b"VXPRF001".as_slice())),
            "Unsupported preferences format"
        );
        let end = bytes.len() - 8;
        ensure!(
            hash(&bytes[..end]) == u64::from_le_bytes(bytes[end..].try_into()?),
            "Preferences checksum mismatch"
        );
        let w: Vec<_> = bytes[8..end]
            .chunks_exact(4)
            .map(|n| u32::from_le_bytes(n.try_into().unwrap()))
            .collect();
        let fov = f32::from_bits(w[0]);
        let sensitivity = f32::from_bits(w[1]);
        ensure!(
            fov.is_finite() && sensitivity.is_finite(),
            "Invalid input preferences"
        );
        let effects_volume = w.get(21).map_or(0.65, |n| f32::from_bits(*n));
        ensure!(effects_volume.is_finite(), "Invalid audio preferences");
        let view = w[7].clamp(4, 64) as i32;
        Ok(Self {
            fov: fov.clamp(50.0, 110.0),
            sensitivity: sensitivity.clamp(0.0005, 0.006),
            invert: w[2] != 0,
            hud: w[3] != 0,
            vsync: w[4] != 0,
            fxaa: w[5] != 0,
            atmosphere: w[6].min(2),
            view,
            lod: (w[8] as i32).clamp(2, view - 1),
            levels: w[9].clamp(1, 4) as u8,
            scale: w[10].min(2),
            world: w[11].min(2),
            cpu: w[12].clamp(32, 2048) as usize,
            // Migrate the former default, otherwise old preferences would
            // silently pin upgraded installations to the broken 512 MiB cap.
            gpu: if w[13] == 0
                || (bytes.get(..8) != Some(MAGIC.as_slice())
                    && bytes.get(..8) != Some(b"VXPRF007".as_slice())
                    && bytes.get(..8) != Some(b"VXPRF006".as_slice())
                    && bytes.get(..8) != Some(b"VXPRF005".as_slice())
                    && w[13] == 512)
            {
                0
            } else {
                w[13].clamp(64, render::MAX_GPU_MIB as u32) as usize
            },
            mesh: w[14].clamp(160, 2048) as usize,
            edited: w[15].clamp(16, 1024) as usize,
            autosave: w[16] != 0,
            interval: w[17].clamp(10, 300) as usize,
            width: w[18].clamp(640, 7680),
            height: w[19].clamp(480, 4320),
            fullscreen: w[20] != 0,
            effects_volume: effects_volume.clamp(0.0, 1.0),
            cloud_scale: w.get(22).map_or(3, |n| n & 0xff).clamp(2, 4),
            clouds: w.get(22).is_none_or(|n| n >> CLOUDS_OFF_BIT & 1 == 0),
            auto_quality: w.get(22).is_none_or(|n| n >> AUTO_QUALITY_OFF_BIT & 1 == 0),
            render_scale: w
                .get(22)
                .map(|n| n >> RENDER_SCALE_SHIFT & 0xff)
                .filter(|p| *p != 0)
                .map_or(100, |p| p.clamp(render::MIN_SCENE_SCALE_PERCENT, 100)),
            indirect_lighting: w.get(23).is_none_or(|v| *v != 0),
            ui_scale: w
                .get(24)
                .map(|n| f32::from_bits(*n))
                // 1.0 was the old default, so it counts as unchosen.
                .filter(|n| n.is_finite() && (*n - 1.0).abs() > 1.0e-6)
                .unwrap_or(UI_SCALE_DEFAULT)
                .clamp(UI_SCALE_MIN, UI_SCALE_MAX),
            hotbar_scale: w
                .get(25)
                .map(|n| f32::from_bits(*n))
                .filter(|n| n.is_finite())
                .unwrap_or(1.0)
                .clamp(0.7, 1.6),
            magic: crate::egui_ui::magic_style::Settings::decode(w.get(26..).unwrap_or(&[])),
        })
    }
    fn bytes(self) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        for w in self.words() {
            b.extend_from_slice(&w.to_le_bytes());
        }
        b.extend_from_slice(&hash(&b).to_le_bytes());
        b
    }
}
fn read(path: &std::path::Path) -> Result<(Preferences, Vec<u8>)> {
    let mut b = Vec::new();
    fs::File::open(path)?.take(1025).read_to_end(&mut b)?;
    ensure!(b.len() <= 1024, "Preferences exceed size limit");
    Ok((Preferences::decode(&b)?, b))
}

fn write(value: Preferences) -> Result<()> {
    fs::create_dir_all(storage::save_directory())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(storage::save_directory().join(".preferences-lock"))?;
    lock.lock()?;
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let atomic = |path: PathBuf, bytes: &[u8], tag: &str| -> Result<()> {
        let temp = path.with_extension(format!("{}-{serial}-{tag}.tmp", std::process::id()));
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(temp, path)?;
        Ok(())
    };
    if let Ok((_, old)) = read(&path()) {
        atomic(path().with_extension("prev.vxp"), &old, "previous")?;
    }
    atomic(path(), &value.bytes(), "current")
}
pub struct PreferenceStore {
    pub loaded: Preferences,
    /// The player has a saved preferences file, so first-run hardware
    /// defaults (see `low_spec::apply_integrated`) must not override it.
    pub from_file: bool,
    pub status: Option<String>,
    failed: bool,
    tx: Option<mpsc::SyncSender<Preferences>>,
    rx: mpsc::Receiver<(Preferences, Result<()>)>,
    wanted: Preferences,
    saved: Preferences,
    in_flight: bool,
    last_change: Instant,
    retry: Instant,
    enabled: bool,
}
impl PreferenceStore {
    pub fn new(enabled: bool) -> Self {
        let (mut loaded, mut status) = (Preferences::default(), None);
        let mut from_file = false;
        let load_path = storage::preference_read_path();
        if enabled && (load_path.is_file() || load_path.with_extension("prev.vxp").is_file()) {
            match read(&load_path) {
                Ok((p, _)) => (loaded, from_file) = (p, true),
                Err(error) => match read(&load_path.with_extension("prev.vxp")) {
                    Ok((p, _)) => {
                        (loaded, from_file) = (p, true);
                        status = Some("Recovered your previous preferences".into());
                    }
                    Err(_) => {
                        status = Some(format!(
                            "Preferences could not be read; defaults are active: {error:#}"
                        ))
                    }
                },
            }
        }
        let (tx, jobs) = mpsc::sync_channel(1);
        let (done, rx) = mpsc::sync_channel(1);
        let tx = if enabled {
            match std::thread::Builder::new()
                .name("preferences".into())
                .spawn(move || {
                    while let Ok(p) = jobs.recv() {
                        if done.send((p, write(p))).is_err() {
                            break;
                        }
                    }
                }) {
                Ok(_) => Some(tx),
                Err(e) => {
                    status = Some(format!("Cannot start preferences writer: {e}"));
                    None
                }
            }
        } else {
            None
        };
        Self {
            loaded,
            from_file,
            status,
            failed: false,
            tx,
            rx,
            wanted: loaded,
            saved: loaded,
            in_flight: false,
            last_change: Instant::now(),
            retry: Instant::now(),
            enabled,
        }
    }
    pub fn busy(&self) -> bool {
        self.enabled && (self.in_flight || (!self.failed && self.wanted != self.saved))
    }
    pub fn tick(&mut self, value: Preferences, flush: bool) {
        if !self.enabled {
            return;
        }
        if value != self.wanted {
            self.wanted = value;
            self.last_change = Instant::now();
        }
        if self.in_flight {
            match self.rx.try_recv() {
                Ok((p, Ok(()))) => {
                    self.saved = p;
                    self.in_flight = false;
                    self.failed = false;
                    self.status = None;
                }
                Ok((_, Err(e))) => {
                    self.in_flight = false;
                    self.failed = true;
                    self.retry = Instant::now() + Duration::from_secs(5);
                    self.status = Some(format!("Cannot save preferences: {e:#}"));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.in_flight = false;
                    self.failed = true;
                    self.retry = Instant::now() + Duration::from_secs(5);
                    self.status = Some("Preferences writer stopped".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if !self.in_flight
            && self.wanted != self.saved
            && Instant::now() >= self.retry
            && (flush || self.last_change.elapsed() >= Duration::from_millis(500))
        {
            if self
                .tx
                .as_ref()
                .is_some_and(|tx| tx.try_send(self.wanted).is_ok())
            {
                self.in_flight = true;
            } else {
                self.status = Some("Preferences writer unavailable".into());
                self.failed = true;
                self.retry = Instant::now() + Duration::from_secs(5);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scale_round_trips() {
        let mut preferences = Preferences::default();
        preferences.ui_scale = 1.35;
        let bytes = preferences.bytes();
        let decoded = Preferences::decode(&bytes).expect("preferences must decode");
        assert!((decoded.ui_scale - 1.35).abs() < 1.0e-6);
    }

    #[test]
    fn version_five_preferences_default_to_full_scale() {
        let bytes = Preferences::default().bytes();
        let mut body = bytes[..8 + 24 * 4].to_vec();
        body[..8].copy_from_slice(b"VXPRF005");
        body.extend_from_slice(&hash(&body).to_le_bytes());
        let decoded = Preferences::decode(&body).expect("legacy preferences must decode");
        assert_eq!(decoded.ui_scale, UI_SCALE_DEFAULT);
    }

    #[test]
    fn a_saved_one_point_zero_scale_migrates_to_the_default() {
        let mut preferences = Preferences::default();
        preferences.ui_scale = 1.0;
        let decoded = Preferences::decode(&preferences.bytes()).expect("preferences must decode");
        assert_eq!(decoded.ui_scale, UI_SCALE_DEFAULT);
    }

    #[test]
    fn clouds_off_round_trips_beside_the_cloud_scale() {
        let mut preferences = Preferences::default();
        preferences.clouds = false;
        preferences.cloud_scale = 4;
        let decoded = Preferences::decode(&preferences.bytes()).expect("preferences must decode");
        assert!(!decoded.clouds);
        assert_eq!(decoded.cloud_scale, 4);
        let decoded = Preferences::decode(&Preferences::default().bytes()).expect("decode");
        assert!(decoded.clouds);
        assert_eq!(decoded.cloud_scale, 3);
    }

    #[test]
    fn render_scale_defaults_to_full_and_round_trips_beside_the_other_flags() {
        assert_eq!(Preferences::default().render_scale, 100);
        let mut preferences = Preferences::default();
        preferences.render_scale = 70;
        preferences.clouds = false;
        preferences.auto_quality = false;
        preferences.cloud_scale = 4;
        let decoded = Preferences::decode(&preferences.bytes()).expect("preferences must decode");
        assert_eq!(decoded.render_scale, 70);
        assert!(!decoded.clouds && !decoded.auto_quality);
        assert_eq!(decoded.cloud_scale, 4);
        // An older file has no scale in those bits: it reads as full resolution.
        let mut old = Preferences::default();
        old.render_scale = 100;
        let mut bytes = old.bytes();
        let word = 8 + 22 * 4;
        bytes[word..word + 4].copy_from_slice(&3u32.to_le_bytes());
        let end = bytes.len() - 8;
        let checksum = hash(&bytes[..end]).to_le_bytes();
        bytes[end..].copy_from_slice(&checksum);
        assert_eq!(Preferences::decode(&bytes).expect("decode").render_scale, 100);
    }

    #[test]
    fn automatic_quality_defaults_on_and_round_trips_beside_clouds() {
        assert!(Preferences::default().auto_quality);
        let mut preferences = Preferences::default();
        preferences.auto_quality = false;
        preferences.clouds = false;
        preferences.cloud_scale = 4;
        let decoded = Preferences::decode(&preferences.bytes()).expect("preferences must decode");
        assert!(!decoded.auto_quality && !decoded.clouds);
        assert_eq!(decoded.cloud_scale, 4);
        let decoded = Preferences::decode(&Preferences::default().bytes()).expect("decode");
        assert!(decoded.auto_quality && decoded.clouds);
    }

    #[test]
    fn out_of_range_ui_scale_is_clamped() {
        let mut preferences = Preferences::default();
        preferences.ui_scale = 9.0;
        let decoded = Preferences::decode(&preferences.bytes()).expect("preferences must decode");
        assert_eq!(decoded.ui_scale, UI_SCALE_MAX);
    }
}
