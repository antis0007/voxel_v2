use engine_content::BlockId as B;
use engine_worldgen::civilization::{
    Burial, CHRONICLE_GENERATOR, History, Livelihood, chronicle::*,
};
const SITE_ID: u32 = (1 << 28) + (61 << 11) + (69 << 4);
fn record(seed: u32, risk: u32, linked: bool) -> Chronicle {
    let sites = std::array::from_fn(|i| {
        Some(SiteRecord {
            id: SITE_ID + i as u32 * 8,
            center_mm: [i as i32 * 264000, 35000, 0],
            shelter: 3,
            burial_candidate: if i == 0 {
                Burial::Alluvium
            } else {
                Burial::Bedrock
            },
            landscape: Landscape {
                top_mm: 34000,
                slope_ppm: 10000,
                moisture_ppm: 600000,
                sediment_ppm: 700000,
                flood_risk_ppm: if i == 0 { risk } else { 0 },
                scree_mm: 0,
                bedrock: B::Limestone,
            },
            history: History {
                livelihood: if i == 0 {
                    Livelihood::Floodplain
                } else {
                    Livelihood::Pasture
                },
                founded: 20,
                peak_year: 20,
                peak_households: 0,
                households: 0,
                abandoned: None,
                flood_year: None,
                rockfall_year: None,
                refugees_received: 0,
                burial: Burial::Bedrock,
                rock: B::Limestone,
                cap_rock: B::Limestone,
                neighbor: None,
            },
        })
    });
    Chronicle {
        provenance: Provenance {
            seed,
            generator: CHRONICLE_GENERATOR,
            planet_recipe: 0,
            landscape_checksum: 123,
            model_revision: MODEL_REVISION,
            region: [-3, 5],
            deep_time_myr: 240,
            end_year: YEARS,
            step_years: STEP_YEARS,
        },
        linked,
        sites,
        events: Vec::new(),
    }
    .replay()
    .unwrap()
}
#[test]
fn chronicle_conserves_food_and_households_across_64_seeds() {
    let mut transfers = 0;
    let mut departures = 0;
    for seed in 0..64 {
        let c = record(seed, 1_000_000, true);
        assert!(c.events.len() <= MAX_EVENTS);
        for e in &c.events {
            let household_balance =
                e.households_before + u32::from(e.founded) * 2 + e.growth + e.migrants_in
                    - e.migrants_out
                    - e.shortage_departures;
            assert_eq!(household_balance, e.households_after);
            let food_balance = e.stores_before + i32::from(e.founded) * 8 + e.harvest - e.demand
                + e.aid_received
                - e.aid_sent
                - e.surplus_discarded
                + e.unmet_demand
                - e.growth as i32 * 8;
            assert_eq!(food_balance, e.stores_after, "seed={seed}, event={e:?}");
            assert!((0..=30).contains(&e.stores_after));
            assert!(e.households_after <= 6);
            if e.shortage_departures > 0 {
                assert!(e.unmet_demand > 0);
            }
            if e.aid_received > 0 || e.migrants_out > 0 {
                let peer = c
                    .events
                    .iter()
                    .find(|p| Some(p.site) == e.peer && p.year == e.year)
                    .unwrap();
                assert_eq!(e.aid_received, peer.aid_sent);
                assert_eq!(e.migrants_out, peer.migrants_in);
                transfers += 1;
            }
            departures += e.shortage_departures;
        }
        for s in c.sites.iter().flatten() {
            let final_event = c.events.iter().rev().find(|e| e.site == s.id).unwrap();
            assert_eq!(s.history.households, final_event.households_after);
            if let Some(year) = s.history.abandoned {
                assert_eq!(year, final_event.year);
                assert_eq!(final_event.households_after, 0);
            }
        }
    }
    assert!(transfers > 0);
    assert!(departures > 0);
}

#[test]
fn chronicle_geography_counterfactual_changes_real_households_and_ruins() {
    let mut changed = 0;
    for seed in 0..64 {
        let exposed = record(seed, 1_000_000, true);
        let sheltered = record(seed, 0, true);
        assert!(sheltered.events.iter().all(|e| e.cause != Cause::Flood));
        let a = exposed.sites[0].unwrap().history;
        let b = sheltered.sites[0].unwrap().history;
        changed += usize::from(
            a.households != b.households
                || a.peak_households != b.peak_households
                || a.abandoned != b.abandoned
                || a.burial != b.burial,
        );
        assert_eq!(
            a.flood_year,
            exposed
                .events
                .iter()
                .rev()
                .find(|e| e.site == exposed.sites[0].unwrap().id && e.cause == Cause::Flood)
                .map(|e| e.year)
        );
    }
    assert!(changed > 0);
}

#[test]
fn chronicle_is_repeatable_and_disconnected_regions_do_not_transfer() {
    for seed in [0, 16, 74, 731, u32::MAX] {
        let a = record(seed, 800_000, false);
        assert_eq!(a.replay().unwrap(), a);
        assert_eq!(a, record(seed, 800_000, false));
        assert_eq!(a.fingerprint(), record(seed, 800_000, false).fingerprint());
        assert!(a.events.iter().all(|e| e.aid_received == 0
            && e.aid_sent == 0
            && e.migrants_in == 0
            && e.migrants_out == 0
            && e.peer.is_none()));
        let mut changed = a.clone();
        changed.provenance.model_revision += 1;
        assert!(changed.replay().is_err());
        assert_ne!(a.fingerprint(), changed.fingerprint());
    }
}

#[test]
fn chronicle_archive_uses_recorded_evidence_and_no_unsupported_burial() {
    for risk in [0, 1_000_000] {
        for seed in 0..32 {
            let c = record(seed, risk, true);
            let id = SITE_ID;
            let text = c.archive_text(id).unwrap();
            assert!(text.contains("1000"));
            if let Some(e) = c
                .events
                .iter()
                .find(|e| e.site == id && e.cause == Cause::Flood)
            {
                assert!(text.contains(&format!("First recorded flood: year {}", e.year)));
            } else {
                assert!(!text.contains("sealed after"));
            }
            assert_eq!(c.archive_text(id + 123), None);
        }
    }
}

#[test]
fn chronicle_cost_is_bounded_and_does_not_allocate_per_voxel() {
    let start = std::time::Instant::now();
    let mut checksum = 0;
    let mut bytes = 0;
    for seed in 0..1000 {
        let c = record(seed, 700_000, true);
        assert!(c.events.capacity() <= MAX_EVENTS);
        bytes = bytes.max(c.events.capacity() * std::mem::size_of::<Event>());
        checksum ^= c.fingerprint();
    }
    eprintln!(
        "chronicle 1000 regional replays: {:.3} ms, max event bytes={bytes}, digest={checksum:016x}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}
