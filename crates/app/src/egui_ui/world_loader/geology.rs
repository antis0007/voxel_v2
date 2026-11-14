mod history_events;
pub(super) mod motion;

use std::sync::Arc;

use egui::epaint::Shadow;
use egui::{Align2, Color32, FontId, Id, Painter, Pos2, Rect, Stroke, pos2, vec2};
use engine_core::cube_sphere::{Face, Projection, atlas};
use engine_worldgen::planetary::{DriftFrame, DriftTexel};

use super::super::themes::Palette;
use super::{
    WorldLoadReport,
    sphere::AtlasPalette,
    surface::{self, Surface, Texel},
};

const HISTORY_SECONDS: f64 = 12.0;
/// While history is still being produced, playback holds about this far
/// behind the newest frame (one and a half 3 Myr frames).
const LAG_MYR: f64 = 4.5;
/// Playback speed changes settle over about this long.
const RATE_SECONDS: f64 = 0.6;
pub(super) const HANDOFF_SECONDS: f32 = 2.0;
/// Sea/land colour band, metres either side of sea level: a coast that moves
/// between frames slides instead of flipping whole texels.
const COAST_M: f32 = 20.0;
const MARGINS: [([u8; 3], &str); 3] = [
    ([244, 130, 80], "Collision"),
    ([96, 206, 235], "Rift"),
    ([227, 196, 116], "Transform"),
];

#[derive(Clone)]
struct Playback {
    myr: f64,
    at: f64,
    /// Smoothed playback speed, Myr per second.
    rate: f64,
    /// Newest frame seen and when, for the producer's pace.
    seen_myr: f64,
    seen_at: f64,
    /// Smoothed producer pace, Myr per second; zero until measured.
    producer: f64,
}

impl Playback {
    fn new(myr: f64, now: f64) -> Self {
        Self { myr, at: now, rate: 0.0, seen_myr: myr, seen_at: now, producer: 0.0 }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Frame<'a> {
    pub from: &'a DriftFrame,
    pub to: &'a DriftFrame,
    pub blend: f32,
    pub event_frames: &'a [Arc<DriftFrame>],
    /// The whole history so far, for the timeline.
    pub frames: &'a [Arc<DriftFrame>],
}

impl Frame<'_> {
    fn myr(self) -> f32 {
        f32::from(self.from.myr) + f32::from(self.to.myr - self.from.myr) * self.blend
    }
}

pub(super) fn reset(ctx: &egui::Context, world: u64) {
    ctx.data_mut(|data| data.remove::<Playback>(Id::new(("loading_drift_clock", world))));
}

pub(super) fn frame<'a>(
    ctx: &egui::Context,
    report: &'a WorldLoadReport,
    now: f64,
    still: bool,
    handoff_ready: bool,
) -> Option<Frame<'a>> {
    let first = report.drift.first()?;
    let last = report.drift.last()?;
    let id = Id::new(("loading_drift_clock", report.world_key));
    let myr = ctx.data_mut(|data| {
        let clock = data.get_temp_mut_or_insert_with(id, || Playback::new(f64::from(first.myr), now));
        let latest = f64::from(last.myr);
        let dt = (now - clock.at).clamp(0.0, 0.1);
        let base_rate = f64::from(first.duration_myr) / HISTORY_SECONDS;
        // Frames arrive in lumps; pace playback on the producer's smoothed
        // rate, not on the distance to the newest frame, or it surges and
        // stops with every arrival.
        if latest > clock.seen_myr {
            let pace = (latest - clock.seen_myr) / (now - clock.seen_at).max(0.05);
            clock.producer = if clock.producer > 0.0 { clock.producer + (pace - clock.producer) * 0.25 } else { pace };
            clock.seen_myr = latest;
            clock.seen_at = now;
        }
        // Follow a live producer without dropping whole epochs or spending
        // time that elapsed while the window was suspended.
        let gap = latest - clock.myr;
        let complete = last.myr >= first.duration_myr;
        let target = if complete || clock.producer <= 0.0 {
            base_rate.max(gap / 2.0)
        } else {
            // Steer gently toward the lag, and ease to rest on the newest frame
            // if the producer pauses.
            clock.producer * (0.5 + 0.5 * gap / LAG_MYR) * (gap / 1.5).clamp(0.0, 1.0)
        }.min(base_rate * 1.5);
        clock.rate += (target - clock.rate) * (dt / RATE_SECONDS).min(1.0);
        clock.myr = if still { latest } else { (clock.myr + dt * clock.rate.max(0.0)).min(latest) };
        clock.at = now;
        clock.myr
    });
    if myr >= f64::from(first.duration_myr) && handoff_ready {
        return None;
    }
    let upper = report.drift.partition_point(|f| f64::from(f.myr) <= myr).min(report.drift.len() - 1);
    let to = &report.drift[upper];
    let from = &report.drift[upper.saturating_sub(1)];
    let span = f64::from(to.myr - from.myr);
    let blend = if span > 0.0 { ((myr - f64::from(from.myr)) / span).clamp(0.0, 1.0) as f32 } else { 1.0 };
    let event_frames = &report.drift[history_events::range(&report.drift, myr, |f| f64::from(f.myr))];
    Some(Frame { from, to, blend, event_frames, frames: &report.drift })
}

/// Plate motion carrying the globe between the frame's endpoints, or `None`
/// when the history has no motion stages (the endpoints then crossfade).
/// `VOXEL_GEOLOGY_DISSOLVE=1` keeps the crossfade, for A/B captures.
pub(super) fn motion(world: u64, frame: Frame<'_>) -> Option<Arc<motion::Motion>> {
    static DISSOLVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *DISSOLVE.get_or_init(|| std::env::var_os("VOXEL_GEOLOGY_DISSOLVE").is_some_and(|v| v == "1")) {
        return None;
    }
    motion::of(world, frame.from, frame.to)
}

/// Captures only: hold the history clock at `myr`.
pub(super) fn pin(ctx: &egui::Context, world: u64, myr: f64, now: f64) {
    ctx.data_mut(|data| data.insert_temp(Id::new(("loading_drift_clock", world)), Playback::new(myr, now)));
}

/// Margin tints, linear, in `MARGINS` order.
fn margin_colours() -> [[f32; 3]; 3] {
    MARGINS.map(|(rgb, _)| surface::linear(rgb.map(|v| f32::from(v) / 255.0)))
}

pub(super) fn surface(world: u64, frame: &DriftFrame) -> Option<Arc<Surface>> {
    let key = world.rotate_left(17) ^ 0x6c4a_7d17_940e_f000 ^ u64::from(frame.myr);
    surface::history_cached(key, || {
        let palette = AtlasPalette::new();
        let colours = margin_colours();
        let n = frame.n;
        let mut texels: Vec<Texel> = frame
            .texels
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let latitude = Projection::Tangent.to_sphere(Face::ALL[i / (n * n)],
                    surface::grid(i % n, n), surface::grid(i / n % n, n)).y.abs() as f32;
                shade(&palette, &colours, frame.budget.is_some(), t, latitude, COAST_M)
            })
            .collect();
        let plates: Vec<u16> = frame.texels.iter().map(|t| t.plate).collect();
        let stress: Vec<[f32; 3]> = frame.texels.iter().map(|t| t.stress).collect();
        seams(n, &mut texels, &plates, &stress, &colours);
        Some(Surface {
            key,
            n: frame.n,
            texels,
            radius_km: atlas::RADIUS_M as f32 * 0.001,
        })
    })
}

/// One history texel's display colour. `coast_m` > 0 blends sea and land over
/// that height band, so a shoreline that moves between samples slides instead
/// of flipping whole texels.
fn shade(palette: &AtlasPalette, colours: &[[f32; 3]; 3], budget: bool, t: &DriftTexel,
    latitude: f32, coast_m: f32) -> Texel
{
    let height_km = t.height_m * 0.001;
    let sea_ice = if height_km < 0.0 { super::sphere::sea_ice_cover(t.winter_c.unwrap_or(0.0)) } else { 0.0 };
    let land = || if let Some([moisture, annual]) = t.climate {
        palette.biome(t.height_m.max(0.0), moisture, annual, 0.0)
    } else {
        let runoff = (t.runoff_m2 / 1_500_000.0).clamp(0.0, 1.0);
        let moisture = (0.53 - 0.24 * latitude + 0.30 * runoff).clamp(0.0, 1.0);
        let annual = 26.0 - 43.0 * latitude - 0.018 * t.height_m.max(0.0);
        surface::land_rgb(height_km.max(0.0) * 4.0, Some(moisture), Some(annual), 0.0)
    };
    let ocean = || palette.ocean((-height_km).max(0.0), sea_ice);
    let mut base = if coast_m > 0.0 && t.height_m.abs() < coast_m {
        let w = (t.height_m / coast_m * 0.5 + 0.5).clamp(0.0, 1.0);
        let w = w * w * (3.0 - 2.0 * w);
        let (sea, ground) = (ocean(), land());
        std::array::from_fn(|c| sea[c] + (ground[c] - sea[c]) * w)
    } else if height_km < 0.0 {
        ocean()
    } else {
        land()
    };
    if budget {
        // Deposits and runoff come from this frame's evolving bed.
        let sediment = (t.sediment_m / 8.0).clamp(0.0, 1.0)
            * (1.0 - (t.height_m.abs() / 35.0).clamp(0.0, 1.0));
        let sand = surface::linear([0.58, 0.50, 0.32]);
        let river = if t.height_m > 0.0 {
            ((t.runoff_m2 / 300_000.0).ln_1p() * 0.16).min(0.45)
        } else { 0.0 };
        let water = surface::linear([0.20, 0.39, 0.43]);
        for c in 0..3 {
            base[c] += (sand[c] - base[c]) * sediment * 0.55;
            base[c] += (water[c] - base[c]) * river;
        }
    }
    if t.climate.is_some() {
        let eruption = (t.eruption_m / 18.0).clamp(0.0, 0.80);
        let rupture = (t.rupture_m.abs() / 35.0).clamp(0.0, 0.28);
        let lava = surface::linear([0.48, 0.19, 0.07]);
        let scarp = surface::linear([0.62, 0.53, 0.41]);
        for c in 0..3 {
            base[c] += (scarp[c] - base[c]) * rupture;
            base[c] += (lava[c] - base[c]) * eruption;
        }
    }
    // A faint belt either side of a margin; the seam line itself is drawn by
    // `seams`, so the wash no longer has to carry the boundary and the terrain
    // under it stays readable.
    let weight = t.stress.map(|v| v.max(0.0).powf(1.5));
    let sum = weight.iter().sum::<f32>();
    let opacity = if t.climate.is_some() { (sum * 0.16).min(0.14) }
        else if budget { (sum * 0.30).min(0.26) }
        else { (sum * 0.45).min(0.38) };
    let rgb = std::array::from_fn(|c| {
        let tint = (0..3).map(|i| colours[i][c] * weight[i]).sum::<f32>();
        base[c] * (1.0 - opacity) + tint * opacity / sum.max(0.001)
    });
    // Fresh lava fields and active arcs glow faintly through the night side.
    let lava = (t.eruption_m / 18.0).clamp(0.0, 1.0);
    let glow = [0.95, 0.38, 0.10].map(|c| c * lava * 0.7);
    Texel { rgb, height_km, sea_ice, glow }
}

/// Plate seams: a one-texel line where the owning plate changes, tinted by
/// the margin's dominant motion and brighter where it is under more stress.
/// Painted into the albedo for the day side and stored as emission so the
/// boundaries stay lit across the night side.
pub(super) fn seams(n: usize, texels: &mut [Texel], plates: &[u16], stress: &[[f32; 3]], colours: &[[f32; 3]; 3]) {
    if n < 2 || texels.len() != 6 * n * n || plates.len() != texels.len() || stress.len() != texels.len() {
        return;
    }
    for i in 0..texels.len() {
        let (x, z) = (i % n, i / n % n);
        let own = plates[i];
        // Mark only the side with the larger id, so the line is one texel wide.
        let differs = |j: usize| plates[j] < own;
        let edge = (x > 0 && differs(i - 1)) || (x + 1 < n && differs(i + 1))
            || (z > 0 && differs(i - n)) || (z + 1 < n && differs(i + n));
        if !edge {
            continue;
        }
        let w = stress[i].map(|v| v.max(0.0));
        let sum = w.iter().sum::<f32>();
        // A quiet boundary is a pale hairline; an active one takes its colour.
        let quiet = [0.62, 0.66, 0.70];
        let tint: [f32; 3] = std::array::from_fn(|c| if sum > 1e-3 {
            (0..3).map(|k| colours[k][c] * w[k]).sum::<f32>() / sum
        } else { quiet[c] });
        let strength = 0.35 + 0.65 * sum.min(1.0);
        let t = &mut texels[i];
        for c in 0..3 {
            t.rgb[c] += (tint[c] - t.rgb[c]) * 0.45 * strength;
            t.glow[c] = t.glow[c].max(tint[c] * strength);
        }
    }
}

const COOL: Color32 = Color32::from_rgb(96, 206, 235);
const WARM: Color32 = Color32::from_rgb(244, 130, 80);
const ERUPTION: Color32 = Color32::from_rgb(235, 132, 58);
const QUAKE: Color32 = Color32::from_rgb(214, 192, 148);

fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let c = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(c(a.r(), b.r()), c(a.g(), b.g()), c(a.b(), b.b()))
}

/// Cool to warm through a pale neutral, so a mild climate does not read hot.
fn climate_colour(t: f32) -> Color32 {
    let neutral = Color32::from_rgb(206, 198, 178);
    if t < 0.5 { lerp_colour(COOL, neutral, t * 2.0) } else { lerp_colour(neutral, WARM, t * 2.0 - 1.0) }
}

/// Deep-time readout under the globe: the model age, the climate and sea
/// level so far as a timeline, dated events as ticks, and the margin legend.
/// Without room below the globe (portrait layouts) it shrinks to an age badge.
pub(super) fn caption(p: &Painter, area: Rect, centre: Pos2, radius: f32, frame: Frame<'_>,
    theme: Palette, seconds: f32, still: bool)
{
    const H: f32 = 98.0;
    let duration = f32::from(frame.from.duration_myr).max(1.0);
    let now = frame.myr();
    let age = (duration - now).max(0.0);
    let age_text = if age < 0.5 { "Present".to_owned() } else { format!("{age:.1} Ma") };
    let text = theme.text();
    let muted = theme.muted();
    let card = |r: Rect| {
        p.add(Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(r, 6));
        p.rect_filled(r, 6.0, theme.panel.gamma_multiply(0.86));
        p.rect_stroke(r, 6.0, Stroke::new(1.0, theme.edge), egui::StrokeKind::Inside);
    };

    let below = area.bottom() - (centre.y + radius);
    if below < H + 30.0 {
        // Badge straddling the top limb: age and a progress hairline.
        let r = Rect::from_center_size(pos2(centre.x, centre.y - radius - 2.0), vec2(172.0, 34.0));
        card(r);
        p.text(pos2(r.center().x, r.top() + 7.0), Align2::CENTER_TOP, age_text, FontId::monospace(15.0), text);
        let track = Rect::from_min_size(pos2(r.left() + 14.0, r.bottom() - 7.0), vec2(r.width() - 28.0, 2.0));
        p.rect_filled(track, 1.0, theme.edge);
        p.rect_filled(Rect::from_min_size(track.min, vec2(track.width() * now / duration, 2.0)), 1.0, theme.accent);
        return;
    }

    let w = (radius * 2.1).clamp(320.0, 560.0).min(area.width() - 32.0);
    let top = (centre.y + radius + 24.0).min(area.bottom() - H - 10.0);
    let r = Rect::from_min_size(pos2(centre.x - w * 0.5, top), vec2(w, H));
    card(r);
    let p = p.with_clip_rect(r);
    let left = r.left() + 14.0;
    let right = r.right() - 14.0;

    // Row 1: age on the left; climate range and sea level on the right.
    p.text(pos2(left, r.top() + 9.0), Align2::LEFT_TOP, age_text, FontId::monospace(20.0), text);
    let blend = |a: f32, b: f32| a + (b - a) * frame.blend;
    let sea = blend(frame.from.sea_m, frame.to.sea_m) - engine_voxel::WATER_LEVEL_METERS;
    let mut x = right;
    let readout = |x: &mut f32, value: String, colour: Color32| {
        let g = p.layout_no_wrap(value, FontId::monospace(12.5), text);
        let gw = g.size().x;
        p.galley(pos2(*x - gw, r.top() + 15.0), g, text);
        p.circle_filled(pos2(*x - gw - 8.0, r.top() + 22.0), 3.0, colour);
        *x -= gw + 24.0;
    };
    readout(&mut x, format!("Sea {sea:+.1} m"), COOL);
    if let (Some(a), Some(b)) = (frame.from.annual_range_c, frame.to.annual_range_c) {
        readout(&mut x, format!("{:.0} / {:.0}°C", blend(a[0], b[0]), blend(a[1], b[1])), WARM);
    }

    // Row 2: the timeline. Past is drawn, the produced-but-unplayed lead is a
    // pale fill, the future is the bare track.
    let chart = Rect::from_min_max(pos2(left, r.top() + 42.0), pos2(right, r.top() + 70.0));
    let x_at = |myr: f32| chart.left() + chart.width() * (myr / duration).clamp(0.0, 1.0);
    p.rect_filled(chart, 3.0, theme.recess.gamma_multiply(0.7));
    let latest = frame.frames.last().map_or(now, |f| f32::from(f.myr));
    let head = x_at(now);
    if latest > now {
        p.rect_filled(Rect::from_min_max(pos2(head, chart.top()), pos2(x_at(latest), chart.bottom())),
            0.0, text.gamma_multiply(0.05));
    }
    // Climate ribbon along the bottom: mean annual temperature per interval.
    let ribbon = Rect::from_min_max(pos2(chart.left(), chart.bottom() - 5.0), chart.max);
    let mids: Vec<Option<f32>> = frame.frames.iter().map(|f| f.annual_range_c.map(|[c, h]| 0.5 * (c + h))).collect();
    let (lo, hi) = mids.iter().flatten().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &m| (a.min(m), b.max(m)));
    let span = (hi - lo).max(4.0);
    for (k, pair) in frame.frames.windows(2).enumerate() {
        let (a, b) = (f32::from(pair[0].myr), f32::from(pair[1].myr).min(now));
        if a >= now {
            break;
        }
        let colour = mids[k + 1].map_or(theme.accent.gamma_multiply(0.5),
            |m| climate_colour((m - 0.5 * (lo + hi)) / span + 0.5));
        let seg = Rect::from_min_max(pos2(x_at(a), ribbon.top()), pos2(x_at(b).max(x_at(a) + 1.0), ribbon.bottom()));
        p.rect_filled(seg, 0.0, colour);
    }
    // Sea level above the ribbon, about its own datum.
    let band = Rect::from_min_max(pos2(chart.left(), chart.top() + 7.0), pos2(chart.right(), ribbon.top() - 3.0));
    let seas: Vec<(f32, f32)> = frame.frames.iter()
        .map(|f| (f32::from(f.myr), f.sea_m - engine_voxel::WATER_LEVEL_METERS)).collect();
    let reach = seas.iter().fold(8.0_f32, |m, &(_, s)| m.max(s.abs()));
    let y_at = |s: f32| band.center().y - (s / reach).clamp(-1.0, 1.0) * band.height() * 0.5;
    p.hline(chart.left()..=chart.right(), band.center().y, Stroke::new(1.0, theme.edge.gamma_multiply(0.6)));
    let mut line: Vec<Pos2> = seas.iter().take_while(|(m, _)| *m <= now).map(|&(m, s)| pos2(x_at(m), y_at(s))).collect();
    if !line.is_empty() {
        line.push(pos2(head, y_at(sea)));
        p.add(egui::Shape::line(line, Stroke::new(1.5, COOL.gamma_multiply(0.9))));
    }
    // Dated events as ticks along the top edge; recent ones stand taller.
    for f in frame.frames.iter().take_while(|f| f32::from(f.myr) <= now + 3.0) {
        for e in &f.events {
            let at = e.myr as f32;
            if at > now {
                continue;
            }
            let recent = 1.0 - ((now - at) / history_events::WINDOW_MYR as f32).clamp(0.0, 1.0);
            let colour = if e.eruption { ERUPTION } else { QUAKE };
            let x = x_at(at);
            p.line_segment([pos2(x, chart.top() + 1.0), pos2(x, chart.top() + 4.0 + 5.0 * recent)],
                Stroke::new(1.0, colour.gamma_multiply(0.45 + 0.55 * recent)));
        }
    }
    // Playhead.
    p.line_segment([pos2(head, chart.top() - 2.0), pos2(head, chart.bottom() + 2.0)], Stroke::new(1.5, theme.accent));
    let pulse = if still { 1.0 } else { 0.85 + 0.15 * (seconds * 3.0).sin() };
    p.circle_filled(pos2(head, chart.top() - 2.0), 5.5 * pulse, theme.accent.gamma_multiply(0.25));
    p.circle_filled(pos2(head, chart.top() - 2.0), 2.5, theme.accent);

    // Row 3: endpoints and the legend for what the globe shows.
    let y = chart.bottom() + 8.0;
    let ends = FontId::monospace(11.0);
    p.text(pos2(left, y), Align2::LEFT_TOP, format!("{duration:.0} Ma"), ends.clone(), muted);
    p.text(pos2(right, y), Align2::RIGHT_TOP, "Present", ends, muted);
    let font = FontId::proportional(12.0);
    let mut items: Vec<(Color32, &str, u8)> =
        MARGINS.iter().map(|(c, l)| (Color32::from_rgb(c[0], c[1], c[2]), *l, 0)).collect();
    items.push((ERUPTION, "Eruption", 1));
    items.push((QUAKE, "Quake", 2));
    let galleys: Vec<_> = items.iter()
        .map(|(c, l, k)| (p.layout_no_wrap((*l).to_owned(), font.clone(), muted), *c, *k)).collect();
    let item_w = |g: &Arc<egui::Galley>| g.size().x + 26.0;
    let room = r.width() - 2.0 * 64.0;
    let mut shown = galleys.len();
    while shown > 3 && galleys[..shown].iter().map(|(g, ..)| item_w(g)).sum::<f32>() > room {
        shown -= 1;
    }
    let total: f32 = galleys[..shown].iter().map(|(g, ..)| item_w(g)).sum::<f32>() - 12.0;
    let mut x = r.center().x - total * 0.5;
    for (g, colour, kind) in galleys.into_iter().take(shown) {
        let mid = pos2(x + 4.0, y + 7.0);
        match kind {
            0 => {
                p.line_segment([mid - vec2(4.0, 0.0), mid + vec2(4.0, 0.0)], Stroke::new(2.0, colour));
            }
            1 => {
                p.circle_filled(mid, 3.0, colour);
            }
            _ => {
                p.circle_stroke(mid, 3.2, Stroke::new(1.1, colour));
            }
        }
        let gw = g.size().x;
        p.galley(pos2(x + 13.0, y), g, muted);
        x += gw + 26.0;
    }
}

/// Sparse dated events share the globe's camera and clock. Their onset is the
/// simulated threshold crossing; each plays out over `WINDOW_MYR` of model time.
/// Eruptions glow and throw a shock ring; quakes ripple twice.
pub(super) fn events(p: &Painter, centre: Pos2, radius: f32, frame: Frame<'_>, pose: &super::globe_gpu::Pose) {
    let myr = f64::from(frame.myr());
    let scale = (radius / 300.0).clamp(0.6, 1.6);
    for event in frame.event_frames.iter().flat_map(|f| &f.events) {
        let age = myr - event.myr;
        if !(0.0..history_events::WINDOW_MYR).contains(&age) {
            continue;
        }
        let at = pose.rotation * glam::Vec3::from_array(event.direction);
        if at.z <= 0.08 {
            continue;
        }
        let life = (age / history_events::WINDOW_MYR) as f32;
        let fade = ((at.z - 0.08) * 4.0).min(1.0) * (1.0 - life).powi(2);
        // Foreshortened toward the limb, so a pulse lies on the surface.
        let squash = at.z.clamp(0.25, 1.0);
        let point = centre + vec2(at.x, -at.y) * radius;
        let size = scale * (event.metres.abs() / 18.0).sqrt().clamp(0.75, 1.5);
        let ring = |r: f32, alpha: f32, colour: Color32| {
            if r > 0.5 {
                p.add(egui::Shape::ellipse_stroke(point, vec2(r, r * squash),
                    Stroke::new(1.1, colour.gamma_multiply(alpha))));
            }
        };
        if event.eruption {
            let glow = 1.0 - life * 0.6;
            for (k, a) in [(7.0, 0.10), (4.0, 0.26), (2.0, 0.85)] {
                p.circle_filled(point, k * size * glow, ERUPTION.gamma_multiply(a * fade));
            }
            p.circle_filled(point, size, Color32::from_rgb(255, 226, 170).gamma_multiply(fade));
            ring((2.5 + life * 16.0) * size, fade, ERUPTION);
        } else {
            ring((3.0 + life * 20.0) * size, fade, QUAKE);
            ring((3.0 + (life - 0.22).max(0.0) * 20.0) * size, fade * 0.6, QUAKE);
            p.circle_filled(point, 1.3 * size, QUAKE.gamma_multiply(fade));
        }
    }
}
