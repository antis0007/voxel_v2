//! Contextual inspection panel for the block
//! under the crosshair. Non-interactive, compact, top centre.
//!
//! Layout, top to bottom:
//! * a small icon, the name, and a temperature chip when the cell is hot;
//! * the material category in the accent colour,
//!   with state, hardness pips and distance on the same line;
//! * for an assembly, a status dot and only the readings that matter for its
//!   kind: power in W, stored energy in Wh, batch progress with time left,
//!   hearth temperature, multiblock shell, buffers and the grid balance;
//! * controls as keycaps (`F  Inspect · E  Inventory`).
//!
//! All data is passed in; this module reads no world state.
use super::icons::{self, Icon};
use super::presentation::{AMBER, DANGER, EDGE, JADE, MANA, MUTED, PAPER};
use crate::material::{BlockId, CollisionKind, MatterState};
use crate::world::expedition::NAMES;
use crate::world::factory::{Kind, Probe, assembler, induction, multiblock, pile, power};
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, vec2};

pub struct LookAt<'a> {
    pub block: BlockId,
    /// Measured cell temperature, including ambient and frozen states.
    pub temperature_c: Option<f32>,
    /// What the climate alone would hold the cell at. The temperature chip shows
    /// only when the cell departs from it (or on Shift): `20°C` on every brick is noise.
    pub ambient_c: Option<f32>,
    /// Survival mining facts for the held tool; `None` in creative or for cells
    /// with nothing to mine.
    pub harvest: Option<Harvest>,
    pub volume_ul: Option<u64>,
    pub water: Option<crate::world::water_tools::Reading>,
    pub bucket_ul: Option<u32>,
    pub environment: Option<crate::world::environment::inspect::Inspection>,
    pub distance_m: Option<f32>,
    /// Control hint from `expedition::hint`, e.g. `F  Inspect   ·   E  Inventory`.
    pub hint: &'a str,
    /// The assembly at the crosshair, when the block is one.
    pub machine: Option<&'a Probe>,
}

/// The Jade harvest line: which tool the block needs and whether the held one
/// gets a drop.
#[derive(Clone, Copy, Debug)]
pub struct Harvest {
    /// Cheapest craftable tool that harvests the block, when one is needed.
    pub tool: Option<crate::world::inventory::Item>,
    /// Minimum tool tier; 0 means the hand drops it.
    pub tier: u8,
    /// Hold time with the held tool.
    pub seconds: f32,
    /// False when the held tool is below the tier: the block breaks with no drop.
    pub drops: bool,
}

/// Degrees from ambient before a temperature counts as news.
const TEMPERATURE_NEWS_C: f32 = 10.0;

/// Seconds the panel takes to settle when the target changes.
const SETTLE: f64 = 0.12;
const WIDTH: f32 = 240.0;
const SMALL: f32 = 13.0;

fn scaled(ui: &egui::Ui, value: f32) -> f32 {
    value * super::typography::inspector_scale(ui.ctx())
}

fn label_text(ui: &egui::Ui, value: impl Into<String>) -> RichText {
    RichText::new(value).family(super::typography::inspector_family()).size(scaled(ui, SMALL))

}

pub fn draw(ctx: &egui::Context, look: &LookAt) {
    // Restart the fade-in whenever the target block changes.
    let state_id = egui::Id::new("look_at_state");
    let now = ctx.input(|i| i.time);
    let (last, since) = ctx
        .data_mut(|d| d.get_temp::<(u32, f64)>(state_id))
        .unwrap_or((u32::MAX, now));
    let since = if last == look.block.as_u32() { since } else { now };
    ctx.data_mut(|d| d.insert_temp(state_id, (look.block.as_u32(), since)));
    let appear = if super::magic_fx::reduced(ctx) { 1.0 } else {
        ((((now - since) / SETTLE) as f32).clamp(0.0, 1.0) * 4.0).floor() / 4.0
    };

    // Below the compass when it shows; the rise-in stays a two-point step.
    let slot = super::hud_layout::place(ctx, "look_at");
    let shown = egui::Area::new(egui::Id::new("target_name"))
        .anchor(slot.align, slot.offset - vec2(0.0, (1.0 - appear) * 2.0))
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_opacity(appear);
            egui::Frame::new()
                .fill(if super::themes::get(ctx) == 0 { Color32::from_rgba_unmultiplied(22, 19, 23, 228) }
                    else { super::themes::current(ctx).panel })
                .stroke(Stroke::new(1.0, if super::themes::get(ctx) == 0 { EDGE }
                    else { super::themes::current(ctx).edge }))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    let width = scaled(ui, WIDTH).min((ctx.content_rect().width() - 32.0).max(80.0));
                    ui.set_width(width);
                    ui.spacing_mut().interact_size.y = scaled(ui, 16.0);
                    ui.spacing_mut().item_spacing = vec2(scaled(ui, 10.0), scaled(ui, 5.0));
                    header(ui, look);
                    if let Some(water) = look.water {
                        let prefix = if water.partial || water.scanning { "≥ " } else { "≈ " };
                        row(ui, "Pool", &format!("{prefix}{}", crate::world::water_tools::volume(water.ul)), MANA);
                        if water.scanning { ui.label(label_text(ui, "Measuring").color(MUTED)); }
                        else if water.partial { ui.label(label_text(ui, "Partial Volume").color(MUTED)); }
                    }
                    if let Some(amount) = look.bucket_ul {
                        let shown = crate::world::water_tools::bucket_contents(amount);
                        row(ui, "Bucket", &shown, MANA);
                        keycap_row(ui, if matches!(look.block, BlockId::Water | BlockId::Lava | BlockId::SulfuricAcid) { "RMB  Scoop · Shift+RMB  Pour" } else { "RMB  Pour" });
                    }
                    if let Some(environment) = look.environment {
                        ui.label(label_text(ui, environment.condition).color(AMBER));
                        if ui.input(|i| i.modifiers.shift) {
                            row(ui, "Water", &format!("{:.3} L", environment.water_ul as f64 / 1_000_000.0), MANA);
                            if environment.sediment_mg > 0 {
                                row(ui, "Sediment", &format!("{:.3} kg", environment.sediment_mg as f64 / 1_000_000.0), AMBER);
                            }
                        }
                    }
                    if let Some(e) = look.environment {
                        if e.pollution_mg>0 {row(ui,"Pollution",&format!("{:.2} g",e.pollution_mg as f64/1000.0),AMBER);}
                        for (i,c) in e.air.iter().enumerate().filter(|(_,v)|**v>=1.0) {row(ui,&crate::world::industry::config::CONFIG.species[i].name,&format!("{c:.1} mg/m³"),AMBER);}
                    }
                    if let Some(probe) = look.machine {
                        ui.add_space(1.0);
                        machine(ui, probe);
                    }
                    if ui.input(|i| i.modifiers.shift) && !look.hint.is_empty()
                        && !crate::world::interaction::block_interactable(look.block) {
                        keycap_row(ui, look.hint);
                    }
                });
        });
    super::hud_layout::claim(ctx, "look_at", shown.response.rect);
}

fn header(ui: &mut egui::Ui, look: &LookAt) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(scaled(ui, 32.0), scaled(ui, 32.0)), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_rgb(16, 14, 17));
        let (sides, accent) = if look.temperature_c.is_some_and(|t| t > 300.0) {
            (3, DANGER)
        } else if look.machine.is_some() {
            (5, AMBER)
        } else {
            (6, JADE)
        };
        super::sigils::draw(painter, rect.center(), scaled(ui, 14.0), sides, accent);
        super::draw_voxel_icon(painter, rect.shrink(scaled(ui, 4.0)), look.block);
        // One brief acquisition glint; a held target never keeps sparkling.
        let now = ui.ctx().input(|i| i.time);
        let since = ui.ctx().data(|d| d.get_temp::<(u32, f64)>(egui::Id::new("look_at_state")))
            .map_or(now, |(_, since)| since);
        let age = now - since;
        if age > 0.04 && age < 0.28 && !super::magic_fx::reduced(ui.ctx()) {
            let pixel = 1.0 / ui.ctx().pixels_per_point();
            let c = rect.right_top() + vec2(-2.0, 2.0);
            for offset in [vec2(0.0,0.0), vec2(pixel,0.0), vec2(-pixel,0.0), vec2(0.0,pixel), vec2(0.0,-pixel)] {
                let q = c + offset;
                let q = egui::pos2((q.x / pixel).round() * pixel, (q.y / pixel).round() * pixel);
                painter.rect_filled(egui::Rect::from_min_size(q, vec2(pixel,pixel)), 0.0, PAPER);
            }
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.horizontal_wrapped(|ui| {
                let name = look
                    .machine
                    .and_then(|m| m.shell_of)
                    .map(|_| look.block.display_name())
                    .unwrap_or(look.block.display_name());
                ui.label(label_text(ui, name).size(scaled(ui, 12.0)).color(PAPER));
                let shift = ui.input(|i| i.modifiers.shift);
                if let Some(temp_c) = look.temperature_c.filter(|t| {
                    shift || look.ambient_c.is_none_or(|a| (t - a).abs() >= TEMPERATURE_NEWS_C)
                }) {
                    let colour = if temp_c > 300.0 { DANGER } else { AMBER };
                    ui.label(label_text(ui, format!("{temp_c:.0}°C")).color(colour));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                ui.label(
                    label_text(ui, crate::world::inventory::Item::Block(look.block).category())

                        .color(JADE),
                );
                facts(ui, look);
            });
            if let Some(harvest) = look.harvest {
                harvest_line(ui, harvest);
            }
        });
    });
}

/// `Requires: Stone pickaxe · 1.2 s`, red with `No Drop` when the held tool is
/// under tier. A hand-harvestable block shows only its time, and only on Shift.
fn harvest_line(ui: &mut egui::Ui, h: Harvest) {
    let shift = ui.input(|i| i.modifiers.shift);
    if h.tier == 0 && !shift {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        if h.tier > 0 {
            let need = h.tool.map_or_else(|| format!("Tier {}", h.tier), |tool| tool.name().to_owned());
            ui.label(label_text(ui, format!("Requires: {need}")).color(if h.drops { JADE } else { DANGER }));
            if !h.drops {
                ui.label(label_text(ui, "No Drop").color(DANGER));
            }
        }
        ui.label(label_text(ui, format!("{:.1} s", h.seconds)).color(MUTED));
    });
}

/// State, hardness and distance on the category line, only when they say
/// something: machines skip matter state, air-like cells skip hardness.
fn facts(ui: &mut egui::Ui, look: &LookAt) {
    let material = look.block.material();
    let solid = material.physics.matter_state == MatterState::Solid;
    if look.machine.is_none() {
        let state = match material.physics.matter_state {
            MatterState::Solid => None,
            MatterState::Powder => Some(("Powder", AMBER)),
            MatterState::Liquid => Some(("Liquid", MANA)),
            MatterState::Gas => Some(("Gas", MUTED)),
            MatterState::Plasma => Some(("Plasma", DANGER)),
            MatterState::Vacuum => Some(("Vacuum", MUTED)),
        };
        if let Some((text, colour)) = state {
            ui.label(label_text(ui, text).color(colour));
        }
        if (!solid || ui.input(|i| i.modifiers.shift))
            && let Some(volume) = look.volume_ul {
                ui.label(label_text(ui, format!("Cell: {}", crate::world::water_tools::volume(volume)))
                    .color(MUTED));
            }
        if ui.input(|i| i.modifiers.shift)
            && material.physics.collision == CollisionKind::Passable && solid {
            ui.label(label_text(ui, "Passable").color(MUTED));
        }
        if ui.input(|i| i.modifiers.shift) && solid && material.physics.hardness > 0.0 {
            hardness_pips(ui, material.physics.hardness);
        }
    }
    if let Some(distance) = look.distance_m.filter(|_| ui.input(|i| i.modifiers.shift)) {
        ui.label(label_text(ui, format!("{distance:.1} m")).color(MUTED));
    }
}

/// Five pips; hardness 0..1.1 maps onto them.
fn hardness_pips(ui: &mut egui::Ui, hardness: f32) {
    let filled = ((hardness / 1.1) * 5.0).ceil().clamp(1.0, 5.0) as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(5.0 * 4.0, 10.0), Sense::hover());
    let painter = ui.painter();
    for i in 0..5 {
        let pip = egui::Rect::from_min_size(rect.min + vec2(i as f32 * 4.0, 3.0), vec2(3.0, 5.0));
        painter.rect_filled(pip, 0.0, if i < filled { JADE } else { EDGE });
    }
}

/// `label ........ value` on one line.
fn row(ui: &mut egui::Ui, label: &str, value: &str, colour: Color32) {
    ui.horizontal(|ui| {
        ui.label(label_text(ui, label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(label_text(ui, value).color(colour));
        });
    });
}

/// A thin bar with its caption drawn inside.
fn bar(ui: &mut egui::Ui, fraction: f32, colour: Color32, caption: &str) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, scaled(ui, 32.0)), Sense::hover());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, Color32::from_rgb(14, 12, 16));
    let mut fill = rect;
    fill.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    p.rect_filled(fill, 0.0, colour.gamma_multiply(0.75));
    p.rect_stroke(rect, 0.0, Stroke::new(1.0, EDGE), egui::StrokeKind::Inside);
    p.text(
        rect.left_center() + vec2(3.0, 0.0),
        Align2::LEFT_CENTER,
        caption,
        FontId::new(scaled(ui, SMALL), super::typography::inspector_family()),
        PAPER,
    );
}

/// A row of tiny item icons with counts, for buffers.
fn stacks(ui: &mut egui::Ui, label: &str, items: &[(usize, u32)]) {
    if items.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.label(label_text(ui, label).color(MUTED));
        for &(i, n) in items.iter().take(5) {
            let (r, _) = ui.allocate_exact_size(vec2(scaled(ui, 18.0), scaled(ui, 18.0)), Sense::hover());
            icons::paint(ui.painter(), r, Icon::Resource(i));
            let count = if n % 64 == 0 {
                format!("{}", n / 64)
            } else {
                format!("{:.2}", n as f32 / 64.0)
            };
            ui.label(label_text(ui, count).color(PAPER))
                .on_hover_text(NAMES[i]);
        }
    });
}

fn status_colour(status: &str, enabled: bool) -> Color32 {
    if !enabled {
        return MUTED;
    }
    match status {
        "Processing" | "Generating" | "Coking" | "Smelting" | "Charging" | "Full" => {
            Color32::from_rgb(120, 214, 150)
        }
        "Discharging" | "Holding" | "Heating" | "Balancing" => AMBER,
        "Idle" | "Ready" => MUTED,
        _ => DANGER,
    }
}

fn machine(ui: &mut egui::Ui, probe: &Probe) {
    let n = &probe.node;
    // Status line: coloured dot, status, and the live power flow.
    ui.horizontal(|ui| {
        let colour = status_colour(n.status, n.enabled);
        let (dot, _) = ui.allocate_exact_size(vec2(7.0, 10.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 3.0, colour);
        let status = if n.enabled { super::factory::status_label(n.status) } else { "Paused" };
        ui.label(label_text(ui, status).strong().color(colour));
        if n.upgrade > 0 {
            let tier = if n.kind == Kind::Battery {
                power::battery_tier_name(n.upgrade).to_string()
            } else {
                format!("Drive ×{}", n.upgrade + 1)
            };
            ui.label(label_text(ui, tier).color(MANA));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if n.flow != 0 {
                let (text, colour) = if n.flow > 0 {
                    (format!("+{}", power::fmt_power(n.flow as f64)), Color32::from_rgb(120, 214, 150))
                } else {
                    (format!("−{}", power::fmt_power(-n.flow as f64)), AMBER)
                };
                ui.label(label_text(ui, text).color(colour));
            }
        });
    });
    if let Some(controller) = &probe.controller {
        ui.horizontal(|ui| {
            let (art, _) = ui.allocate_exact_size(vec2(15.0, 12.0), Sense::hover());
            super::chip_art::package(ui.painter(), art, MANA);
            ui.label(label_text(ui, controller).color(MANA));
        });
    }
    if !ui.input(|i| i.modifiers.shift) {
        if let Some((name, progress)) = probe.lab_work {
            bar(ui, progress, JADE, &format!("{name} · {:.0}%", progress * 100.0));
        } else if let Some(process) = n.process() {
            bar(ui, n.progress as f32 / process.ticks.max(1) as f32, JADE,
                &format!("{} → {}  {:.0}%", short(NAMES[process.inputs[0].0]),
                    short(NAMES[process.output.0]),
                    n.progress as f32 * 100.0 / process.ticks.max(1) as f32));
        }
        if let Some((stored, capacity, _)) = probe.pool {
            bar(ui, stored as f32 / capacity.max(1) as f32, MANA,
                &format!("{} / {}", power::fmt_energy(stored as f64),
                    power::fmt_energy(capacity as f64)));
        } else if n.kind.powered() {
            bar(ui, n.energy as f32 / power::CONSUMER_BUFFER as f32, MANA,
                &format!("{} / {}", power::fmt_joules(n.energy as f64),
                    power::fmt_joules(power::CONSUMER_BUFFER as f64)));
        }
        if n.water > 0 || n.kind.water_port() {
            bar(ui, n.water as f32 / 8192.0, MANA,
                &format!("{:.1} / 16,000 L", n.water as f32 * 1000.0 / 512.0));
        }
        return;
    }
    if let Some(mode) = n.kind.modes().get(n.mode as usize)
        && (n.kind != Kind::Furnace || n.mode == 1) {
            let short = mode.split(':').next().unwrap_or(mode);
            row(ui, "Mode", short, PAPER);
        }
    match n.kind {
        Kind::Battery | Kind::InductionPort | Kind::Transformer => {
            if let Some((stored, cap, rate)) = probe.pool {
                bar(
                    ui,
                    stored as f32 / cap.max(1) as f32,
                    MANA,
                    &format!(
                        "{} / {}  {:.0}%",
                        power::fmt_energy(stored as f64),
                        power::fmt_energy(cap as f64),
                        stored as f64 * 100.0 / cap.max(1) as f64
                    ),
                );
                row(ui, "Rate limit", &power::fmt_power(rate as f64), MUTED);
            }
            if n.kind == Kind::InductionPort {
                if n.formed {
                    row(
                        ui,
                        "Matrix",
                        &format!("{} cells · {} providers", n.structure.0, n.structure.1),
                        JADE,
                    );
                } else {
                    row(ui, "Matrix", induction::problem(n.structure.1), DANGER);
                }
            }
        }
        Kind::InductionCasing | Kind::InductionCell | Kind::InductionProvider => {
            if n.formed {
                row(ui, "Part of", "formed induction matrix", JADE);
            } else {
                row(ui, "Matrix", induction::problem(n.structure.1), DANGER);
            }
            match n.kind {
                Kind::InductionCell => row(ui, "Adds", "64 kWh", MUTED),
                Kind::InductionProvider => row(ui, "Adds", "256 kW", MUTED),
                _ => {}
            }
        }
        Kind::Cable => {
            let cap = power::cable_capacity(n.upgrade);
            let used = probe.grid.map(|g| g.throughput).unwrap_or(0);
            row(ui, "Tier", power::cable_tier_name(n.upgrade), PAPER);
            bar(
                ui,
                used as f32 / cap as f32,
                if used >= cap { DANGER } else { MANA },
                &format!("{} / {}", power::fmt_power(used as f64), power::fmt_power(cap as f64)),
            );
        }
        Kind::Assembler => {
            match assembler::recipe(n) {
                Some(r) => {
                    row(ui, "Recipe", r.name, PAPER);
                    bar(
                        ui,
                        n.progress as f32 / assembler::CRAFT_TICKS as f32,
                        Color32::from_rgb(110, 190, 160),
                        &format!("{:.0}%", n.progress as f32 * 100.0 / assembler::CRAFT_TICKS as f32),
                    );
                    if n.stock > 0 {
                        row(ui, "Finished", &format!("{} kits", n.stock), JADE);
                    }
                }
                None => row(ui, "Recipe", "none set", DANGER),
            }
        }
        Kind::Generator | Kind::Bioreactor => {
            if pile::is_pile(n) {
                let colour = if n.progress >= pile::WARN_HEAT { DANGER } else { AMBER };
                bar(
                    ui,
                    n.progress as f32 / pile::MAX_HEAT as f32,
                    colour,
                    &format!("Core heat {} / {}", n.progress, pile::MAX_HEAT),
                );
            }
            if n.burn > 0 {
                let rate = if n.kind == Kind::Bioreactor { 32.0 } else if pile::is_pile(n) { pile::RATE as f64 } else { 16.0 };
                row(
                    ui,
                    "Burning",
                    &format!(
                        "{} · {:.0} s",
                        power::fmt_joules(n.burn as f64),
                        n.burn as f64 / rate * power::TICK_SECONDS
                    ),
                    PAPER,
                );
            }
            bar(
                ui,
                n.energy as f32 / power::GENERATOR_BUFFER as f32,
                AMBER,
                &format!(
                    "Buffer {} / {}",
                    power::fmt_energy(n.energy as f64),
                    power::fmt_energy(power::GENERATOR_BUFFER as f64)
                ),
            );
        }
        Kind::CokeOven | Kind::BlastFurnace => {
            let (found, needed) = n.structure;
            if n.formed {
                row(ui, "Shell", &format!("formed · {needed} bricks"), JADE);
            } else {
                row(ui, "Shell", &format!("{found} / {needed} bricks"), DANGER);
            }
            let t = multiblock::temperature(n);
            let target = multiblock::working_temperature(n.kind, n.mode);
            let max = if n.kind == Kind::CokeOven {
                multiblock::COKE_OVEN_MAX_C
            } else {
                multiblock::BLAST_MAX_C
            };
            bar(
                ui,
                t as f32 / max as f32,
                if t >= target { Color32::from_rgb(240, 120, 50) } else { AMBER },
                &format!("Hearth {t} °C · works at {target} °C"),
            );
        }
        _ => {}
    }
    if let Some((name, progress)) = probe.lab_work {
        bar(ui, progress, JADE, &format!("{name} · {:.0}%", progress * 100.0));
    } else if let Some(process) = n.process() {
        let progress = n.progress as f32 / process.ticks.max(1) as f32;
        let speed = if n.kind.powered() { 1 + n.upgrade } else { 1 };
        let left = (process.ticks - n.progress.min(process.ticks)) as f32 * 0.25 / speed as f32;
        let input = process.inputs[0].0;
        bar(
            ui,
            progress,
            Color32::from_rgb(110, 190, 160),
            &format!(
                "{} → {}  {:.0}% · {:.1} s",
                short(NAMES[input]),
                short(NAMES[process.output.0]),
                progress * 100.0,
                left
            ),
        );
    }
    if n.kind.powered() {
        row(
            ui,
            "Local buffer",
            &format!(
                "{} / {}",
                power::fmt_joules(n.energy as f64),
                power::fmt_joules(power::CONSUMER_BUFFER as f64)
            ),
            MUTED,
        );
    }
    if n.water > 0 || (n.kind.water_port() && n.kind != Kind::Generator) {
        bar(
            ui,
            n.water as f32 / 8192.0,
            MANA,
            &format!("Water {:.0} / 16000 L", n.water as f32 * 1000.0 / 512.0),
        );
    }
    let mut inputs: Vec<(usize, u32)> = (0..NAMES.len()).filter(|&i| n.input[i] > 0).map(|i| (i, n.input[i])).collect();
    inputs.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
    let mut outputs: Vec<(usize, u32)> = (0..NAMES.len()).filter(|&i| n.output[i] > 0).map(|i| (i, n.output[i])).collect();
    outputs.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
    if n.kind == Kind::Storage || n.kind == Kind::Feeder {
        inputs.append(&mut outputs);
        stacks(ui, "Holds", &inputs);
    } else {
        stacks(ui, "In", &inputs);
        stacks(ui, "Out", &outputs);
    }
    if let Some(grid) = probe.grid {
        let balance = grid.generation as i64 - grid.load as i64;
        let mut text = format!(
            "+{} / −{}",
            power::fmt_power(grid.generation as f64),
            power::fmt_power(grid.load as f64)
        );
        if grid.capacity > 0 {
            text.push_str(&format!(" · {:.0}%", grid.stored as f64 * 100.0 / grid.capacity as f64));
        }
        row(
            ui,
            "Grid",
            &text,
            if balance >= 0 { MUTED } else if grid.stored > 0 { AMBER } else { DANGER },
        );
        if let Some(s) = grid.runtime_seconds() {
            row(ui, "Reserve lasts", &duration(s), AMBER);
        }
    } else if n.kind.powered() || n.kind.generator() || n.kind.stores_power() {
        row(ui, "Grid", "no power cable", DANGER);
    }
    if let Some(kind) = probe.shell_of {
        let name = if kind == Kind::CokeOven { "Coke oven" } else { "Blast furnace" };
        row(ui, "Part of", name, JADE);
    }
}

fn short(name: &str) -> &str {
    name.strip_suffix(" ingots")
        .or_else(|| name.strip_suffix(" dust"))
        .unwrap_or(name)
}

fn duration(seconds: f64) -> String {
    if seconds >= 3600.0 {
        format!("{:.1} h", seconds / 3600.0)
    } else if seconds >= 60.0 {
        format!("{:.0} min", seconds / 60.0)
    } else {
        format!("{seconds:.0} s")
    }
}

/// `F  Inspect   ·   E  Inventory` becomes `[F] Inspect  [E] Inventory`. A
/// segment without a double-space is plain explanatory text.
fn keycap_row(ui: &mut egui::Ui, hint: &str) {
    ui.add_space(1.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for segment in hint.split('·').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some((key, action)) = segment.split_once("  ") {
                keycap(ui, key.trim());
                ui.label(label_text(ui, action.trim()).color(PAPER));
            }
        }
    });
}

fn keycap(ui: &mut egui::Ui, key: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(52, 46, 50))
        .stroke(Stroke::new(1.0, EDGE))
        .inner_margin(egui::Margin::symmetric(3, 0))
        .show(ui, |ui| {
            ui.label(label_text(ui, format!("[{}]", crate::keybinds::rebound_label(key))).strong().color(PAPER));
        });
}
