use super::*;
const LAW: SoilLaw = SoilLaw {
    porosity_per_mille: 500,
    retention_per_mille: 600,
    infiltration_um_s: 1_000,
    drainage_um_s: 40,
    erosion_resistance_kj_kg: 10,
    density_kg_m3: 1500,
};

#[test]
fn environment_gravity_retains_water_that_plants_can_use_at_every_scale() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut soil = Soil { water: LAW.capacity(scale), nutrient: 1_000, ..Default::default() };
        let mut below = Soil::default();
        let initial = soil.water;
        for _ in 0..20_000 {
            drain(&mut soil, &mut below, LAW, LAW, scale, true);
        }
        let retained = LAW.retained_water(scale);
        assert!((retained..=retained + 3).contains(&soil.water));
        assert_eq!(soil.water + below.water, initial);
        let before = soil.water;
        let exchange = vegetation(
            &mut soil,
            PlantLaw {
                moisture: [100, 900],
                temperature_c: [0, 40],
                shade_tolerance: 500,
                growth_mg_m2_s: 100,
                water_ul_per_mg: 50,
                mature_mg_m2: 100_000,
            },
            LAW,
            scale,
            20,
            0,
            120,
        );
        assert!(exchange.transpired > 0);
        assert_eq!(before - soil.water, exchange.transpired);
        assert!(soil.water < retained);
    }
}

#[test]
fn environment_slow_soil_integrates_flux_in_seconds_at_every_scale() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let run = |seconds| {
            let mut a = Soil { water: LAW.capacity(scale), ..Default::default() };
            let mut b = Soil::default();
            for _ in 0..64 / seconds {
                drain_over(&mut a, &mut b, LAW, LAW, scale, scale, face_area_cm2(scale), true, seconds);
            }
            (a.water, b.water)
        };
        assert_eq!(run(1), run(8));
    }
}

#[test]
fn environment_exact_infiltration_has_no_packet_latency_and_scales_by_area() {
    let mut physical_total = None;
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut soil = Soil::default();
        let moved = infiltrate_exact(&mut soil, LAW, scale, cell_volume(scale));
        assert!(moved > 0);
        assert_eq!(soil.water, moved);
        let per_square_metre = moved * 10_000 / face_area_cm2(scale);
        assert_eq!(*physical_total.get_or_insert(per_square_metre), per_square_metre);
    }
}

#[test]
fn environment_small_cohorts_accumulate_growth_and_settling_conserves_grains() {
    let plant = PlantLaw {
        moisture: [0, 1_000],
        temperature_c: [0, 40],
        shade_tolerance: 700,
        growth_mg_m2_s: 3,
        water_ul_per_mg: 1,
        mature_mg_m2: 100_000,
    };
    let mut totals = Vec::new();
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut soil = Soil { water: LAW.capacity(scale) / 2, nutrient: 10_000, ..Default::default() };
        for _ in 0..30 {
            vegetation(&mut soil, plant, LAW, scale, 20, 0, 120);
        }
        assert!(soil.biomass > 0);
        totals.push(soil.biomass * 10_000 / face_area_cm2(scale));
    }
    assert!(totals.iter().all(|&n| n.abs_diff(totals[0]) <= 640));
    let mut grains = Sediment::from(1_000_000);
    for _ in 0..100 {
        grains.settle(1_000_000, 10_000);
        assert_eq!(grains.mass(), 1_000_000);
    }
    assert!(grains.bed > grains.suspended);
    let bed = grains.bed;
    assert_eq!(grains.entrain(200, 2), 200);
    assert_eq!(grains.bed, bed - 100);
    assert_eq!(grains.mass(), 1_000_000);
}

#[test]
fn environment_infiltration_preserves_source_and_units_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut soil = Soil::default();
        let mut water = cell_volume(scale);
        let total = water;
        for _ in 0..1000 {
            let moved = infiltrate(&mut soil, LAW, scale, water);
            water -= moved;
            assert_eq!(water + soil.water, total);
            assert!(soil.water <= LAW.capacity(scale));
        }
        assert_eq!(soil.water, total / 2);
    }
}

#[test]
fn environment_drainage_conserves_and_cannot_overfill_receiver() {
    let scale = VoxelScalePreset::QuarterMeter;
    let mut a = Soil { water: LAW.capacity(scale), ..Default::default() };
    let mut b = Soil::default();
    let total = a.water;
    for _ in 0..5000 {
        drain(&mut a, &mut b, LAW, LAW, scale, false);
        assert_eq!(a.water + b.water, total);
        assert!(a.water >= b.water);
    }
    assert!(b.water > total / 4);
}

#[test]
fn environment_transport_never_duplicates_or_loses_a_full_drain() {
    for mass in [1, 7, 8, 123_456_789] {
        let mut remaining = mass;
        let mut transported = 0;
        for source in (1..=8).rev() {
            let moved = advected_stock(remaining, source, 1);
            transported += moved;
            remaining -= moved;
        }
        assert_eq!(transported, mass);
        assert_eq!(remaining, 0);
    }
}

#[test]
fn environment_nutrients_and_biomass_account_for_growth_and_decay() {
    let scale = VoxelScalePreset::OneMeter;
    let mut soil = Soil { water: 300_000_000, nutrient: 10_000, ..Default::default() };
    let plant = PlantLaw {
        moisture: [100, 900],
        temperature_c: [0, 40],
        shade_tolerance: 500,
        growth_mg_m2_s: 10,
        water_ul_per_mg: 100,
        mature_mg_m2: 100_000,
    };
    let mut carbon = 0;
    let mut water = soil.water;
    for step in 0..1000 {
        let ex = vegetation(&mut soil, plant, LAW, scale, if step < 200 { 20 } else { -20 }, 0, 120);
        carbon += ex.atmospheric_biomass;
        carbon -= ex.decomposed;
        water -= ex.transpired;
        assert_eq!(soil.nutrient + soil.plant_nutrient + soil.litter_nutrient, 10_000);
        assert_eq!(soil.biomass + soil.litter + soil.nutrient, carbon + 10_000);
        assert_eq!(soil.water, water);
    }
}
