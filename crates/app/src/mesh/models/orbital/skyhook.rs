//! Rotating orbital tethers in the celestial layer. Hardware keeps its true
//! size; at hundreds of kilometres it is a few pixels at most. The spectacle
//! comes from the tether's projected length sweeping across the sky, drawn as
//! a coverage line so it stays continuous instead of dotted.
use super::{Model, Quat, Vec3, architecture::{part, strut}, megastructures::long_range_since, tether};
use crate::world::{World, DayNightCycle, terrain_height, orbital::{megastructures::{PENUMBRA_M, PLANET_RADIUS_M}, skyhook::{self, Size}}};

use super::palette::{BRAID,STEEL as METAL,CERAMIC as SHELL,CARGO as POD};
/// Half-width of the dwell window around a port cusp, as a fraction of a tether turn.
const DWELL_TURNS: f32 = 0.08;

fn capsule(mesh: &mut Model, at: Vec3, size: f32, axis: Vec3) {
    let mark=mesh.mark();
    super::pressure::hull(mesh,at,&[
        (-0.60*size,0.0),(-0.56*size,0.22*size),(-0.43*size,0.40*size),
        (0.43*size,0.40*size),(0.56*size,0.22*size),(0.60*size,0.0)],32,POD,0.0);
    for end in [-1.0,1.0] {
        let centre=at+Vec3::X*end*size*0.35;
        super::pressure::hull(mesh,centre,&[
            (-0.02*size,0.0),(-0.02*size,0.415*size),
            (0.02*size,0.415*size),(0.02*size,0.0)],32,METAL,0.5);
        part(mesh,at+Vec3::X*end*size*0.595,
            Vec3::new(size*0.025,size*0.20,size*0.20),Quat::IDENTITY,METAL,0.5);
    }
    // Separate sealed service hatch, hinges and recessed status strip.
    part(mesh,at+Vec3::Z*size*0.397,Vec3::new(size*0.36,size*0.22,size*0.025),
        Quat::IDENTITY,SHELL,0.0);
    for x in [-0.13,0.13] {
        part(mesh,at+Vec3::new(x*size,0.0,size*0.415),
            Vec3::new(size*0.035,size*0.20,size*0.025),Quat::IDENTITY,METAL,0.5);
    }
    mesh.rotate_since(mark,at,Quat::from_rotation_arc(Vec3::X,axis));
}

/// Four curled jaws on a spreader housing, open around the capture point.
/// Local +Y runs along the tether, outward; `align` maps it into the chart.
fn grapple_claw(mesh: &mut Model, tip: Vec3, align: Quat, g: f32) {
    let at=|r:f32,theta:f32,h:f32| tip+align*Vec3::new(r*theta.cos(),h,r*theta.sin())*g;
    strut(mesh,at(0.0,0.0,-0.75),at(0.0,0.0,-0.3),0.2*g,METAL);
    strut(mesh,at(0.0,0.0,-0.8),at(0.0,0.0,-0.68),0.3*g,SHELL);
    strut(mesh,at(0.0,0.0,-0.36),at(0.0,0.0,-0.28),0.26*g,SHELL);
    for k in 0..4 {
        let theta=k as f32*std::f32::consts::FRAC_PI_2+std::f32::consts::FRAC_PI_4;
        let (root,knee,heel,hook)=(at(0.2,theta,-0.32),at(0.48,theta,-0.02),at(0.4,theta,0.3),at(0.16,theta,0.4));
        strut(mesh,root,knee,0.06*g,SHELL);
        strut(mesh,knee,heel,0.055*g,SHELL);
        strut(mesh,heel,hook,0.045*g,METAL);
        part(mesh,knee,Vec3::splat(0.11*g),align,METAL,0.5);
    }
}

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
    if !skyhook::supported(world.sample_config().world_type) { return; }
    let sun = super::observer_sun(day,viewer);
    let centre = Vec3::new(viewer.x, -(PLANET_RADIUS_M as f32), viewer.z);
    for c in skyhook::physical_contacts(world.seed,day,viewer,&world.orbital.dynamics) {
        let h = c.hook;
        let mark = mesh.mark();
        let axis = (c.tip - c.hub).normalize();
        let (truss, grapple) = hardware(h.size);
        // Hardware retains its physical size. Only cable raster support widens;
        // pixel floors here made distant pods larger than their host station.
        let braid = braid_width(h.size);
        // Counter-arm: a short stub past the hub carries the ballast mass.
        let ballast = c.hub - axis * (h.tether_m as f32 * 0.12);
        // Reference cargo ratings, not current inventory. Approximate the braid
        // as a bundled fibre strap (1600 kg/m3; section width squared / 1000).
        // Equal-stress section diameter varies with sqrt(tension), rather than
        // the previous arbitrary linear widening. Size for peak load, not phase.
        let payload = match h.size {Size::Courier=>20_000.0,Size::Talon=>500_000.0,Size::Ark=>10_000_000.0};
        let line_mass = 1600.0*(braid as f64).powi(2)/1000.0;
        let tip_load = h.design_tension_n(1.0,payload,line_mass).max(1.0);
        let width_at=|s:f64| braid*(h.design_tension_n(s,payload,line_mass)/tip_load).sqrt() as f32;
        let last=(c.line.len()-1) as f64;
        // Spans wide enough on screen to read as a flat card become solid after
        // the ribbon normal override below, so they keep their own shading.
        let mut solid=Vec::new();
        for i in 0..c.line.len()-1 {
            let (lo,hi)=(c.line[i],c.line[i+1]);
            let width=width_at((i as f64+0.5)/last);
            let near=(width/(pixel*super::BRAID_SOLID_PX)).min(super::BRAID_NEAR_M);
            let Some((s0,s1))=super::near_span(lo,hi,viewer,near) else {
                super::profiled_tether(mesh,lo,hi,viewer,pixel,BRAID,|along| width_at((i as f64+along as f64)/last),1);
                continue;
            };
            let (n0,n1)=(lo.lerp(hi,s0),lo.lerp(hi,s1));
            if s0>0.0 { super::profiled_tether(mesh,lo,n0,viewer,pixel,BRAID,|_|width,1); }
            solid.push((n0,n1,width));
            if s1<1.0 { super::profiled_tether(mesh,n1,hi,viewer,pixel,BRAID,|_|width,1); }
        }
        tether(mesh, ballast, c.hub, viewer, pixel, BRAID, braid);
        // A round braid always shows its sunward side. The flat ribbon's own
        // normal can face away and turn a sunlit cable black at twilight.
        let lit = (sun - axis * sun.dot(axis)).try_normalize()
            .unwrap_or_else(|| axis.any_orthonormal_vector());
        for v in &mut mesh.vertices[mark..] { v.normal = lit.to_array(); }
        for (a,b,width) in solid { super::solid_braid(mesh,a,b,width,BRAID); }
        let align = Quat::from_rotation_arc(Vec3::Y, axis);
        // Build hardware at its own attachment point. Rotating the entire set
        // about the hub also rotated the already-posed tip and ballast away.
        let station = mesh.mark();
        for side in [-1.0, 1.0] {
            let offset = Vec3::X * side * truss * 0.09;
            strut(mesh, c.hub+offset-Vec3::Y*truss*0.5,
                c.hub+offset+Vec3::Y*truss*0.5, truss*0.012, METAL);
            for bay in 0..6 {
                let y = (bay as f32/6.0-0.5)*truss;
                strut(mesh, c.hub+offset+Vec3::Y*y,
                    c.hub-offset+Vec3::Y*(y+truss/6.0), truss*0.006, SHELL);
            }
            part(mesh, c.hub+Vec3::new(side*truss*0.28,0.0,0.0),
                Vec3::new(truss*0.30,truss*0.36,truss*0.008), Quat::IDENTITY, METAL, 1.0);
        }
        let bearing=mesh.mark();
        super::pressure::hull(mesh,c.hub,&[
            (-0.12*truss,0.0),(-0.10*truss,0.04*truss),(-0.07*truss,0.06*truss),
            (0.07*truss,0.06*truss),(0.10*truss,0.04*truss),(0.12*truss,0.0)],32,SHELL,0.5);
        mesh.rotate_since(bearing,c.hub,Quat::from_rotation_arc(Vec3::X,Vec3::Y));
        mesh.rotate_since(station, c.hub, align);
        part(mesh, ballast, Vec3::splat(truss*0.3), align, METAL, 0.0);
        grapple_claw(mesh,c.tip,align,grapple);
        // Ark second stage: the drop line pays out around the port cusp only.
        if h.size.drop_line_m() > 0.0 {
            let to_next = c.next_port_cusp_s;
            let since = h.pass_period() as f32 - to_next;
            let dt = if to_next < since { -to_next } else { since };
            let dwell = h.turn_period() as f32 * DWELL_TURNS;
            if dt.abs() < dwell {
                let reach = (std::f32::consts::FRAC_PI_2 * (1.0 - dt.abs() / dwell)).sin();
                let drop = c.tip + axis * h.size.drop_line_m() as f32 * reach;
                tether(mesh, c.tip, drop, viewer, pixel, BRAID, braid * 0.5);
                part(mesh, drop, Vec3::splat(grapple * 0.6), Quat::IDENTITY, METAL, 0.5);
            }
        }
        // Only a real, saved freight manifest produces a loaded capsule. Its
        // trajectory follows the same elastic line as the grapple and ledger.
        if let Some(at)=world.orbital.dynamics.cargo(&c,||port_on_ground(world,h.port_lon),skyhook::clock(day),viewer) {
            let pod_size = grapple * 0.6;
            capsule(mesh,at,pod_size,world.orbital.dynamics.cargo_axis(&c,skyhook::clock(day)));
        }
        long_range_since(mesh, mark);
        for v in &mut mesh.vertices[mark..] {
            v.ao_strength = 4.0 + sunlight(Vec3::from_array(v.pos) * mesh.scale, centre, sun);
        }
    }
}
