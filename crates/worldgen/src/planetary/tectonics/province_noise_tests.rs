//! Independent frozen equations from the pre-cache implementation.
//! Reference functions below must not call province_noise or cached helpers.
use super::*;
use crate::WorldType;
use engine_core::VoxelScalePreset;

fn config(seed: u32, generator_version: u32) -> WorldSampleConfig {
    WorldSampleConfig { seed, generator_version, world_type: WorldType::Planet,
        planet: None, voxel_scale: VoxelScalePreset::OneMeter }
}

fn province_bits(p: &Province) -> [u64; 9] {
    [p.height, p.collision, p.rift, p.carbonate, p.boundary_m, p.convergence,
        p.fault_damage, p.fault_displacement_m, p.trench].map(f64::to_bits)
}

#[test]
fn fixed_province_terms_match_original_equations_at_every_mesh_node() {
    for side in [4, 17, 84] {
        let mesh = crate::planetary::topology::Mesh::with_side(side);
        for seed in [0, 74, 731, u32::MAX] {
            let plates = Plates::new(&config(seed, 287));
            for (i, &dir) in mesh.directions.iter().enumerate() {
                let cached = plates.province_noise(dir);
                // Deliberately repeat the old formulas, including multiply then
                // divide for body-space noise; no production helper supplies expected.
                let n = |f, salt| noise_at(dir * f, seed.wrapping_add(salt));
                let body = dir * engine_core::cube_sphere::atlas::RADIUS_M;
                let expected = [
                    1.0 - (n(37.0, 827) * 2.0 - 1.0).abs(),
                    smooth(0.35, 0.66, n(18.0, 839)),
                    noise_at(body / 1450.0, seed.wrapping_add(853)) * 2.0 - 1.0,
                    1.0 - (noise_at(body / 850.0, seed.wrapping_add(857)) * 2.0 - 1.0).abs(),
                    smooth(0.45, 0.68, n(9.0, 829)),
                ];
                let actual = [cached.ridge, cached.country, cached.rounded, cached.spine, cached.carbonate];
                assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits),
                    "seed={seed}, side={side}, node={i}");
            }
        }
    }
}

#[test]
fn cached_and_uncached_provinces_match_frozen_reference_after_plate_mutations() {
    let mesh = crate::planetary::topology::Mesh::with_side(4);
    for seed in [74, 731] {
        for version in [270, 278, 282, 283, 285, 286, 287] {
            let mut plates = Plates::new(&config(seed, version));
            let cached: Vec<_> = mesh.directions.iter().map(|&d| plates.province_noise(d)).collect();
            let warped: Vec<_> = mesh.directions.iter().map(|&d| plates.warp(d)).collect();
            for phase in 0..5 {
                match phase {
                    1 => { let _ = plates.advance(0.006); }
                    2 => plates.split_strained(0, DVec3::X),
                    3 => {
                        let torque = vec![(DVec3::new(0.4, -0.2, 0.1), 1.0); plates.count()];
                        plates.relax_motion(&torque);
                    }
                    4 => {
                        let crust = vec![Crust { continental: 0.8, continental_km: 22.4, ..Default::default() }; mesh.directions.len()];
                        plates.evolve_continents(&mesh, &crust, &warped, 1.0);
                    }
                    _ => {}
                }
                for (i, &dir) in mesh.directions.iter().enumerate() {
                    let edge = plates.warped_edge(warped[i]);
                    for case in 0..3 {
                        let sample = |d: DVec3| {
                            let continental = (case as f64 * 0.45 + d.y * 0.2).clamp(0.0, 1.0);
                            Crust { continental, continental_km: continental * 28.0,
                                relief_m: d.x * 35.0 + phase as f64 * 4.0 - 20.0,
                                ocean_age_myr: 80.0 + 60.0 * d.z,
                                carbonate: 0.45 + 0.3 * d.x, slip_m: phase as f64 * 0.7,
                                ..Default::default() }
                        };
                        let expected = reference_evolved(&plates, dir, &edge, &sample);
                        let actual = plates.evolved_at_with_noise(dir, &edge, &cached[i], &sample);
                        let uncached = plates.evolved_at(dir, &edge, &sample);
                        assert_eq!(province_bits(&actual), province_bits(&expected),
                            "cached: seed={seed}, version={version}, phase={phase}, node={i}, case={case}");
                        assert_eq!(province_bits(&uncached), province_bits(&expected),
                            "uncached: seed={seed}, version={version}, phase={phase}, node={i}, case={case}");
                    }
                }
            }
        }
    }
}

    fn reference_evolved(plates: &Plates, dir: DVec3, edge: &Edge, sample: impl Fn(DVec3) -> Crust) -> Province {
        let crust = sample(dir);
        let (own, other) = if edge.boundary < 0.095 && edge.closing > 0.0 {
            // Measure crust on both sides of the moving margin, outside the trench.
            let reach = 0.065;
            (sample((dir - edge.normal * (reach - edge.boundary)).normalize()),
                sample((dir + edge.normal * (reach + edge.boundary)).normalize()))
        } else {
            (crust, crust)
        };
        let mut province = reference_province(plates,dir, edge, crust.continental, crust.relief_m,
            [own.continental, other.continental], edge.stresses());
        if crate::planetary::coupled_history(plates.cfg.generator_version) {
            province.carbonate = crust.carbonate;
        }
        if plates.faults && edge.boundary < 0.095 {
            let (displacement, damage) = plates.fault(dir, edge, own, other);
            province.height = (province.height + displacement).max(-58.0);
            province.fault_damage = damage;
            province.fault_displacement_m = displacement;
        }
        if plates.traverse && edge.boundary < 0.095 && edge.closing > 0.0 {
            province.height += plates.fold_belt(dir, edge, own.continental.min(other.continental));
        }
        if crate::planetary::geological_history(plates.cfg.generator_version)
            || plates.cfg.generator_version == crate::planetary::LAYERED_CANYON_GENERATOR {
            // The advected basement already includes past slip. Keep its
            // offset here so the rock contacts move with that same crust.
            province.fault_displacement_m += crust.slip_m;
            province.fault_damage = province.fault_damage.max(0.7 * smooth(0.6, 14.0, crust.slip_m.abs()));
        }
        province
    }

    fn reference_province(plates: &Plates, dir: DVec3, edge: &Edge, crust: f64, history_m: f64, sides: [f64; 2], stresses: [f64; 3]) -> Province {
        let n = |f, salt| noise_at(dir * f, plates.seed.wrapping_add(salt));
        let [collision, rift, _] = stresses;
        let Edge { boundary, closing, .. } = *edge;
        let coast = crust - 0.52 - plates.ocean;
        let coast = if plates.rifted_margins { coast } else {
            coast + (n(3.4 * plates.scale, 821) - 0.5) * 0.55
                + (n(11.0 * plates.scale, 823) - 0.5) * 0.13
        };
        let land = smooth(-0.025, 0.14, coast);
        let ridge = 1.0 - (n(37.0, 827) * 2.0 - 1.0).abs();
        let relief = plates.relief * (0.10 + collision * (0.55 + ridge * 0.45));
        // Regional uplift seeds hills and divides before water is routed.
        // A slower province mask leaves broad plains between hill countries;
        // erosion, not a post-process texture, connects their valleys.
        let body = dir * engine_core::cube_sphere::atlas::RADIUS_M;
        let country = smooth(0.35, 0.66, n(18.0, 839));
        let rounded = noise_at(body / 1450.0, plates.seed.wrapping_add(853)) * 2.0 - 1.0;
        let spine = 1.0 - (noise_at(body / 850.0, plates.seed.wrapping_add(857)) * 2.0 - 1.0).abs();
        let regional = land * country * plates.relief * 0.5
            * (rounded + 0.65 * (spine * spine - 0.55));
        let sea = f64::from(crate::WATER_LEVEL_METERS);
        let mut height = sea + coast * plates.relief * 0.75
            + land * relief - rift * plates.relief * 0.13 + regional;
        if history_m != 0.0 {
            height += history_m;
        }
        if plates.coastal {
            let margin = 1.0 - smooth(0.045, 0.29, coast.abs());
            if !plates.rifted_margins && margin > 0.0 {
                // Broad headlands and embayments share the same height field as drainage.
                let headlands = (n(31.0, 861) - 0.5) * 26.0 + (n(67.0, 863) - 0.5) * 8.0;
                let rift_cut = smooth(0.28, 0.62, rift) * smooth(0.38, 0.68, n(18.0, 877));
                let saddle = smooth(0.48, 0.73, n(21.0, 881)) * (1.0 - smooth(0.18, 0.62, rift));
                height += margin * (headlands - 30.0 * rift_cut + 17.0 * saddle);
            }
            // On mixed-crust boundaries, a broken ridge on the ocean side
            // can emerge as a chain of islands rather than isolated noise peaks.
            let [own, other] = sides;
            let mixed = (own - other).abs();
            let ocean_side = if own < other { boundary } else { -boundary };
            if !plates.rifted_margins && mixed > 0.0 && closing > 0.0 && ocean_side > 0.0 && ocean_side < 0.095 {
                let arc = (-((ocean_side - 0.045) / 0.018).powi(2)).exp();
                let summits = smooth(0.36, 0.77, n(45.0, 887));
                height += 72.0 * smooth(0.1, 0.7, closing) * arc * summits;
            }
            if plates.shape_shelf && !crate::planetary::changing_seas(plates.cfg.generator_version) && height < sea {
                let depth = sea - height;
                // Preserve the shoreline zero crossing and compress low shelves.
                let shaped = plates.shelf_depth(dir, depth);
                let trench = 12.0 * mixed * collision * smooth(6.0, 25.0, depth);
                height = sea - shaped - trench;
            }
        }
        Province { height: height.max(-58.0), collision, rift,
            trench: (sides[0] - sides[1]).abs() * collision,
            carbonate: smooth(0.45, 0.68, n(9.0, 829)) * (1.0 - collision.min(1.0)),
            boundary_m: boundary * engine_core::cube_sphere::atlas::RADIUS_M, convergence: closing,
            fault_damage: 0.0, fault_displacement_m: 0.0 }
    }
