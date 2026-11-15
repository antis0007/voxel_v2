use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, Painter, Pos2, Stroke, vec2};
use engine_voxel::WorldSampleConfig;
use engine_worldgen::civilization::regional::{self, RegionalSurvey};
use glam::{Vec2, Vec3};

const MAP_SIDE: usize = 48;

pub struct Map {
    terrain: Vec<Color32>,
    survey: RegionalSurvey,
}

impl Map {
    pub fn settlement_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.survey.settlements.iter().map(|site| site.id)
    }
}

pub type Slot = Arc<OnceLock<Map>>;
static SURVEY: Mutex<Option<(u64, Slot)>> = Mutex::new(None);

pub fn request(cfg: WorldSampleConfig, center: Vec3, key: u64) -> Option<Slot> {
    if cfg.generator_version < engine_voxel::REGIONAL_CIVILIZATION_GENERATOR
        || !engine_worldgen::civilization::enabled(&cfg)
    {
        return None;
    }
    let mut current = SURVEY.lock().ok()?;
    if let Some((_, survey)) = current.as_ref().filter(|(id, _)| *id == key) {
        return Some(Arc::clone(survey));
    }
    let survey = Arc::new(OnceLock::new());
    let worker = Arc::clone(&survey);
    std::thread::Builder::new().name("regional-loading-survey".into()).spawn(move || {
        let step = 2.0 * regional::RADIUS_M / MAP_SIDE as f32;
        let mut terrain = Vec::with_capacity(MAP_SIDE * MAP_SIDE);
        for z in 0..MAP_SIDE {
            for x in 0..MAP_SIDE {
                let at = center + Vec3::new(
                    (x as f32 + 0.5) * step - regional::RADIUS_M, 0.0,
                    (z as f32 + 0.5) * step - regional::RADIUS_M,
                );
                let scale = cfg.inv_voxel_meters();
                let column = engine_worldgen::sample_column(&cfg, (at.x * scale).floor() as i32, (at.z * scale).floor() as i32);
                let tint = if column.height < cfg.water_level() {
                    Color32::from_rgb(27, 63, 82)
                } else {
                    let [r, g, b] = column.surface.color(glam::IVec3::ZERO);
                    Color32::from_rgb((r * 125.0) as u8, (g * 125.0) as u8, (b * 125.0) as u8)
                };
                terrain.push(tint);
            }
        }
        let _ = worker.set(Map { terrain, survey: regional::survey(&cfg, center) });
    }).ok()?;
    *current = Some((key, Arc::clone(&survey)));
    Some(survey)
}

#[derive(Clone)]
pub struct View {
    pub origin: Vec2,
    pub map: Slot,
    pub populations: std::collections::BTreeMap<u32, usize>,
}

pub fn draw(p: &Painter, center: Pos2, radius: f32, view: &View, map: &Map) {
    let origin = view.origin;
    let survey = &map.survey;
    let radius = (radius * 0.37).min(112.0);
    let rect = egui::Rect::from_center_size(center, vec2(radius * 2.0, radius * 2.0));
    let p = p.with_clip_rect(rect);
    let project = |at: Vec2| center + vec2(at.x - origin.x, at.y - origin.y) * (radius / regional::RADIUS_M);
    let step = rect.width() / MAP_SIDE as f32;
    let mut mesh = egui::Mesh::default();
    for (index, &color) in map.terrain.iter().enumerate() {
        let pos = rect.min + vec2((index % MAP_SIDE) as f32, (index / MAP_SIDE) as f32) * step;
        mesh.add_colored_rect(egui::Rect::from_min_size(pos, vec2(step, step)), color);
    }
    p.add(egui::Shape::mesh(mesh));
    let roads = Color32::from_rgb(141, 126, 97);
    let living = Color32::from_rgb(142, 224, 174);
    let ruins = Color32::from_rgb(242, 186, 100);
    for path in &survey.roads {
        for edge in path.windows(2) {
            p.line_segment([project(edge[0]), project(edge[1])], Stroke::new(1.1, roads));
        }
    }
    for city in &survey.cities {
        p.rect_stroke(egui::Rect::from_center_size(project(*city), vec2(7.0, 7.0)), 1.0,
            Stroke::new(1.4, ruins), egui::StrokeKind::Inside);
    }
    for site in &survey.settlements {
        let point = project(Vec2::new(site.center.x, site.center.z));
        let population = view.populations.get(&site.id).copied().unwrap_or(site.history.households as usize);
        if population > 0 {
            p.circle_filled(point, (2.0 + (population as f32).sqrt() * 0.6).min(8.0), living);
        } else {
            p.circle_stroke(point, 2.0, Stroke::new(1.0, ruins));
        }
    }
    p.circle_stroke(center, 4.0, Stroke::new(1.0, Color32::WHITE));
    p.text(rect.left_top() + vec2(8.0, 6.0), egui::Align2::LEFT_TOP,
        "Regional civilization", egui::FontId::proportional(12.0), Color32::WHITE);
    p.text(rect.left_bottom() + vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM,
        "Settlements · city ruins", egui::FontId::proportional(10.0), Color32::LIGHT_GRAY);
}
