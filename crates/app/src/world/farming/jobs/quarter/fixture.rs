use super::{B, IVec3, Lot, State, Vec3, World, geometry, traditions};
use crate::world::{
    adventure::Enemy,
    inventory::{Item, ItemStack},
};

pub(super) fn install(w: &mut World, p: &mut crate::player::Player) -> Result<(), String> {
    if super::frontier::enabled() {return super::frontier::install(w,p);}
    #[cfg(feature = "multiplayer")]
    if w.multiplayer.is_some() {
        return Err("Construction quarter is local-only".into());
    }
    if w.world_type != crate::world::WorldType::Superflat || w.voxel_scale.meters() != 1.0 {
        return Err("City quarter requires a one-metre Superflat world".into());
    }
    let styles = traditions()?;
    let ground = engine_worldgen::terrain_height(&w.sample_config(), 0, 0) + 1;
    let base = IVec3::new(0, ground, 0);
    let style = std::env::var("VOXEL_CITY_TRADITION")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
        .min(2);
    let complexity = std::env::var("VOXEL_CITY_COMPLEXITY")
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1)
        .clamp(1, 3);
    let profile = super::layout::selected()?;
    if super::layout::district::enabled() && (profile.is_none() || super::market_fixture::enabled() && style!=0) {
        return Err("Districts require a street profile; live timber barter requires timber framing".into());
    }
    if profile.is_some() && super::market_fixture::enabled() && !super::layout::district::enabled() {
        return Err("Layout fixture and market fixture require separate runs".into());
    }
    let layout = profile.map(|p| if super::layout::district::enabled() {
        super::layout::district::generate(super::layout::seed(w.seed),p,style,complexity,&super::layout::scene::height)
    } else {super::layout::generate(super::layout::seed(w.seed),p,style,complexity,&super::layout::scene::height)}).transpose()?;
    let edits = if let Some(plan) = &layout { super::layout::scene::edits(plan,base)? }
        else { legacy_scene(w,base,&styles) };
    w.set_many(&edits);
    let pile = base + super::layout::STOCKPILE;
    w.set(pile, B::Chest);
    let feet = (base + IVec3::new(1, 1, 9)).as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let mut builder = Enemy::new(IVec3::new(0, 0, 8), 0, 40, feet, 0);
    builder.home = feet;
    w.adventure.enemies.push(builder);
    w.travel.homes.insert(0, feet);
    super::super::assign(w, 0);
    super::super::designate(w, pile);
    super::super::set_priorities(w, 0, [0, 0, 1]);
    for (id, dx) in [(1u32, -1.0), (2u32, 1.0)] {
        let pos = feet + Vec3::new(dx, 0.0, 2.0);
        let mut helper = Enemy::new(IVec3::new(id as i32, 0, 8), 0, 40, pos, 0);
        helper.home = pos;
        w.adventure.enemies.push(helper);
        w.travel.homes.insert(id, pos);
        if !super::super::crew::add(w, id) { return Err("Builder crew assignment failed".into()); }
    }
    let lot = if let Some(plan) = &layout { plan.parcels[0].lot(base, true) } else { Lot {
        origin: (base + IVec3::new(5, 1, 5)).to_array(),
        tradition: style,
        complexity,
        turn: false,
        layer: 0,
        occupied: false,
        market_cover: super::market_fixture::enabled(),
        frontage: false,
        roof_access: false,
        variant: engine_core::hash::hash_u32(w.seed),
    }};
    let mut counts = std::collections::BTreeMap::<u32, u32>::new();
    for (_, block) in geometry::plan(&lot, &styles[style]) {
        let item = Item::canonical_block(block).ok_or("Unsupported building material")?;
        *counts.entry(item as u32).or_default() += 1;
    }
    if super::market_fixture::enabled() {
        let timber = counts.get_mut(&(B::Log as u32)).ok_or("No timber in bill")?;
        *timber = timber.checked_sub(super::market_fixture::TIMBER).ok_or("Timber bill too small")?;
        if let Some(d)=layout.as_ref().and_then(|p|p.design.as_ref()) {
            let seller=base+IVec3::from(d.merchant);
            let source=d.timber.iter().map(|p|(base+IVec3::from(*p)).to_array()).collect();
            let feet=(base+IVec3::from(d.merchant_feet)).as_vec3()+Vec3::new(0.5,0.0,0.5);
            super::market_fixture::install_at(w,pile,seller,source,feet)?;
        } else {super::market_fixture::install(w, base, pile)?;}
    }
    for (id, count) in counts {
        let mut stack = ItemStack::new(Item::Block(B::from_u32(id)), count);
        crate::world::block_data::store_hauled(w, pile, &mut stack);
        if stack.count != 0 {
            return Err("Construction stockpile is full".into());
        }
    }
    // District review scenes count the physically assigned roster, including a staffed supplier.
    let residents = if layout.as_ref().is_some_and(|p| p.design.is_some()) {
        super::super::crew::count(w) as u32
    } else { 20 };
    w.farming.jobs.quarter = Some(State {
        frontier: None,
        layout,
        lots: vec![lot],
        housing_demand: 4,
        residents,
        paused: false,
        completed: 0,
        origin: base.to_array(),
        status: "Housing Deficit".into(),
    });
    p.pos = base.as_vec3() + Vec3::new(0.5, 1.0, 20.5);
    p.precise_pos = p.pos.as_dvec3();
    p.vel = Vec3::ZERO;
    p.yaw = 0.0;
    p.pitch = 0.02;
    p.flying = false;
    println!(
        "[city-quarter] installed style={} complexity={complexity} ground={ground}",
        styles[style].name
    );
    Ok(())
}

fn legacy_scene(w: &World, base: IVec3, styles: &[super::Tradition]) -> Vec<(IVec3, B)> {
    let mut edits = Vec::new();
    // This explicit disposable fixture supplies its streets and existing buildings.
    // Only the vacant south-east parcel enters the paid construction pipeline.
    for z in -26i32..=26 {
        for x in -42i32..=42 {
            let road = x.abs() <= 2 || z.abs() <= 2 || z.abs() == 23 || x == 21 || x == -23;
            let paving = if road { B::Cobblestone } else { B::Grass };
            edits.push((base + IVec3::new(x, 0, z), paving));
        }
    }
    let specs = [
        (-19, 5, 0, 2, false),
        (-5, -5, 1, 2, true),
        (19, -5, 2, 3, true),
        (-34, 5, 2, 1, false),
        (34, -5, 0, 1, true),
    ];
    for (x, z, style, complexity, turn) in specs {
        let lot = Lot {
            origin: (base + IVec3::new(x, 1, z)).to_array(),
            tradition: style,
            complexity,
            turn,
            layer: 0,
            occupied: true,
            market_cover: false,
                frontage: false,
        roof_access: false,
            variant: engine_core::hash::hash_u32(w.seed ^ x as u32 ^ (z as u32).rotate_left(12)),
        };
        edits.extend(
            geometry::plan(&lot, &styles[style])
                .into_iter()
                .map(|(cell, b)| (geometry::at(&lot, cell), b)),
        );
    }
    for (x, z) in [(25, 8), (35, 9), (26, 18), (36, 19), (-27, -11), (-32, -18)] {
        for y in 1..=4 {
            edits.push((base + IVec3::new(x, y, z), B::Log));
        }
        for dy in 3..=5 {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    if dx.abs() + dz.abs() < 4 {
                        edits.push((base + IVec3::new(x + dx, dy, z + dz), B::Leaves));
                    }
                }
            }
        }
        edits.push((base + IVec3::new(x + 3, 1, z), B::Bush));
    }
    for x in 24..=37 {
        edits.push((base + IVec3::new(x, 0, 14), B::Gravel));
    }
    edits
}
