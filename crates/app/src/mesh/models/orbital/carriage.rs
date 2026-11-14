//! Suspended service vessel: exposed load paths, shielded cargo and folding fins.
use super::super::posed;
use super::{Model, Quat, Vec3, architecture::{part, strut}};
use crate::{mesh::Vertex3, world::{DayNightCycle, orbital::{Class, Service}}};
use std::ops::Range;

/// Screen width below which the vessel is drawn as its massing only.
const DETAIL_PIXELS: f32 = 8.0;

/// Static local geometry. The two wings are separate because their fold angle
/// follows the carriage's lowering; the collar follows only the braid frame.
struct CachedCarriage {
    body: Vec<Vertex3>,
    wings: [Vec<Vertex3>; 2],
    collar: Vec<Vertex3>,
}

impl CachedCarriage {
    fn new(local: Model, wing_ranges: [Range<usize>; 2], collar: Range<usize>) -> Self {
        let [left, right] = wing_ranges;
        let mut body = Vec::with_capacity(collar.start - (left.len() + right.len()));
        body.extend_from_slice(&local.vertices[..left.start]);
        body.extend_from_slice(&local.vertices[left.end..right.start]);
        body.extend_from_slice(&local.vertices[right.end..collar.start]);
        Self {
            body,
            wings: [local.vertices[left].to_vec(), local.vertices[right].to_vec()],
            collar: local.vertices[collar].to_vec(),
        }
    }
}

thread_local! {
    static CARRIAGE_CACHE: std::cell::RefCell<std::collections::HashMap<(u8, bool, [u32; 3]), std::rc::Rc<CachedCarriage>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn class_tag(class: Class) -> u8 {
    match class { Class::Courier => 0, Class::Freight => 1, Class::Conservatory => 2 }
}

fn cached(class: Class, detailed: bool, signal: [f32; 3]) -> std::rc::Rc<CachedCarriage> {
    let key = (class_tag(class), detailed, signal.map(f32::to_bits));
    CARRIAGE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 24 { cache.clear(); }
        cache.entry(key).or_insert_with(|| {
            let (local, wings, collar) = if detailed {
                detailed_local(class, signal)
            } else {
                massing_local(class, signal)
            };
            std::rc::Rc::new(CachedCarriage::new(local, wings, collar))
        }).clone()
    })
}

/// `pass` is 0 for the direct CPU path, 1 for a near GPU placement and 2 for
/// the long-range material family. Deferred vertices must bake the caller pass.
fn place(mesh: &mut Model, class: Class, detailed: bool, signal: [f32; 3],
    part_tag: u32, pass: u32, source: &[Vertex3], rotation: Quat, translation: Vec3) {
    let signal = signal.map(f32::to_bits);
    let asset = posed::key(&[0x4341_5252, u32::from(class_tag(class)), u32::from(detailed),
        part_tag, pass, signal[0], signal[1], signal[2]]);
    let range = posed::intern(asset, || {
        if pass == 0 { return source.to_vec(); }
        let mut baked = Model::with_vertices(1.0, source.to_vec());
        if pass == 2 {
            super::megastructures::long_range_since(&mut baked, 0);
        } else {
            for vertex in &mut baked.vertices {
                if vertex.surface[2] != 62.0 && vertex.surface[2] != 63.0 && vertex.surface[1] != 3.0 {
                    vertex.surface[1] = 2.0;
                }
            }
        }
        baked.vertices
    });
    posed::place(mesh, range, rotation, translation);
}

/// Sway, heading and bank of the vessel under the joint, and the line frame.
fn pose(axis: Vec3, service: Service, day: &DayNightCycle) -> (Quat, Quat, f32) {
    let t = service.lowering;
    let align = ((t-0.35)/0.61).clamp(0.0,1.0);
    let align = align.powi(3)*(align*(align*6.0-15.0)+10.0);
    let bank = (std::f32::consts::PI*t).sin().powi(2);
    let heading = (service.site.key>>8) as f32/16777216.0*std::f32::consts::TAU;
    let sense = if service.site.key&1==0 {1.0} else {-1.0};
    let line_frame=Quat::from_rotation_arc(Vec3::Y,axis);
    let rotation=line_frame*Quat::from_rotation_y(heading+sense*2.6*(1.0-align))
        *Quat::from_rotation_z(sense*bank*0.25+service.suspension_angle(day))
        *Quat::from_rotation_x(bank*0.12);
    (line_frame, rotation, align)
}

/// `pixels` is the vessel's width on screen. A few pixels cannot show the
/// canisters, ribs and feathers, which are ~24k vertices per vessel.
pub(super) fn build(mesh: &mut Model, p: Vec3, axis: Vec3, service: Service,
    day: &DayNightCycle, signal: [f32;3], pixels: f32, long_range: bool) {
    let class = service.site.class;
    let detailed = pixels >= DETAIL_PIXELS;
    let cached = cached(class, detailed, signal);
    let (line_frame, rotation, align) = pose(axis, service, day);
    let joint = Vec3::Y * class.width() * 0.71;
    let body_translation = p + joint - rotation * joint;
    let pass = if !posed::recording() { 0 } else if long_range { 2 } else { 1 };
    place(mesh, class, detailed, signal, 0, pass, &cached.body, rotation, body_translation);
    for (index, side) in [-1.0, 1.0].into_iter().enumerate() {
        let hinge = Vec3::new(side * class.width() * 0.43, class.width() * 0.025, class.width() * 0.06);
        let fold = Quat::from_rotation_z(side * (0.20 + (1.0 - align) * 0.95));
        place(mesh, class, detailed, signal, index as u32 + 1, pass, &cached.wings[index],
            rotation * fold, body_translation + rotation * (hinge - fold * hinge));
    }
    place(mesh, class, detailed, signal, 3, pass, &cached.collar,
        line_frame, p + joint - line_frame * joint);
}

/// Unit-scale geometry around the carriage centre, with unfolded wings and no
/// suspension/body rotation. Dynamic transforms are composed in `build`.
fn detailed_local(class: Class, signal: [f32; 3]) -> (Model, [Range<usize>; 2], Range<usize>) {
    let w = class.width();
    let p = Vec3::ZERO;
    let metal = super::palette::STEEL;
    let shell = super::palette::CERAMIC;
    let copper = super::palette::COPPER;
    let dark = super::palette::GRAPHITE;
    let mut local = Model::new(1.0);
    let mesh = &mut local;
    let mut wings = Vec::with_capacity(2);
    // Narrow central pressure hull; the cargo outriggers leave a clear void
    // beneath the suspension tower, rather than filling a rectangular slab.
    part(mesh,p,Vec3::new(w*0.22,w*0.15,w*0.83),Quat::IDENTITY,dark,0.5);
    for end in [-1.0,1.0] {
        part(mesh,p+Vec3::new(0.0,w*0.015,end*w*0.34),
            Vec3::new(w*0.19,w*0.14,w*0.23),Quat::from_rotation_x(end*0.16),shell,0.0);
    }
    // Raised, sloped bridge with a recessed panoramic observation band.
    part(mesh,p+Vec3::new(0.0,w*0.13,-w*0.18),
        Vec3::new(w*0.20,w*0.15,w*0.22),Quat::from_rotation_x(-0.18),shell,0.0);
    mesh.cuboid(p+Vec3::new(0.0,w*0.14,-w*0.295),
        Vec3::new(w*0.145,w*0.028,w*0.009),Quat::from_rotation_x(-0.18),[0.36,0.62,0.64],0.45);
    for side in [-1.0,1.0] {
        let cargo=p+Vec3::new(side*w*0.32,-w*0.09,w*0.02);
        // Three individual pressure canisters, with thermal shields and
        // exposed gaps between ribs. Lamps belong to the recessed service rail.
        for bay in -1..=1 {
            let c=cargo+Vec3::Z*bay as f32*w*0.19;
            super::detail::vessel(mesh,c,w*0.18,w*0.085,
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),copper);
            part(mesh,c+Vec3::Y*w*0.09,Vec3::new(w*0.22,w*0.025,w*0.17),
                Quat::from_rotation_z(-side*0.15),shell,0.0);
            for z in [-1.0,1.0] {
                part(mesh,c+Vec3::Z*z*w*0.08,Vec3::new(w*0.195,w*0.19,w*0.012),Quat::IDENTITY,metal,0.5);
            }
            mesh.cuboid(c+Vec3::new(side*w*0.099,0.0,0.0),
                Vec3::new(w*0.009,w*0.025,w*0.09),Quat::IDENTITY,[0.88,0.63,0.30],0.8);
        }
        // Two open triangular load frames carry the pods up to the gimbal.
        for z in [-0.29,0.29] {
            let low=p+Vec3::new(side*w*0.43,0.0,z*w);
            let high=p+Vec3::new(0.0,w*0.65,z*w*0.18);
            strut(mesh,low,high,w*0.013,metal);
            strut(mesh,low,p+Vec3::new(0.0,w*0.14,z*w),w*0.012,metal);
            strut(mesh,low.lerp(high,0.5),p+Vec3::new(0.0,w*0.14,z*w),w*0.007,copper);
        }
        // Long, separated radiator feathers; each has a real hinge and spine.
        let hinge=p+Vec3::new(side*w*0.43,w*0.025,w*0.06);
        let wing=mesh.mark();
        let count=if matches!(class,Class::Conservatory) {4} else {3};
        for feather in 0..count {
            let z=(feather as f32-(count-1) as f32*0.5)*w*0.135;
            let length=w*(0.49-0.055*(feather as f32-(count-1) as f32*0.5).abs());
            let c=hinge+Vec3::new(side*length*0.5,0.0,z);
            part(mesh,c,Vec3::new(length,w*0.012,w*0.105),Quat::IDENTITY,[0.065,0.12,0.18],1.0);
            strut(mesh,hinge+Vec3::Z*z,hinge+Vec3::new(side*length,0.0,z),w*0.0055,metal);
            for rib in 1..5 {
                part(mesh,hinge+Vec3::new(side*length*rib as f32/5.0,w*0.009,z),
                    Vec3::new(w*0.006,w*0.007,w*0.102),Quat::IDENTITY,[0.37,0.42,0.43],0.5);
            }
        }
        wings.push(wing..mesh.mark());
        mesh.cuboid(p+Vec3::new(side*w*0.43,w*0.025,-w*0.30),
            Vec3::splat(w*0.018),Quat::IDENTITY,signal,1.0);
    }
    // Split ceramic gimbal cheeks and a dark bearing, kept mechanically
    // separate from the fixed cable collar above the rotating vessel.
    let joint=p+Vec3::Y*w*0.71;
    for side in [-1.0,1.0] {
        part(mesh,joint+Vec3::X*side*w*0.055-Vec3::Y*w*0.05,
            Vec3::new(w*0.035,w*0.16,w*0.12),Quat::IDENTITY,shell,0.0);
    }
    strut(mesh,joint-Vec3::X*w*0.07,joint+Vec3::X*w*0.07,w*0.026,copper);
    let collar=mesh.mark();
    strut(mesh,joint,joint+Vec3::Y*w*0.16,w*0.025,metal);
    part(mesh,joint+Vec3::Y*w*0.10,Vec3::new(w*0.14,w*0.035,w*0.14),Quat::IDENTITY,shell,0.0);
    let collar = collar..mesh.mark();
    (local, wings.try_into().unwrap(), collar)
}

/// The distant outline uses the same local-frame and rigid-placement path.
fn massing_local(class: Class, signal: [f32; 3]) -> (Model, [Range<usize>; 2], Range<usize>) {
    let w = class.width();
    let p = Vec3::ZERO;
    let mut local = Model::new(1.0);
    let mesh = &mut local;
    let mut wings = Vec::with_capacity(2);
    part(mesh,p+Vec3::Y*w*0.04,Vec3::new(w*0.22,w*0.30,w*0.83),Quat::IDENTITY,super::palette::GRAPHITE,0.5);
    let feathers = if matches!(class,Class::Conservatory) {4.0} else {3.0};
    for side in [-1.0,1.0] {
        part(mesh,p+Vec3::new(side*w*0.32,-w*0.07,w*0.02),
            Vec3::new(w*0.22,w*0.21,w*0.62),Quat::IDENTITY,super::palette::CERAMIC,0.0);
        let hinge=p+Vec3::new(side*w*0.43,w*0.025,w*0.06);
        let wing=mesh.mark();
        part(mesh,hinge+Vec3::X*side*w*0.245,Vec3::new(w*0.49,w*0.012,w*0.135*feathers),
            Quat::IDENTITY,[0.065,0.12,0.18],1.0);
        wings.push(wing..mesh.mark());
        mesh.cuboid(p+Vec3::new(side*w*0.43,w*0.025,-w*0.30),
            Vec3::splat(w*0.018),Quat::IDENTITY,signal,1.0);
    }
    let joint=p+Vec3::Y*w*0.71;
    strut(mesh,p+Vec3::Y*w*0.14,joint,w*0.05,super::palette::STEEL);
    let collar=mesh.mark();
    strut(mesh,joint,joint+Vec3::Y*w*0.16,w*0.025,super::palette::STEEL);
    let collar = collar..mesh.mark();
    (local, wings.try_into().unwrap(), collar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::orbital::{Phase, Site};
    use glam::IVec2;

    fn bounds(mesh: &Model) -> (Vec3, Vec3) {
        mesh.vertices.iter().map(|v| Vec3::from_array(v.pos))
            .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)))
    }

    fn service(class: Class, key: u32, lowering: f32, requested: bool) -> Service {
        Service {
            site: Site { cell: IVec2::ZERO, key, class, x: 0.0, z: 0.0 },
            phase: Phase::Descending, lowering, minutes_to_change: 0,
            requested, handoff_day: 0.0,
        }
    }

    #[test]
    fn distant_massing_keeps_the_vessel_outline_at_a_fraction_of_the_cost() {
        let day = DayNightCycle::default();
        for class in [Class::Courier, Class::Freight, Class::Conservatory] {
            for (key, lowering) in [(7, 0.2), (0x1234_5601, 0.8)] {
                let service = service(class, key, lowering, false);
                let p = Vec3::new(100.0, 2000.0, -50.0);
                let axis = Vec3::new(0.2, 1.0, 0.1).normalize();
                let mut detail = Model::new(1.0);
                build(&mut detail, p, axis, service, &day, [1.0; 3], DETAIL_PIXELS, false);
                let mut massing = Model::new(1.0);
                build(&mut massing, p, axis, service, &day, [1.0; 3], DETAIL_PIXELS - 0.1, false);
                assert!(massing.vertices.len() * 20 < detail.vertices.len(),
                    "{class:?}: {} vs {}", massing.vertices.len(), detail.vertices.len());
                let (dl, dh) = bounds(&detail);
                let (ml, mh) = bounds(&massing);
                let w = class.width();
                // Within a fifth of the vessel's width on every side, so the
                // few pixels it covers do not jump at the switch.
                assert!((dl - ml).abs().max_element() < w * 0.2 && (dh - mh).abs().max_element() < w * 0.2,
                    "{class:?}: detail {dl}..{dh}, massing {ml}..{mh}");
            }
        }
    }

    fn apply_cpu_pass(mesh: &mut Model, long_range: bool) {
        if long_range {
            super::super::megastructures::long_range_since(mesh, 0);
        } else {
            for vertex in &mut mesh.vertices {
                if vertex.surface[2] != 62.0 && vertex.surface[2] != 63.0 && vertex.surface[1] != 3.0 {
                    vertex.surface[1] = 2.0;
                }
            }
        }
    }

    #[test]
    fn posed_carriages_match_cpu_and_reuse_resident_geometry() {
        let day = DayNightCycle::new(0.5, 600.0);
        let p = Vec3::new(100.0, 2000.0, -50.0);
        let axis = Vec3::new(0.2, 1.0, 0.1).normalize();
        let mut cases = 0;
        let mut pool_len = 0;
        for class in [Class::Courier, Class::Freight, Class::Conservatory] {
            for (key, lowering) in [(7, 0.2), (0x1234_5601, 0.8)] {
                for detailed in [false, true] {
                    for long_range in [false, true] {
                        for requested in [false, true] {
                            let service = service(class, key, lowering, requested);
                            let signal = if requested { [0.41, 0.83, 0.76] } else { [0.91, 0.60, 0.26] };
                            let pixels = if detailed { DETAIL_PIXELS } else { DETAIL_PIXELS - 0.1 };
                            posed::set_enabled(false);
                            let mut cpu = Model::new(0.25);
                            build(&mut cpu, p, axis, service, &day, signal, pixels, long_range);
                            apply_cpu_pass(&mut cpu, long_range);

                            posed::set_enabled(true);
                            posed::begin_frame();
                            let mut deferred = Model::new(0.25);
                            build(&mut deferred, p, axis, service, &day, signal, pixels, long_range);
                            posed::end_frame();
                            posed::set_enabled(false);
                            assert!(deferred.vertices.is_empty());
                            let (placed, placement_count, pool) = posed::with_frame(|placements, pool, _| {
                                let vertices: Vec<Vertex3> = placements.iter().flat_map(|placement| {
                                    pool[placement.first as usize..(placement.first + placement.count) as usize]
                                        .iter().map(move |v| posed::pose(v, placement.rotation,
                                            placement.translation, placement.scale))
                                }).collect();
                                (vertices, placements.len(), pool.len())
                            });
                            assert_eq!(placement_count, 4, "hull, two wings and collar");
                            pool_len = pool;
                            let mut a: Vec<[u32; 14]> = cpu.vertices.iter().map(|v| bytemuck::cast(*v)).collect();
                            let mut b: Vec<[u32; 14]> = placed.iter().map(|v| bytemuck::cast(*v)).collect();
                            a.sort_unstable();
                            b.sort_unstable();
                            assert_eq!(a, b, "{class:?} lowering={lowering} detailed={detailed} far={long_range} requested={requested}");
                            cases += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 48);

        // Repeating the final case adds only placements, not another asset copy.
        posed::set_enabled(true);
        posed::begin_frame();
        let mut deferred = Model::new(0.25);
        build(&mut deferred, p, axis, service(Class::Conservatory, 0x1234_5601, 0.8, true),
            &day, [0.41, 0.83, 0.76], DETAIL_PIXELS, true);
        posed::end_frame();
        posed::set_enabled(false);
        let repeated_pool = posed::with_frame(|placements, pool, _| {
            assert_eq!(placements.len(), 4);
            pool.len()
        });
        assert_eq!(repeated_pool, pool_len, "resident carriage geometry was rebuilt");
    }

}
