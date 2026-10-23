//! Rotating orbital tethers in the celestial layer. Hardware keeps its true
//! size; at hundreds of kilometres it is a few pixels at most. The spectacle
//! comes from the tether's projected length sweeping across the sky, drawn as
//! a coverage line so it stays continuous instead of dotted.
use super::{Model, Quat, Vec3, architecture::{part, strut}, megastructures::long_range_since, tether};
use crate::world::{World, DayNightCycle, terrain_height, orbital::{megastructures::{PENUMBRA_M, PLANET_RADIUS_M}, skyhook::{self, Size}}};

const BRAID: [f32; 3] = [0.58, 0.55, 0.47];
const METAL: [f32; 3] = [0.22, 0.29, 0.32];
const SHELL: [f32; 3] = [0.67, 0.69, 0.65];
const POD: [f32; 3] = [1.0, 0.92, 0.55];
/// Half-width of the dwell window around a port cusp, as a fraction of a tether turn.
const DWELL_TURNS: f32 = 0.08;

fn braid_width(size: Size) -> f32 {
    match size { Size::Courier => 2.0, Size::Talon => 8.0, Size::Ark => 30.0 }
}

/// Station scale: hub truss length and grapple size, metres.
fn hardware(size: Size) -> (f32, f32) {
    match size { Size::Courier => (300.0, 40.0), Size::Talon => (1_200.0, 120.0), Size::Ark => (6_000.0, 400.0) }
}

/// The hook's guarded ground port: on the equator chart, at the terrain surface.
fn port_on_ground(world: &World, port_lon: f64) -> Vec3 {
    let cfg = world.sample_config();
    let x = (port_lon * PLANET_RADIUS_M) as f32;
    let ground = cfg.world_y(terrain_height(&cfg, (x * cfg.inv_voxel_meters()).floor() as i32, 0));
    Vec3::new(x, ground.max(cfg.world_y(cfg.water_level())), 0.0)
}

/// Soft cylindrical eclipse in the chart frame, as in the megastructure path.
fn sunlight(p: Vec3, centre: Vec3, sun: Vec3) -> f32 {
    let r = p - centre;
    let behind = r.dot(sun);
    if behind >= 0.0 { return 1.0; }
    (((r - sun * behind).length() - PLANET_RADIUS_M as f32) / PENUMBRA_M as f32 + 0.5).clamp(0.0, 1.0)
}

pub(super) fn build(mesh: &mut Model, world: &World, day: &DayNightCycle, viewer: Vec3, pixel: f32) {
    let sun = day.lighting().sun_dir.normalize();
    let centre = Vec3::new(viewer.x, -(PLANET_RADIUS_M as f32), viewer.z);
    for c in skyhook::contacts(world.seed, day, viewer) {
        let h = c.hook;
        let mark = mesh.mark();
        let axis = (c.tip - c.hub).normalize();
        let (truss, grapple) = hardware(h.size);
        // Counter-arm: a short stub past the hub carries the ballast mass.
        let ballast = c.hub - axis * (h.tether_m as f32 * 0.12);
        tether(mesh, c.hub, c.tip, viewer, pixel, BRAID, braid_width(h.size));
        tether(mesh, ballast, c.hub, viewer, pixel, BRAID, braid_width(h.size));
        // A round braid always shows its sunward side. The flat ribbon's own
        // normal can face away and turn a sunlit cable black at twilight.
        let lit = (sun - axis * sun.dot(axis)).normalize_or_zero();
        for v in &mut mesh.vertices[mark..] { v.normal = lit.to_array(); }
        let station = mesh.mark();
        strut(mesh, c.hub - Vec3::Y * truss * 0.5, c.hub + Vec3::Y * truss * 0.5, truss * 0.02, METAL);
        part(mesh, c.hub, Vec3::splat(truss * 0.12), Quat::IDENTITY, SHELL, 0.5);
        part(mesh, ballast, Vec3::splat(truss * 0.3), Quat::IDENTITY, METAL, 0.0);
        part(mesh, c.tip, Vec3::new(grapple, grapple * 0.4, grapple), Quat::IDENTITY, SHELL, 0.5);
        // Truss and grapple align with the tether; vertical in their local frame.
        mesh.rotate_since(station, c.hub, Quat::from_rotation_arc(Vec3::Y, -axis));
        // Ark second stage: the drop line pays out around the port cusp only.
        if h.size.drop_line_m() > 0.0 {
            let to_next = c.next_port_cusp_s;
            let since = h.pass_period() as f32 - to_next;
            let dt = if to_next < since { -to_next } else { since };
            let dwell = h.turn_period() as f32 * DWELL_TURNS;
            if dt.abs() < dwell {
                let reach = (std::f32::consts::FRAC_PI_2 * (1.0 - dt.abs() / dwell)).sin();
                let drop = c.tip + axis * h.size.drop_line_m() as f32 * reach;
                tether(mesh, c.tip, drop, viewer, pixel, BRAID, braid_width(h.size) * 0.5);
                part(mesh, drop, Vec3::splat(grapple * 0.6), Quat::IDENTITY, METAL, 0.5);
            }
        }
        // Cargo pod: climbs from the guarded ground port to meet the tip, rides the
        // grapple, then is hauled in. Orbital-clock driven; nothing is saved.
        if let Some(at) = c.pod(port_on_ground(world, h.port_lon)) {
            // Never less than ~14 px across, so the catch can be watched from the ground.
            let pod_size = (grapple * 0.6).max((at - viewer).length() * pixel * 14.0);
            part(mesh, at, Vec3::splat(pod_size), Quat::IDENTITY, POD, 0.9);
        }
        long_range_since(mesh, mark);
        for v in &mut mesh.vertices[mark..] {
            v.ao_strength = 4.0 + sunlight(Vec3::from_array(v.pos) * mesh.scale, centre, sun);
        }
    }
}
