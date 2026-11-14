//! One map key, three views: Region (explored tiles), Planet (the globe) and
//! System (the orrery). The key opens the view that fits the player's height;
//! while a map is open it follows the player across bands until a tab is
//! picked by hand.
use std::sync::Mutex;

use egui::{Align2, Color32, FontId, Id, Order, Rect, Sense, Stroke, pos2, vec2};

use super::world_loader::planet_map;
use crate::creative::CreativeState;
use crate::space_nav;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Region,
    Planet,
    System,
}

const TABS: [(Tab, &str); 3] = [(Tab::Region, "Region"), (Tab::Planet, "Planet"), (Tab::System, "System")];

/// Above this height the planet view replaces the region view.
const PLANET_M: f64 = 2_500.0;
/// Beyond this many planet radii the system view takes over.
const SYSTEM_RADII: f64 = 3.0;
/// Bands overlap by this share so a player on a boundary does not flicker.
const HYSTERESIS: f64 = 0.12;

#[derive(Clone, Copy)]
struct Hub {
    /// Band of the last frame, with hysteresis applied.
    band: Tab,
    planet_ok: bool,
    system_ok: bool,
    /// A tab was chosen by hand since the map opened: stop following height.
    manual: bool,
    cfg: Option<engine_voxel::WorldSampleConfig>,
}

static HUB: Mutex<Hub> = Mutex::new(Hub { band: Tab::Region, planet_ok: false, system_ok: false, manual: false, cfg: None });

fn hub() -> std::sync::MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(|e| e.into_inner())
}

/// Height above the ground in metres and whether the player is off the home body.
fn height(world: &crate::world::World, player: &crate::player::Player) -> (f64, bool) {
    if let (Some(system), Some(fix)) = (space_nav::system(), space_nav::fix(world, player)) {
        let away = player.anchor.is_some() || player.void.is_some() || system.name(fix.reference) != "home";
        return (fix.altitude_m(system), away);
    }
    let cfg = world.sample_config();
    let eye_m = player.pos.as_dvec3() * f64::from(cfg.voxel_meters());
    (crate::altitude::altitude_m(eye_m, cfg.is_planet()), false)
}

fn band(previous: Tab, altitude_m: f64, away: bool, planet_ok: bool, system_ok: bool) -> Tab {
    let up = 1.0 + HYSTERESIS;
    let down = 1.0 - HYSTERESIS;
    let system_m = SYSTEM_RADII * engine_core::cube_sphere::atlas::RADIUS_M;
    let edge = |limit: f64, inside: bool| if inside { limit * down } else { limit * up };
    if system_ok && (away || altitude_m > edge(system_m, previous == Tab::System)) {
        Tab::System
    } else if planet_ok && altitude_m > edge(PLANET_M, previous != Tab::Region) {
        Tab::Planet
    } else {
        Tab::Region
    }
}

/// The map view that is open, if any.
pub fn current(creative: &CreativeState) -> Option<Tab> {
    if creative.show_world_map {
        Some(Tab::Region)
    } else if creative.show_planet_map {
        Some(Tab::Planet)
    } else if creative.show_system_map {
        Some(Tab::System)
    } else {
        None
    }
}

fn open(creative: &mut CreativeState, world: Option<&crate::world::World>, h: &Hub, tab: Tab) {
    creative.close_panels();
    match tab {
        Tab::Region => super::map::toggle_panel(creative, h.cfg),
        Tab::Planet => creative.show_planet_map = h.planet_ok,
        Tab::System => super::system_map::toggle(creative, world),
    }
    // A view that could not open falls back to the region map.
    if current(creative).is_none() {
        super::map::toggle_panel(creative, h.cfg);
    }
}

/// Map key: close any open map, or open the view for the current height.
pub fn toggle(creative: &mut CreativeState, world: Option<&crate::world::World>) {
    if current(creative).is_some() {
        creative.close_panels();
        return;
    }
    let mut h = hub();
    h.manual = false;
    let snapshot = *h;
    drop(h);
    open(creative, world, &snapshot, snapshot.band);
}

/// From a menu: open the map hub on `tab`, or on the height band when `None`.
pub fn open_from_menu(creative: &mut CreativeState, tab: Option<Tab>) {
    let mut h = hub();
    h.manual = tab.is_some();
    let snapshot = *h;
    drop(h);
    let tab = tab.unwrap_or(snapshot.band);
    // The menu has no world handle; the system view only needs the flag.
    creative.close_panels();
    match tab {
        Tab::Region => super::map::toggle_panel(creative, snapshot.cfg),
        Tab::Planet if snapshot.planet_ok => creative.show_planet_map = true,
        Tab::System if snapshot.system_ok => creative.show_system_map = true,
        _ => super::map::toggle_panel(creative, snapshot.cfg),
    }
}

/// Every frame in play: track the height band, follow it while a map is open,
/// draw the planet view and the tab bar over whichever map is showing.
pub fn frame(ctx: &egui::Context, creative: &mut CreativeState, world: &crate::world::World, player: &crate::player::Player) {
    let cfg = world.sample_config();
    let (altitude_m, away) = height(world, player);
    let planet_ok = planet_map::available(&cfg);
    let system_ok = world.world_type == crate::world::WorldType::Planet && space_nav::system().is_some();
    let snapshot = {
        let mut h = hub();
        h.cfg = Some(cfg);
        h.planet_ok = planet_ok;
        h.system_ok = system_ok;
        h.band = band(h.band, altitude_m, away, planet_ok, system_ok);
        if current(creative).is_none() {
            h.manual = false;
        }
        *h
    };
    let Some(shown) = current(creative) else { return };
    if !snapshot.manual && shown != snapshot.band {
        open(creative, Some(world), &snapshot, snapshot.band);
    }

    if creative.show_planet_map {
        let body = space_nav::system().zip(space_nav::fix(world, player))
            .map(|(system, fix)| space_nav::text::name(system, fix.reference));
        let input = planet_map::Inputs {
            cfg,
            player_voxel: player.pos,
            forward: player.forward_dir(),
            altitude_m: Some(altitude_m),
            body,
            waypoints: super::map::waypoint_marks(),
        };
        planet_map::draw(ctx, &mut creative.show_planet_map, &input);
    }
    if let Some(shown) = current(creative) {
        if let Some(pick) = tab_bar(ctx, shown, planet_ok, system_ok) {
            hub().manual = true;
            open(creative, Some(world), &snapshot, pick);
        }
    }
}

/// Segmented control centred at the top of the screen; returns a clicked tab.
fn tab_bar(ctx: &egui::Context, shown: Tab, planet_ok: bool, system_ok: bool) -> Option<Tab> {
    let theme = super::themes::current(ctx);
    let tabs: Vec<(Tab, &str)> = TABS.into_iter().filter(|(t, _)| match t {
        Tab::Region => true,
        Tab::Planet => planet_ok,
        Tab::System => system_ok,
    }).collect();
    if tabs.len() < 2 {
        return None;
    }
    let screen = ctx.content_rect();
    let w = 104.0;
    let h = 30.0;
    let total = vec2(w * tabs.len() as f32 + 8.0, h + 8.0);
    let at = pos2(screen.center().x - total.x * 0.5, screen.top() + 14.0);
    let mut picked = None;
    egui::Area::new(Id::new("map_hub_tabs"))
        .order(Order::Tooltip)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let outer = Rect::from_min_size(at, total);
            let p = ui.painter().clone();
            p.add(egui::epaint::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(outer, 8));
            p.rect_filled(outer, 8.0, theme.panel.gamma_multiply(0.92));
            p.rect_stroke(outer, 8.0, Stroke::new(1.0, theme.edge), egui::StrokeKind::Inside);
            for (i, (tab, name)) in tabs.iter().enumerate() {
                let r = Rect::from_min_size(at + vec2(4.0 + i as f32 * w, 4.0), vec2(w, h));
                let response = ui.allocate_rect(r, Sense::click());
                let selected = *tab == shown;
                if selected {
                    p.rect_filled(r, 6.0, theme.accent.gamma_multiply(0.22));
                    p.rect_stroke(r, 6.0, Stroke::new(1.0, theme.accent.gamma_multiply(0.8)), egui::StrokeKind::Inside);
                } else if response.hovered() {
                    p.rect_filled(r, 6.0, theme.hover());
                }
                let colour = if selected { theme.text() } else { theme.muted() };
                p.text(r.center(), Align2::CENTER_CENTER, *name, FontId::proportional(15.0), colour);
                if response.clicked() && !selected {
                    picked = Some(*tab);
                }
            }
        });
    picked
}

