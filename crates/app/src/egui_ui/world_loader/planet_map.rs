//! Planet map: the whole world on the loading-screen globe, in play.
//!
//! Same surface texels, shader and backdrop as the loading screen, plus a
//! graticule, the player with a heading, waypoints and a coordinate readout.
//! Drag turns the globe, the wheel zooms, a double click recentres.
use std::sync::Arc;

use egui::{Align2, Color32, FontId, Id, Order, Painter, Pos2, Rect, Sense, Stroke, pos2, vec2};
use engine_voxel::WorldSampleConfig;
use glam::{IVec3, Vec3};

use super::globe_gpu::Pose;
use super::surface::Surface;
use super::{globe, globe_gpu, solve, sphere, surface};

pub struct Inputs {
    pub cfg: WorldSampleConfig,
    /// Player position in voxels and look direction in world axes.
    pub player_voxel: Vec3,
    pub forward: Vec3,
    /// Height above the surface in metres, when known.
    pub altitude_m: Option<f64>,
    /// Name of the body the player is on, when the system knows it.
    pub body: Option<String>,
    pub waypoints: Vec<(IVec3, String, Color32)>,
}

/// Whether this world has a whole-planet map at all.
pub fn available(cfg: &WorldSampleConfig) -> bool {
    cfg.is_planet() || engine_worldgen::geo::active(cfg)
}

enum Source {
    Ready(Arc<Surface>),
    /// Planet survey still sampling faces.
    Surveying(usize),
    /// Drainage solve not finished.
    Solving,
    None,
}

fn source(cfg: &WorldSampleConfig, center: (i32, i32)) -> Source {
    if cfg.is_planet() {
        // Cached per world: the loading screen's survey is reused as is.
        return match sphere::request(*cfg, center) {
            Some(s) => s.surface().map_or(Source::Surveying(s.faces()), Source::Ready),
            None => Source::None,
        };
    }
    if !engine_worldgen::geo::active(cfg) {
        return Source::None;
    }
    if !engine_worldgen::geo::entry_for(cfg).ready() {
        return Source::Solving;
    }
    let planet = engine_worldgen::geo::planet_for(cfg);
    let key = Arc::as_ptr(&planet) as u64 ^ 0x9a1e_7a00_5017_ed00;
    surface::latest(key, move || solve::from_planet(&planet, false, key))
        .filter(|s| s.key == key)
        .map_or(Source::Solving, Source::Ready)
}

/// Unit direction on the globe of a voxel position.
fn direction(cfg: &WorldSampleConfig, x: f32, z: f32) -> Vec3 {
    if cfg.is_planet() {
        return sphere::spawn_dir(cfg, (x.floor() as i32, z.floor() as i32));
    }
    // Single solved face on +X, as the loading screen draws it.
    let half = engine_worldgen::geo::face_edge_m(cfg) * 0.5;
    let m = f64::from(cfg.voxel_meters());
    let s = (f64::from(x) * m / half).clamp(-1.0, 1.0);
    let t = (f64::from(z) * m / half).clamp(-1.0, 1.0);
    engine_core::cube_sphere::Projection::Tangent.to_sphere(engine_core::cube_sphere::Face::PosX, s, t).as_vec3()
}

/// Latitude and longitude in degrees; the globe's +Y is north.
fn lat_lon(d: Vec3) -> (f32, f32) {
    (d.y.clamp(-1.0, 1.0).asin().to_degrees(), d.x.atan2(d.z).to_degrees())
}

fn lat_lon_text(d: Vec3) -> String {
    let (lat, lon) = lat_lon(d);
    format!("{:.2}° {}  {:.2}° {}", lat.abs(), if lat >= 0.0 { "N" } else { "S" }, lon.abs(), if lon >= 0.0 { "E" } else { "W" })
}

fn thousands(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if v < 0 { format!("-{out}") } else { out }
}

fn project(pose: &Pose, centre: Pos2, radius: f32, d: Vec3) -> (Pos2, f32) {
    let v = pose.rotation * d;
    (centre + vec2(v.x, -v.y) * radius, v.z)
}

/// Latitude and longitude lines, fading toward the limb.
fn graticule(p: &Painter, pose: &Pose, centre: Pos2, radius: f32, colour: Color32) {
    const STEPS: usize = 72;
    let line = |points: &mut dyn Iterator<Item = Vec3>, weight: f32| {
        let mut last: Option<(Pos2, f32)> = None;
        for d in points {
            let here = project(pose, centre, radius, d);
            if let Some((a, za)) = last {
                let z = za.min(here.1);
                if z > 0.0 {
                    let alpha = (z / 0.35).clamp(0.0, 1.0) * weight;
                    p.line_segment([a, here.0], Stroke::new(1.0, colour.gamma_multiply(alpha)));
                }
            }
            last = Some(here);
        }
    };
    for lat in [-60.0f32, -30.0, 0.0, 30.0, 60.0] {
        let (s, c) = lat.to_radians().sin_cos();
        let weight = if lat == 0.0 { 0.55 } else { 0.28 };
        line(&mut (0..=STEPS).map(|k| {
            let a = std::f32::consts::TAU * k as f32 / STEPS as f32;
            Vec3::new(c * a.sin(), s, c * a.cos())
        }), weight);
    }
    for lon in (0..12).map(|k| k as f32 * 30.0) {
        let (s, c) = lon.to_radians().sin_cos();
        let weight = if lon == 0.0 { 0.5 } else { 0.25 };
        line(&mut (0..=STEPS / 2).map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / (STEPS / 2) as f32;
            Vec3::new(a.cos() * s, a.sin(), a.cos() * c)
        }), weight);
    }
}

/// Translucent plate with a lit top rule, as on the loading panel.
fn plate(p: &Painter, r: Rect, theme: super::super::themes::Palette) {
    p.rect_filled(r, 5.0, theme.panel.gamma_multiply(0.82));
    p.rect_stroke(r, 5.0, Stroke::new(1.0, theme.edge), egui::StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(r.min + vec2(5.0, 0.0), vec2(r.width() - 10.0, 1.5)), 0.0, theme.accent.gamma_multiply(0.7));
}

pub fn draw(ctx: &egui::Context, open: &mut bool, input: &Inputs) {
    if !*open {
        return;
    }
    let r = ctx.content_rect();
    if !r.is_positive() {
        return;
    }
    let theme = super::super::themes::current(ctx);
    let now = ctx.input(|i| i.time);
    let seconds = now as f32;
    let still = super::super::magic_fx::reduced(ctx);
    let cfg = input.cfg;
    let center = (input.player_voxel.x.floor() as i32, input.player_voxel.z.floor() as i32);
    let world = u64::from(cfg.seed) << 8 ^ u64::from(cfg.is_planet());

    egui::Area::new(Id::new("planet_map"))
        .order(Order::Foreground)
        .fixed_pos(r.min)
        .show(ctx, |ui| {
            ui.set_min_size(r.size());
            // Swallow clicks so nothing behind the map reacts.
            let response = ui.allocate_rect(r, Sense::click_and_drag());
            let p = ui.painter().clone();
            let centre = r.center() + vec2(0.0, 8.0);

            let zoom_id = Id::new(("planet_map_zoom", world));
            let mut zoom = ctx.data(|d| d.get_temp::<f32>(zoom_id)).unwrap_or(1.0);
            if response.hovered() {
                let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                zoom = (zoom * (scroll * 0.0025).exp()).clamp(0.7, 3.2);
            }
            ctx.data_mut(|d| d.insert_temp(zoom_id, zoom));
            let radius = r.width().min(r.height()) * 0.38 * zoom;

            let here = direction(&cfg, input.player_voxel.x, input.player_voxel.z);
            let pose = globe::pose(Some(here), true, seconds, true, 0.0, None);
            let (pose, moving) = globe::orbit(ctx, "planet_map", world, centre, radius, now, still, pose);
            globe_gpu::paint_backdrop(&p, r, centre, radius, pose.sun, seconds, still);

            let src = source(&cfg, center);
            let radius_km = match &src {
                Source::Ready(s) => {
                    globe_gpu::paint(&p, centre, radius, Arc::clone(s), pose, seconds, 0.0, [1.0, 0.0]);
                    Some(s.radius_km)
                }
                _ => None,
            };
            if radius_km.is_some() {
                graticule(&p, &pose, centre, radius, theme.text());
            } else {
                // Footprint of the globe while its map is still being made.
                p.circle_stroke(centre, radius, Stroke::new(1.0, theme.edge));
                let spin = if still { 0.0 } else { seconds * 2.0 };
                let arc: Vec<Pos2> = (0..=32).map(|k| {
                    let a = spin + 1.6 * k as f32 / 32.0;
                    centre + vec2(a.cos(), a.sin()) * radius
                }).collect();
                p.add(egui::Shape::line(arc, Stroke::new(2.0, theme.accent)));
                let label = match src {
                    Source::Surveying(faces) => format!("Planet Survey  ·  {faces}/6 Faces"),
                    Source::Solving => "Drainage Solve".to_owned(),
                    _ => "No Planet Map".to_owned(),
                };
                p.text(centre, Align2::CENTER_CENTER, label, FontId::proportional(16.0), theme.muted());
            }

            // Waypoints on the near side.
            for (pos, name, colour) in &input.waypoints {
                let d = direction(&cfg, pos.x as f32 + 0.5, pos.z as f32 + 0.5);
                let (at, z) = project(&pose, centre, radius, d);
                if z <= 0.05 {
                    continue;
                }
                let fade = ((z - 0.05) / 0.2).clamp(0.0, 1.0);
                let c = colour.gamma_multiply(fade);
                let diamond = vec![at + vec2(0.0, -5.0), at + vec2(4.0, 0.0), at + vec2(0.0, 5.0), at + vec2(-4.0, 0.0)];
                p.add(egui::Shape::convex_polygon(diamond, c, Stroke::new(1.0, Color32::from_black_alpha((160.0 * fade) as u8))));
                p.text(at + vec2(7.0, -1.0), Align2::LEFT_CENTER, name, FontId::proportional(12.5), c);
            }

            // Player: pulsing ring and a heading chevron along the ground.
            let (at, z) = project(&pose, centre, radius, here);
            if z > 0.0 {
                let fade = (z / 0.15).clamp(0.0, 1.0);
                let pulse = if still { 0.0 } else { (seconds * 1.4).fract() };
                p.circle_stroke(at, 5.0 + 10.0 * pulse, Stroke::new(1.5, theme.accent.gamma_multiply((1.0 - pulse) * fade)));
                let step = 64.0;
                let east = direction(&cfg, input.player_voxel.x + step, input.player_voxel.z) - here;
                let south = direction(&cfg, input.player_voxel.x, input.player_voxel.z + step) - here;
                let ground = east * input.forward.x + south * input.forward.z;
                let v = pose.rotation * ground.normalize_or_zero();
                let h = vec2(v.x, -v.y).normalized();
                if h.length_sq() > 0.5 {
                    let side = vec2(-h.y, h.x);
                    let tip = at + h * 11.0;
                    p.add(egui::Shape::convex_polygon(vec![tip, at + side * 5.0 - h * 3.0, at - side * 5.0 - h * 3.0],
                        theme.accent.gamma_multiply(fade), Stroke::new(1.0, Color32::from_black_alpha(170))));
                }
                p.circle_filled(at, 3.0, theme.text().gamma_multiply(fade));
            }

            // Coordinates under the cursor.
            if let Some(q) = response.hover_pos() {
                let v = (q - centre) / radius;
                let b2 = v.length_sq();
                if b2 < 1.0 {
                    let view = Vec3::new(v.x, -v.y, (1.0 - b2).sqrt());
                    let d = pose.rotation.transpose() * view;
                    let text = lat_lon_text(d);
                    let galley = p.layout_no_wrap(text, FontId::proportional(13.0), theme.text());
                    let box_r = Rect::from_min_size(q + vec2(16.0, 14.0), galley.size() + vec2(14.0, 8.0));
                    plate(&p, box_r, theme);
                    p.galley(box_r.min + vec2(7.0, 4.0), galley, theme.text());
                }
            }

            // Title plate.
            let title = input.body.clone().unwrap_or_else(|| "Planet".to_owned());
            let title_r = Rect::from_min_size(r.min + vec2(20.0, 20.0), vec2(250.0, 58.0));
            plate(&p, title_r, theme);
            p.text(title_r.min + vec2(14.0, 9.0), Align2::LEFT_TOP, title, FontId::proportional(20.0), theme.text());
            if let Some(km) = radius_km {
                p.text(title_r.min + vec2(14.0, 35.0), Align2::LEFT_TOP, format!("Radius {:.1} km", km),
                    FontId::monospace(12.5), theme.muted());
            }

            // Readout: position on the globe, height and block coordinates.
            let read_r = Rect::from_min_size(pos2(r.left() + 20.0, r.bottom() - 98.0), vec2(340.0, 78.0));
            plate(&p, read_r, theme);
            // Proportional values: the pixel monospace face has no degree sign.
            let line = |k: usize, label: &str, value: String| {
                let y = read_r.top() + 10.0 + k as f32 * 21.0;
                p.text(pos2(read_r.left() + 14.0, y), Align2::LEFT_TOP, label, FontId::proportional(13.0), theme.muted());
                p.text(pos2(read_r.left() + 108.0, y), Align2::LEFT_TOP, value, FontId::proportional(13.0), theme.text());
            };
            line(0, "Position", lat_lon_text(here));
            line(1, "Altitude", input.altitude_m.map_or_else(|| "–".to_owned(), |a| format!("{} m", thousands(a.round() as i64))));
            line(2, "Block", format!("X {}  ·  Z {}", thousands(i64::from(center.0)), thousands(i64::from(center.1))));

            let keys = "[Drag] Rotate  ·  [Scroll] Zoom  ·  [Double Click] Recentre";
            p.text(pos2(r.center().x, r.bottom() - 20.0), Align2::CENTER_BOTTOM, keys, FontId::proportional(13.0), theme.muted());

            if moving {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(if still { 500 } else { 50 }));
            }
        });
}
