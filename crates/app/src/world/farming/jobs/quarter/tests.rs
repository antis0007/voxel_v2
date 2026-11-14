use super::*;
use crate::{
    fluid::VoxelScalePreset,
    world::{
        WorldType,
        inventory::{Item, ItemStack},
    },
};
use std::collections::{BTreeMap, HashSet, VecDeque};

fn lot(complexity: u8) -> Lot {
    Lot {
        origin: [5, 5, 5],
        tradition: 0,
        complexity,
        turn: false,
        layer: 0,
        occupied: false,
        market_cover: false,
                frontage: false,
        roof_access: false,
        variant: 0,
    }
}

#[test]
fn city_quarter_traditions_are_real_materials_and_round_trip() {
    let rows = traditions().expect("architecture content");
    let encoded = ron::to_string(&rows).unwrap();
    let decoded: Vec<Tradition> = ron::from_str(&encoded).unwrap();
    assert_eq!(decoded.len(), 3);
    for row in rows {
        for b in [row.wall, row.frame, row.roof] {
            assert!(
                crate::world::inventory::Item::canonical_block(b).is_some(),
                "{b:?}"
            );
        }
    }
}

#[test]
fn city_quarter_all_styles_have_grounded_routes_to_interiors_and_upper_floors() {
    for style in traditions().unwrap() {
        for complexity in 1..=3 {
            let l = lot(complexity);
            let blocks: BTreeMap<_, _> = geometry::plan(&l, &style)
                .into_iter()
                .map(|(p, b)| ((p.x, p.y, p.z), b))
                .collect();
            let solid = |p: IVec3| p.y == -1 || blocks.contains_key(&(p.x, p.y, p.z));
            let stand = |p: IVec3| solid(p - IVec3::Y) && !solid(p) && !solid(p + IVec3::Y);
            let start = IVec3::new(4, 0, -3);
            let mut seen = HashSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some(p) = queue.pop_front() {
                for d in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
                    for dy in [0, 1, -1] {
                        let n = p + d + IVec3::Y * dy;
                        if n.x < -3 || n.x > 12 || n.z < -4 || n.z > 18 || n.y < 0 || n.y > 18 {
                            continue;
                        }
                        if stand(n) && seen.insert(n) {
                            queue.push_back(n);
                        }
                    }
                }
            }
            for floor in 0..complexity {
                let room = IVec3::new(2, i32::from(floor) * 4 + 1, 3);
                assert!(
                    seen.contains(&room),
                    "{} complexity={complexity} floor={floor}",
                    style.name
                );
            }
            for y in 0..=16 {
                assert!(
                    blocks.keys().filter(|(_, by, _)| *by == y).count()
                        <= super::super::construction::LIMIT
                );
            }
        }
    }
}

#[test]
fn city_quarter_roof_forms_and_complexities_change_geometry() {
    let rows = traditions().unwrap();
    let counts: Vec<_> = rows
        .iter()
        .map(|r| geometry::plan(&lot(2), r).len())
        .collect();
    assert_ne!(counts[0], counts[1]);
    assert_ne!(counts[1], counts[2]);
    assert!(geometry::plan(&lot(3), &rows[0]).len() > geometry::plan(&lot(1), &rows[0]).len());
    let court = geometry::plan(&lot(2), &rows[1]);
    assert!(!court.iter().any(|(p, _)| p.x == 4 && p.z == 7));
}

use super::super as jobs;
#[path = "authority_tests.rs"]
mod authority_tests;

fn assigned_world() -> (World, IVec3, jobs::construction::Order) {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let y = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 3;
    let floor: Vec<_> = (0..12).flat_map(|x| (0..12).flat_map(move |z| [
        (IVec3::new(x,y-1,z), B::Stone), (IVec3::new(x,y,z), B::Air),
        (IVec3::new(x,y+1,z), B::Air),
    ])).collect();
    w.set_many(&floor);
    let pile = IVec3::new(2,y,2);
    w.set(pile,B::Chest);
    let feet = Vec3::new(3.5,y as f32,3.5);
    crate::world::adventure::wildlife::vehicle::spawn_resident(&mut w,0,feet);
    jobs::assign(&mut w,0);
    assert!(jobs::designate(&mut w,pile));
    assert_eq!(jobs::crew::saved_workers(&w),vec![(0,feet)]);
    let order=jobs::construction::Order::new(IVec3::new(7,y,7),B::Planks);
    assert!(order.ready(&w));
    w.farming.jobs.build.push(order);
    (w,pile,order)
}

fn paid_cargo(w: &mut World, j: &mut jobs::State, pile: IVec3) {
    let item=Item::Block(B::Planks);
    let mut supplied=ItemStack::new(item,2);
    crate::world::block_data::store_hauled(w,pile,&mut supplied);
    assert_eq!(supplied.count,0);
    let count=crate::world::block_data::take_hauled(w,pile,item,2);
    assert_eq!(count,2);
    j.carry=Some(ItemStack::new(item,count));
    j.cargo=Some(jobs::supply::Cargo::Build {source:pile.to_array()});
    j.building_material=true;
    j.reconcile_claims();
    assert!(j.valid());
}

fn material_total(w: &World,j: &jobs::State,pile: IVec3,order: jobs::construction::Order)->u32 {
    let item=Item::Block(B::Planks);
    w.block_data.chest(pile).iter().flatten().filter(|s|s.item==item).map(|s|s.count).sum::<u32>()
        +j.carry.filter(|s|s.item==item).map_or(0,|s|s.count)
        +u32::from(w.get(order.cell())==B::Planks)
}

#[test]
fn city_quarter_project_waits_for_materials_and_paid_placement_conserves_stock() {
    let (mut w,pile,order)=assigned_world();
    let cell=order.cell();
    let mut j=std::mem::take(&mut w.farming.jobs);
    jobs::construction::fetch(&mut w,&mut j,pile,order);
    assert!(j.carry.is_none());
    jobs::construction::place(&mut w, &mut j, order, Vec3::splat(100.0));
    assert_eq!(w.get(cell), B::Air);
    assert_eq!(j.build.len(), 1);
    paid_cargo(&mut w,&mut j,pile);
    assert!(j.owns(order));
    assert_eq!(material_total(&w,&j,pile,order),2);
    jobs::construction::place(&mut w, &mut j, order, Vec3::splat(100.0));
    assert_eq!(w.get(cell), B::Planks);
    assert_eq!(j.carry.unwrap().count, 1);
    assert!(j.build.is_empty());
    assert_eq!(material_total(&w,&j,pile,order),2);
    assert!(j.valid());
    jobs::construction::place(&mut w, &mut j, order, Vec3::splat(100.0));
    assert_eq!(j.carry.unwrap().count,1,"duplicate placement never consumes twice");
    assert_eq!(material_total(&w,&j,pile,order),2);
}

#[test]
fn city_quarter_save_preserves_projects_and_legacy_jobs_still_decode() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.farming.jobs.quarter = Some(State {
        frontier: None,
        layout: None,
        lots: vec![lot(2)],
        housing_demand: 4,
        residents: 20,
        paused: true,
        completed: 0,
        origin: [0, 0, 0],
        status: "Paused".into(),
    });
    let bytes = crate::world::farming::sidecar::encode(&w);
    let (loaded, _) = crate::world::farming::sidecar::decode(&bytes).unwrap();
    assert_eq!(loaded.jobs.quarter.unwrap().lots[0].complexity, 2);
    w.farming.jobs.quarter = None;
    w.farming.jobs.worker = Some(0);
    let old = crate::world::farming::sidecar::encode(&w);
    let (loaded, _) = crate::world::farming::sidecar::decode(&old).unwrap();
    assert!(loaded.jobs.quarter.is_none());
    assert_eq!(loaded.jobs.worker, Some(0));
}

#[test]
fn city_quarter_pause_and_obstruction_do_not_advance_or_erase() {
    let (mut w,_,order)=assigned_world();
    w.farming.jobs.build.clear();
    let mut parcel=lot(1);
    parcel.origin=order.cell().to_array();
    let mut city = State {
        frontier: None,
        layout: None,
        lots: vec![parcel],
        housing_demand: 4,
        residents: 20,
        paused: true,
        completed: 0,
        origin: [0, 0, 0],
        status: String::new(),
    };
    advance(&mut w, &mut city);
    assert_eq!(city.lots[0].layer, 0);
    assert_eq!(city.completed,0);
    assert_eq!(city.housing_demand,4);
    assert!(w.farming.jobs.build.is_empty());
    city.paused = false;
    let at = IVec3::from(city.lots[0].origin);
    w.set(at, B::Stone);
    advance(&mut w, &mut city);
    assert_eq!(w.get(at), B::Stone);
    assert_eq!(city.status, "Parcel Obstructed");
    assert_eq!(city.lots[0].layer,0);
    assert_eq!(city.completed,0);
    assert_eq!(city.housing_demand,4);
    assert!(w.farming.jobs.build.is_empty());
}
