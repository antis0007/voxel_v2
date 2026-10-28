//! Signature guardians: site-to-kind mapping, names, and dispatch to each
//! designed boss module. There is no generic boss path: every kind owns its
//! committed actions, presentation and counterplay.
use super::*;
pub(super) fn kind(site: u32) -> u32 {
    match site {
        10 => tollkeeper::KIND,
        3 => engine::KIND,
        5 => glassweaver::KIND,
        8 => 8,
        _ => 0,
    }
}
pub fn name(kind: u32) -> &'static str {
    match kind {
        matron::KIND => "The Orchard Matron",
        tollkeeper::KIND => "The Last Tollkeeper",
        engine::KIND => "The Ossuary Engine",
        glassweaver::KIND => "The Glassweaver",
        _ => "The Prism Hart",
    }
}
/// The site variant's full title, e.g. "The Last Tollkeeper — Ironbound".
pub(super) fn variant_title(kind: u32, variant: usize) -> &'static str {
    const TOLL: [&str; 3] = [
        "The Last Tollkeeper — Ironbound",
        "The Last Tollkeeper — Verdigris",
        "The Last Tollkeeper — Ashwarden",
    ];
    const MATRON: [&str; 3] = [
        "The Orchard Matron — Bramble",
        "The Orchard Matron — Blossom",
        "The Orchard Matron — Mire",
    ];
    const ENGINE: [&str; 3] = [
        "The Ossuary Engine — Foundry",
        "The Ossuary Engine — Bastion",
        "The Ossuary Engine — Siege",
    ];
    const GLASS: [&str; 3] = [
        "The Glassweaver — Prismatic",
        "The Glassweaver — Smoked",
        "The Glassweaver — Aurora",
    ];
    const HART: [&str; 3] = ["Rime Hart · Sentinel", "Amethyst Hart · Sentinel", "Verdant Hart · Sentinel"];
    let v = variant.min(2);
    match kind {
        tollkeeper::KIND => TOLL[v],
        matron::KIND => MATRON[v],
        engine::KIND => ENGINE[v],
        glassweaver::KIND => GLASS[v],
        _ => HART[v],
    }
}
/// The name shown on the boss bar for this particular guardian.
pub(super) fn title(e: &Enemy) -> &'static str {
    if e.kind == 8 {
        hart::name(e)
    } else {
        variant_title(e.kind, boss_look::index(e))
    }
}
pub(super) fn health(kind: u32) -> f32 {
    match kind {
        tollkeeper::KIND => 240.0,
        engine::KIND => 360.0,
        glassweaver::KIND => 280.0,
        _ => 320.0,
    }
}
/// Sigil colours for the workshop sockets (kinds 6, 7, 8).
pub(super) fn color(kind: u32) -> [f32; 3] {
    match kind {
        engine::KIND => [1.0, 0.36, 0.08],
        glassweaver::KIND => [0.72, 0.42, 1.0],
        _ => [0.25, 0.85, 1.0],
    }
}
pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    match e.kind {
        matron::KIND => matron::model(e, m, time),
        engine::KIND => engine::model(e, m, time),
        tollkeeper::KIND => tollkeeper::model(e, m, time),
        glassweaver::KIND => glassweaver::model(e, m, time),
        _ => hart::model(e, m, time),
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    hazards: &mut Vec<tactics::Hazard>,
    sounds: &mut Vec<Sound>,
    dirty: &mut Vec<IVec3>,
    navigation_budget: &mut usize,
) -> (f32, Vec3) {
    match e.kind {
        matron::KIND => matron::tick(e, world, feet, eye, dt, shots, sounds, navigation_budget),
        engine::KIND => engine::tick(e, world, feet, eye, dt, shots, sounds, dirty, navigation_budget),
        tollkeeper::KIND => tollkeeper::tick(e, world, feet, eye, dt, sounds, dirty),
        glassweaver::KIND => glassweaver::tick(e, world, feet, eye, dt, shots, sounds, navigation_budget),
        _ => hart::tick(e, world, feet, eye, dt, shots, hazards, sounds, dirty),
    }
}
