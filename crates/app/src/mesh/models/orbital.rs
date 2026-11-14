//! Bounded world-space orbital silhouettes; no camera-facing cards or terrain
//! chunks are allocated for the kilometre-long suspension braid.
use super::Model;
use crate::world::{
    DayNightCycle, World,
    orbital::{self, Class},
};
use glam::{Quat, Vec3};
mod architecture;
mod carriage;
mod cabin;
mod security;
mod dish;
mod megastructures;
mod skyhook;
mod meteors;
mod pressure;
mod palette;
mod detail;
mod ground;
mod ground_monument;
mod terrain;
use architecture::{part, strut};

/// Minimum raster coverage of a distant corridor braid, from 0 (invisible) to 1.
const FAR_BRAID_COVERAGE: f32 = 0.2;
/// Ground socket on the service side of the passenger shaft.
const FREIGHT_SOCKET_Z_M: f32 = -38.0;

/// Project the origin-chart sun into the observer's tangent frame, matching
/// celestial shading. Keep solar tracking independent of the camera heading.
fn observer_sun(day: &DayNightCycle, viewer: Vec3) -> Vec3 {
    let radius = orbital::megastructures::PLANET_RADIUS_M;
    let lon = (viewer.x as f64/radius) as f32;
    let lat = (-viewer.z as f64/radius) as f32;
    let sun = day.lighting().sun_dir.normalize();
    let east = Vec3::new(lon.cos(),-lon.sin(),0.0);
    let up = Vec3::new(lon.sin()*lat.cos(),lon.cos()*lat.cos(),-lat.sin());
    let south = Vec3::new(lon.sin()*lat.sin(),lon.cos()*lat.sin(),lat.cos());
    Vec3::new(sun.dot(east),sun.dot(up),sun.dot(south)).normalize()
}

pub(super) fn build(
    mesh: &mut Model,
    world: &World,
    day: &DayNightCycle,
    viewer: Vec3,
    pixel_angle: f32,
) {
    // Matched terrain/lighting reviews can exclude independent scene traffic.
    // Restrict the override to explicit benchmark invocations so a leftover
    // environment variable cannot remove infrastructure from normal play.
    static REVIEW_DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *REVIEW_DISABLED.get_or_init(|| {
        std::env::var("VOXEL_REVIEW_NO_ORBITAL").as_deref() == Ok("1")
            && std::env::args().any(|arg| arg.starts_with("--benchmark-"))
    }) {
        return;
    }
    let cfg = world.sample_config();
    let mut trace = crate::gameplay_trace::Trace::new();
    ground::build(mesh,world,viewer);
    trace.mark("orbital-ground-details");
    receiver(mesh, world, day, viewer, pixel_angle);
    cabin::build(mesh, world, viewer);
    security::build(mesh, world, viewer);
    dish::build(mesh,world,day,viewer);
    trace.mark("orbital-stations");
    skyhook::build(mesh, world, day, viewer, pixel_angle);
    trace.mark("orbital-skyhooks");
    let contacts = orbital::visual_contacts(world.seed, day, &world.orbital, viewer);
    // Start accurate attachment surveys during the approach, including above
    // the drawing cutoff. Published heights use the same terrain functions as
    // collision/service logic; no guessed altitude or removed infrastructure.
    for contact in &contacts { terrain::get(cfg, contact.service.site); }
    // Stations, 18 km braids and 27 km streaks are chart-scale. Past the crowns
    // they dwarf the cube sphere, so the orbital view keeps the body only.
    if !orbital::draw_chart_hardware(world.world_type == crate::world::WorldType::Planet, viewer.y) {
        return;
    }
    megastructures::build(mesh,world,day,viewer);
    meteors::build(mesh, world, day, viewer, pixel_angle);
    for contact in contacts {
        let service = contact.service;
        // Between visits the carriage remains high on the same persistent braid.
        let site = service.site;
        let Some(heights) = terrain::get(cfg, site) else { continue; };
        let ground = heights.hold;
        let crown = orbital::crown_position(site, ground);
        let radius = site.class.upper_ring_radius_m();
        // Allow for the carriage's 24 m swing and its overhanging hull. The
        // lowest keel plate sits 0.673 radii below the ring, above the habitat.
        // Socket, carriage and crown share the same freight guide axis.
        // Class-dependent offsets used to pull the cable away from its shaft.
        let terminal_site = site.cell == glam::IVec2::ZERO
            && engine_worldgen::meridian_terminal::enabled(&cfg);
        let monumental = terminal_site
            && cfg.generator_version >= engine_worldgen::meridian_terminal::monument::VERSION;
        // The monument carries its braid on the tower axis and seats it in the
        // crown cage. Beside the tower it landed on a post between two buttresses.
        let freight_offset = if monumental { Vec3::ZERO } else { Vec3::Z * FREIGHT_SOCKET_Z_M };
        let top = crown + freight_offset + Vec3::Y * (16.0 + 0.70 * radius);
        let foot = Vec3::new(site.x, ground, site.z) + freight_offset;
        let socket_site = orbital::Site { z: site.z + FREIGHT_SOCKET_Z_M, ..site };
        let footing = Vec3::new(socket_site.x,
            heights.footing, socket_site.z);
        let sea_surface = cfg.world_y(cfg.water_level() + 1);
        let socket_height = if terminal_site {
            // Connect flooded terminals at the first usable height on the tower.
            (sea_surface - footing.y + 4.0).clamp(9.0, if monumental { 120.0 } else { 40.0 })
        } else {
            9.0
        };
        let crown_seat = Vec3::new(site.x, footing.y
            + engine_worldgen::meridian_terminal::monument::SHAFT_HEIGHT - 6.0, site.z);
        let socket = if monumental { crown_seat } else { footing + Vec3::Y * socket_height };
        // The carriage docks above the crown; it never descends through the tower.
        let foot = if monumental { crown_seat + Vec3::Y * (14.0 + site.class.width()) } else { foot };
        let mark = mesh.mark();
        let signal = if service.requested {
            [0.41, 0.83, 0.76]
        } else {
            [0.91, 0.60, 0.26]
        };
        let p = service.carriage_position(foot, crown + freight_offset);
        let joint = p + Vec3::Y * site.class.width() * 0.71;
        let suspension_axis = (top - joint).normalize();
        // A reeled suspension line terminates at the moving joint. A fixed
        // ground cable would visibly detach from a carriage on this wide arc.
        let terminal = top - suspension_axis * radius * 0.65;
        let upper_width=if monumental {4.8}else{1.2};
        let lower_width=if monumental {4.8}else{2.0};
        let mut bow = world.orbital.dynamics.elevator_bow();
        // Limit crosswind deflection toward the parallel passenger guide.
        bow.z = bow.z.min(6.0);
        let far=contact.distance_m>orbital::SIGHT_RANGE_M;
        if terminal_site {
            // The freight braid runs behind the passenger shaft. Tie its
            // socket into the terminal instead of leaving a separate plinth.
            let base = Vec3::new(site.x, footing.y, site.z);
            ground::distant_landmark(mesh, base, viewer, monumental);
            for side in if monumental { [0.0; 0].as_slice() } else { [-1.0, 1.0].as_slice() }.iter().copied() {
                let anchor = if monumental {
                    // Follow the curved buttresses facing the rear of the tower.
                    let radius = 48.0 - 37.0 * (socket_height / 136.0).powf(0.62);
                    Vec3::new(side * radius * 0.5, socket_height, -radius * 0.8660254)
                } else {
                    let height = if side > 0.0 { socket_height.min(30.0) } else { socket_height };
                    Vec3::new(side * 36.0, height, -34.5)
                };
                strut(mesh, base + anchor, socket, 1.2, palette::STEEL);
            }
            if !monumental { strut(mesh, footing - Vec3::Y, socket, 1.2, palette::STEEL); }
        }
        // Near and distant corridors share one braid path, widths and sag. Two
        // separate curves met at the joint at different angles and left a visible gap.
        if far {
            // Beyond sight range every class is its braid and carriage. The span to
            // the crown stands in for the anchorage; a Conservatory keeps its crown.
            if terminal_site || matches!(site.class, Class::Conservatory) {
                architecture::anchorage(mesh, top, suspension_axis, site.class, site.key, day);
            }
            suspension(mesh,joint,terminal,viewer,pixel_angle,palette::BRAID,upper_width,bow);
            suspension(mesh,socket,joint,viewer,pixel_angle,palette::BRAID,lower_width,bow);
            suspension(mesh,terminal,top,viewer,pixel_angle,palette::BRAID,upper_width,bow);
            // A 1.2 m braid covers about an eighth of its 2 px support at 6 km and
            // blends into sky. Floor its coverage so the hairline still reads.
            for vertex in &mut mesh.vertices[mark..] {
                if vertex.surface[2] == 61.0 { vertex.surface[3] = vertex.surface[3].max(FAR_BRAID_COVERAGE); }
            }
            carriage::build(mesh, p, suspension_axis, service, day, signal,
                site.class.width() / ((p - viewer).length().max(1.0) * pixel_angle), true);
            megastructures::long_range_since(mesh, mark);
            continue;
        }
        architecture::anchorage(mesh, top, suspension_axis, site.class, site.key, day);
        if !terminal_site && matches!(site.class,Class::Conservatory) {
            megastructures::long_range_since(mesh,mark);
        }
        suspension(mesh,joint,terminal,viewer,pixel_angle,palette::BRAID,upper_width,bow);
        if terminal_site {
            suspension(mesh,terminal,top,viewer,pixel_angle,palette::BRAID,upper_width,bow);
        }
        // Lower braid: from the carriage joint down to the plinth on the ground.
        suspension(mesh,socket,joint,viewer,pixel_angle,palette::BRAID,lower_width,bow);
        if !terminal_site {
            for (root_index, side) in [-1.0, 1.0].into_iter().enumerate() {
                let root_site = orbital::Site { x: socket_site.x + side * 6.0, ..socket_site };
                let root_height = heights.roots[root_index];
                let root = Vec3::new(root_site.x, root_height, root_site.z);
                strut(mesh, root, socket, 1.2, palette::STEEL);
                // A flooded anchor is a pile. Exposed ceramic caps on the
                // waterline look like detached blocks.
                if root_height > sea_surface {
                    part(mesh, root, Vec3::new(5.0, 3.0, 8.0), Quat::IDENTITY, palette::CERAMIC, 0.0);
                }
            }
        }
        carriage::build(mesh, p, suspension_axis, service, day, signal,
                site.class.width() / ((p - viewer).length().max(1.0) * pixel_angle), false);
        if terminal_site {
            // Keep the tower, socket and braid visible across the service-range
            // boundary where ordinary corridor meshes fade out.
            megastructures::long_range_since(mesh, mark);
            continue;
        }
        // Family 62 owns local UVs; older pieces reserve surface.y for coverage.
        // The world shader
        // still applies atmosphere, weather and terrain occlusion, but excludes
        // these objects from the nearby terrain streaming-radius fade.
        for vertex in &mut mesh.vertices[mark..] {
            if vertex.surface[2] != 62.0 && vertex.surface[2] != 63.0 && vertex.surface[1] != 3.0 {
                vertex.surface[1] = 2.0;
            }
        }
    }
}

/// A subpixel cable needs filtered coverage, otherwise rasterization produces
/// a dotted line. The centreline stays world-anchored; only its raster support
/// expands to two pixels. Coverage restores its physical 1.2 m width in HDR.
fn tether(
    mesh: &mut Model,
    a: Vec3,
    b: Vec3,
    viewer: Vec3,
    pixel: f32,
    color: [f32; 3],
    width: f32,
) {
    profiled_tether(mesh,a,b,viewer,pixel,color,|_| width,24);
}

fn suspension(mesh:&mut Model,a:Vec3,b:Vec3,viewer:Vec3,pixel:f32,color:[f32;3],width:f32,bow:Vec3) {
    let axis=(b-a).normalize();
    let bow=(bow-axis*bow.dot(axis))*((b-a).length()/18_000.0).powi(2);
    let point=|s:f32|a.lerp(b,s)+bow*(std::f32::consts::PI*s).sin();
    for i in 0..16 {
        let (lo,hi)=(point(i as f32/16.0),point((i+1) as f32/16.0));
        // A ribbon facing the camera reads as a flat card once it spans several
        // pixels; inside BRAID_NEAR_M the span is solid, lit strand geometry.
        let near=(width/(pixel*BRAID_SOLID_PX)).min(BRAID_NEAR_M);
        let Some((s0,s1))=near_span(lo,hi,viewer,near) else {
            profiled_tether(mesh,lo,hi,viewer,pixel,color,|_|width,1);
            continue;
        };
        let (n0,n1)=(lo.lerp(hi,s0),lo.lerp(hi,s1));
        if s0>0.0 { profiled_tether(mesh,lo,n0,viewer,pixel,color,|_|width,1); }
        solid_braid(mesh,n0,n1,width,color);
        if s1<1.0 { profiled_tether(mesh,n1,hi,viewer,pixel,color,|_|width,1); }
    }
}

/// Braid width in pixels at which the ribbon becomes solid geometry. Below it
/// the 2 px ribbon and the solid braid are indistinguishable.
const BRAID_SOLID_PX: f32 = 3.0;
/// Upper bound on the solid radius, which caps vertex count at narrow FOVs.
const BRAID_NEAR_M: f32 = 1500.0;
/// Parameter range of segment `a..b` lying within `radius` of `viewer`.
fn near_span(a:Vec3,b:Vec3,viewer:Vec3,radius:f32)->Option<(f32,f32)> {
    let d=b-a;
    let len2=d.length_squared();
    if len2<1e-6 { return None; }
    let t=(viewer-a).dot(d)/len2;
    let miss2=(a+d*t-viewer).length_squared();
    if miss2>=radius*radius { return None; }
    let half=(radius*radius-miss2).sqrt()/len2.sqrt();
    let (s0,s1)=((t-half).max(0.0),(t+half).min(1.0));
    (s1>s0).then_some((s0,s1))
}

/// Three helical strands around a core, banded by collars. Strand phase is
/// anchored to absolute arc length so pieces split at different viewer
/// distances still line up.
fn solid_braid(mesh:&mut Model,a:Vec3,b:Vec3,width:f32,color:[f32;3]) {
    let axis=(b-a).normalize_or_zero();
    if axis.length_squared()<0.5 { return; }
    let u=axis.cross(if axis.y.abs()<0.9 {Vec3::Y} else {Vec3::X}).normalize();
    let v=axis.cross(u);
    let pitch=width*12.0;
    let collar=width*20.0;
    let base=a.dot(axis);
    let end=b.dot(axis);
    let at=|l:f32|a+axis*(l-base);
    let strand=|l:f32,k:usize|{
        let phase=l/pitch*std::f32::consts::TAU+k as f32*std::f32::consts::TAU/3.0;
        at(l)+(u*phase.cos()+v*phase.sin())*width*0.3
    };
    let core=[color[0]*0.7,color[1]*0.7,color[2]*0.7];
    strut(mesh,a,b,width*0.28,core);
    let mut l=base;
    while l<end {
        let n=(l+width*2.5).min(end);
        for k in 0..3 { strut(mesh,strand(l,k),strand(n,k),width*0.2,color); }
        let mark=(n/collar).floor()*collar;
        if mark>l && mark<=n {
            strut(mesh,at(mark)-axis*width*0.15,at(mark)+axis*width*0.15,width*0.56,palette::STEEL);
        }
        l=n;
    }
}

/// A load-bearing braid thickens toward the hub. Width remains physical;
/// the raster support and its coverage are evaluated independently per end.
fn profiled_tether(mesh: &mut Model, a: Vec3, b: Vec3, viewer: Vec3, pixel: f32,
    color: [f32;3], width: impl Fn(f32)->f32,segments:usize) {
    let axis = (b - a).normalize();
    let side = axis.cross((a + b) * 0.5 - viewer).normalize_or_zero();
    if side.length_squared() < 0.5 {
        return;
    }
    let normal = axis.cross(side);
    mesh.set_surface_kind(61.0);
    for i in 0..segments {
        let lo = a.lerp(b, i as f32 / segments as f32);
        let hi = a.lerp(b, (i + 1) as f32 / segments as f32);
        let physical_lo = width(i as f32/segments as f32).max(0.001);
        let physical_hi = width((i+1) as f32/segments as f32).max(0.001);
        let wl = physical_lo.max((lo - viewer).length() * pixel * 2.0);
        let wh = physical_hi.max((hi - viewer).length() * pixel * 2.0);
        let mark = mesh.mark();
        mesh.triangle(
            [
                lo - side * wl * 0.5,
                hi - side * wh * 0.5,
                hi + side * wh * 0.5,
            ],
            normal,
            color,
            0.0,
        );
        mesh.triangle(
            [
                lo - side * wl * 0.5,
                hi + side * wh * 0.5,
                lo + side * wl * 0.5,
            ],
            normal,
            color,
            0.0,
        );
        for vertex in &mut mesh.vertices[mark..] {
            let pos = Vec3::from_array(vertex.pos) * mesh.scale;
            let t = ((pos - lo).dot(axis) / (hi - lo).length()).clamp(0.0, 1.0);
            vertex.surface[3] = (physical_lo / wl) * (1.0 - t) + (physical_hi / wh) * t;
        }
    }
    mesh.set_surface_kind(41.0);
}

fn receiver(mesh: &mut Model, world: &World, day: &DayNightCycle, viewer: Vec3, pixel: f32) {
    use orbital::receiver as station;
    let Some(p) = station::position(world) else {
        return;
    };
    if (p - viewer).length_squared() > 512.0 * 512.0 {
        return;
    }
    let a = world.orbital.receiver.anchor.unwrap();
    // Presentation must never synchronously read an unloaded region from disk.
    let present = world
        .chunks
        .get(&crate::world::chunk_coord_from_world_block(a))
        .is_some_and(|c| {
            c.blocks[crate::world::ChunkData::local_index(a.x, a.y, a.z)]
                == crate::world::BlockId::Workbench
        });
    if !present {
        return;
    }
    let mark = mesh.mark();
    let metal = [0.26, 0.31, 0.34];
    let ceramic = [0.76, 0.73, 0.63];
    // The actual workbench console and stone footing are editable world blocks.
    // These small fittings distinguish the registered station from a workbench.
    {
        mesh.cuboid(
            p + Vec3::Y * 0.54,
            Vec3::new(0.8, 0.12, 0.8),
            Quat::IDENTITY,
            metal,
            0.0,
        );
        mesh.cuboid(
            p + Vec3::new(0.0, 0.62, -0.25),
            Vec3::new(0.48, 0.08, 0.15),
            Quat::IDENTITY,
            if matches!(world.orbital.receiver.stage,2|4) {
                [0.4, 0.7, 0.5]
            } else {
                [0.3, 0.75, 0.8]
            },
            0.7,
        );
        mesh.cuboid(
            p + Vec3::new(0.65, 1.0, 0.6),
            Vec3::new(0.12, 2.1, 0.12),
            Quat::IDENTITY,
            metal,
            0.0,
        );
        mesh.cuboid(
            p + Vec3::new(0.65, 2.05, 0.6),
            Vec3::new(1.0, 0.12, 0.75),
            Quat::from_rotation_z(0.25),
            ceramic,
            0.0,
        );
    }
    if let Some((upper, pad, progress)) = station::capsule(world, day) {
        let t = progress * progress * (3.0 - 2.0 * progress);
        let sway = (std::f32::consts::PI * progress).sin().powi(2)
            * (progress * std::f32::consts::TAU * 4.0).sin() * (-2.0 * progress).exp() * 0.45;
        let at = upper.lerp(pad, t) + Vec3::X * sway;
        if progress < 1.0 {
            tether(mesh, upper, at + Vec3::Y * 0.65, viewer, pixel, metal, 0.08);
        }
        let cargo = mesh.mark();
        part(mesh, at, Vec3::new(1.2, 1.2, 1.2), Quat::IDENTITY, ceramic, 0.0);
        for x in [-0.38, 0.38] {
            part(mesh,
                at + Vec3::X * x,
                Vec3::new(0.08, 1.24, 1.24),
                Quat::IDENTITY,
                metal,
                0.5,
            );
        }
        mesh.cuboid(
            at + Vec3::new(0.0, 0.0, 0.61),
            Vec3::new(0.35, 0.12, 0.04),
            Quat::IDENTITY,
            [0.3, 0.75, 0.8],
            0.7,
        );
        mesh.rotate_since(cargo, at, Quat::from_rotation_y((1.0 - t) * 0.7)
            * Quat::from_rotation_z(sway * 0.08));
    }
    for vertex in &mut mesh.vertices[mark..] {
        if vertex.surface[2] != 62.0 { vertex.surface[1] = 2.0; }
    }
}
