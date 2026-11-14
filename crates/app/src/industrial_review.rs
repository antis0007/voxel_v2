//! Native review fixture. The fixed-step game runs all processing and casting.
use crate::{
    player::Player,
    world::{
        self, BlockId as B, World, factory as f,
        inventory::{Item, ItemStack},
        smeltery,
    },
};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
mod processors;

const QUARRY: IVec3 = IVec3::ZERO;
const WASHER: IVec3 = IVec3::X;
const FURNACE: IVec3 = IVec3::new(2, 0, 0);
const CONTROLLER: IVec3 = IVec3::new(4, 0, 0);
const DRAIN: IVec3 = IVec3::new(5, 0, -2);
const FAUCET: IVec3 = IVec3::new(5, 0, -3);
const TABLE: IVec3 = IVec3::new(5, -2, -3);

#[derive(Default)]
pub struct Review {
    assembler: Option<world::factory::assembler::window_review::Review>,
    processors: Option<processors::Review>,
    origin: IVec3,
    size: i32,
    pit: Vec<IVec3>,
    metal: usize,
    phase: Phase,
    casts: u32,
    capture: Option<&'static str>,
    saved_mold: Option<B>,
}

#[derive(Default, PartialEq)]
enum Phase {
    #[default]
    Waiting,
    Gold,
    Saving,
    Collect,
    Loading,
    Done,
}

impl Review {
    pub fn enabled(benchmark: bool) -> Option<Self> {
        if !benchmark || std::env::var_os("VOXEL_SAVE_DIR").is_none()
            || std::env::var_os("VOXEL_INDUSTRIAL_REVIEW").is_none() { return None; }
        let mut review = Self::default();
        if std::env::var("VOXEL_INDUSTRIAL_REVIEW").is_ok_and(|v| v == "assembler") {
            review.assembler = Some(world::factory::assembler::window_review::Review::new()?);
        }
        if std::env::var("VOXEL_INDUSTRIAL_REVIEW").is_ok_and(|v| v == "processors") {
            review.processors = Some(processors::Review::default());
        }
        Some(review)
    }

    fn at(&self, p: IVec3) -> IVec3 {
        self.origin + p * self.size
    }

    pub fn install(&mut self, w: &mut World, p: &mut Player) {
        if let Some(review) = self.processors.as_mut() { review.install(w, p); return; }
        if let Some(review) = self.assembler.as_mut() { review.install(w, p); return; }
        use f::Kind::*;
        self.size = f::step(w);
        self.origin = f::anchor(p.pos.floor().as_ivec3(), self.size) + IVec3::Y * 4 * self.size;
        let s = self.size;
        for (cell, kind) in [
            (QUARRY, Quarry),
            (WASHER, Washer),
            (FURNACE, Furnace),
            (IVec3::new(3, 0, 0), Pipe),
            (CONTROLLER, SmelteryController),
            (FAUCET, SmelteryFaucet),
            (TABLE, CastingTable),
            (WASHER + IVec3::Y, FluidPipe),
            (WASHER + IVec3::Y * 2, Tank),
            (WASHER - IVec3::Y, Storage),
        ] {
            f::fixture_place(w, self.at(cell), kind).unwrap();
        }
        for cell in f::multiblock::shell(SmelteryController, self.at(CONTROLLER), IVec3::X, s) {
            f::fixture_place(
                w,
                cell,
                if cell == self.at(DRAIN) {
                    SmelteryDrain
                } else {
                    SearedBrick
                },
            )
            .unwrap();
        }
        for x in 0..=2 {
            f::fixture_place(w, self.at(IVec3::new(x, 0, 1)), Cable).unwrap();
            f::fixture_place(w, self.at(IVec3::new(x, 0, 2)), Generator)
                .unwrap()
                .input[3] = 1024;
        }
        let tank = self.at(WASHER + IVec3::Y * 2);
        w.factory.nodes.get_mut(&tank).unwrap().water = 2048;
        let controller = self.at(CONTROLLER);
        w.factory.nodes.get_mut(&controller).unwrap().input[smeltery::DEF.fuel] = 1024;
        let quarry = self.at(QUARRY);
        self.pit = (0..9 * s.pow(3))
            .map(|i| quarry + f::quarry::offset(i, s, 0) - IVec3::Z * 2 * s)
            .collect();
        w.set_many(
            &self
                .pit
                .iter()
                .map(|&q| (q, B::Paydirt))
                .collect::<Vec<_>>(),
        );
        let operator = quarry.as_vec3() + Vec3::new(0.5, 1.0, -0.5) * s as f32;
        for action in [
            f::Action::SetRecipe(quarry, 0),
            f::Action::SetTarget(quarry, 1),
            f::Action::SetQuarryDirection(quarry, 0),
            f::Action::SetPlacer(quarry, true),
            f::Action::SetRecipe(self.at(WASHER), f::placer::SLUICE_MODE),
            f::Action::SetRecipe(self.at(FURNACE), f::recipes::GOLD_MODE),
            f::Action::Toggle(quarry),
        ] {
            f::action(w, operator, action);
        }
        p.pos = self.at(IVec3::new(3, 3, -7)).as_vec3();
        let delta = self.at(CONTROLLER).as_vec3() - p.eye(w.voxel_scale);
        p.yaw = delta.x.atan2(-delta.z);
        p.pitch = (delta.y / delta.length()).asin();
        self.metal = smeltery::DEF
            .metals
            .iter()
            .position(|m| m.resource == f::GOLD_INGOT)
            .unwrap();
        self.phase = Phase::Gold;
        world::storage::set_autosave_enabled(false);
        println!(
            "[industrial-review] started size={} ore_cells={}",
            w.voxel_scale.meters(),
            self.pit.len()
        );
    }

    pub fn frame(
        &mut self,
        w: &mut World,
        p: &mut Player,
        day: &mut world::DayNightCycle,
        saves: &mut world::storage::SaveService,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) -> bool {
        if let Some(review) = self.processors.as_mut() { review.frame(w); return false; }
        if let Some(review) = self.assembler.as_mut() { return review.frame(w, p, day, saves); }
        if matches!(self.phase, Phase::Waiting | Phase::Done) {
            return false;
        }
        let table = self.at(TABLE);
        let faucet = self.at(FAUCET);
        let controller = self.at(CONTROLLER);
        let mould = table + IVec3::new(self.size / 2, self.size, self.size / 2);
        let metal = &smeltery::DEF.metals[self.metal];
        let ingot = Item::Resource(metal.resource as u8);
        match self.phase {
            Phase::Gold | Phase::Loading => {
                if !w.factory.nodes[&faucet].enabled {
                    smeltery::interact(w, faucet, blocks, chunks);
                }
                let cast = w.get(mould);
                if self.phase == Phase::Gold {
                    if cast != metal.liquid && cast != metal.solid {
                        return false;
                    }
                    assert!(self.pit.iter().all(|&q| w.get(q) == B::Air));
                    let washed = w.factory.nodes[&self.at(WASHER)].blocks.output
                        [f::blocks::Material::Sand.index()];
                    assert!(washed <= self.pit.len() as u32, "washing duplicated byproducts");
                    // The first cast can arrive before the final ore batch.
                    // Verify the completed pipeline, not simultaneous machines.
                    if washed < self.pit.len() as u32 { return false; }
                    println!(
                        "[industrial-review] quarry, sluice, gold furnace and first gold pour reached the mold"
                    );
                    self.saved_mold = Some(cast);
                    saves.request(true);
                    saves.tick(Some(w), p, day);
                    self.phase = Phase::Saving;
                    self.capture = Some("gold-cast.png");
                } else if cast == metal.solid {
                    self.phase = Phase::Collect;
                }
            }
            Phase::Saving => {
                saves.tick(Some(w), p, day);
                if saves.busy() {
                    return false;
                }
                assert!(!saves.failed, "industrial checkpoint: {:?}", saves.status);
                let loaded =
                    world::storage::load_review_checkpoint().expect("industrial checkpoint load");
                assert_eq!(Some(loaded.world.get(mould)), self.saved_mold);
                assert!(self.pit.iter().all(|&q| loaded.world.get(q) == B::Air));
                *w = loaded.world;
                *p = loaded.player;
                *day = loaded.day_night;
                self.phase = Phase::Collect;
                println!(
                    "[industrial-review] reloaded {:?} and excavated terrain from disk",
                    self.saved_mold
                );
                return true;
            }
            Phase::Collect => {
                if w.get(mould) != metal.solid {
                    return false;
                }
                if self.casts == 0 {
                    let original = w.inventory.clone();
                    let filler = Item::Block(B::Dirt);
                    let stack = Some(ItemStack::new(filler, w.inventory.stack_limit(filler)));
                    w.inventory.hotbar.fill(stack);
                    w.inventory.pack.fill(stack);
                    assert_eq!(w.inventory.remaining_capacity_for(ingot), 0);
                    smeltery::interact(w, table, blocks, chunks);
                    assert_eq!(w.get(mould), metal.solid);
                    assert_eq!(w.inventory.count_item(ingot), 0);
                    w.inventory = original;
                    w.sync_from_inventory();
                    println!("[industrial-review] full inventory kept the entire cast in the mold");
                }
                let metal_count = smeltery::DEF.metals.len() as u32;
                let output = if self.casts == metal_count {
                    Item::Block(metal.solid)
                } else if self.casts == metal_count + 1 {
                    Item::Part { kind: 0, material: smeltery::material_for(metal.solid).expect("tool metal") }
                } else {
                    ingot
                };
                let count = if output == ingot { smeltery::INGOTS_PER_BLOCK } else { 1 };
                let before = w.inventory.count_item(output);
                let ingots_before = w.inventory.count_item(ingot);
                for _ in 0..2 {
                    smeltery::interact(w, table, blocks, chunks);
                }
                assert_eq!(
                    w.inventory.count_item(output),
                    before + count
                );
                if matches!(output, Item::Part { .. }) {
                    let cost = u32::from(engine_tools::PARTS.part_kinds[0].material_cost);
                    assert_eq!(w.inventory.count_item(ingot), ingots_before + smeltery::INGOTS_PER_BLOCK - cost);
                    assert_eq!(w.factory.nodes[&table].target, 0);
                }
                assert_eq!(w.get(mould), B::Air);
                println!(
                    "[industrial-review] collected {count} {output:?}; second collect yielded zero"
                );
                self.casts += 1;
                if self.casts == metal_count + 2 {
                    self.phase = Phase::Done;
                    self.capture = Some("all-metals.png");
                    println!(
                        "[industrial-review] PASS size={} casts={}",
                        w.voxel_scale.meters(),
                        self.casts
                    );
                } else {
                    self.metal = if self.casts < metal_count { self.casts as usize - 1 } else { 0 };
                    if self.casts >= metal_count {
                        let kind = if self.casts == metal_count { f::Kind::CastingBasin } else { f::Kind::CastingTable };
                        f::fixture_place(w, table, kind).unwrap();
                        if kind == f::Kind::CastingTable {
                            w.adventure.hand_slot = 0;
                            w.inventory.hotbar[0] = Some(ItemStack::new(Item::Cast(0), 1));
                            smeltery::interact(w, table, blocks, chunks);
                            assert_eq!(w.factory.nodes[&table].target, 1);
                        }
                    }
                    let resource = smeltery::DEF.metals[self.metal].resource;
                    let item = Item::Resource(resource as u8);
                    w.adventure.hand_slot = 0;
                    w.inventory.hotbar[0] = Some(ItemStack::new(item, smeltery::INGOTS_PER_BLOCK));
                    for _ in 0..smeltery::INGOTS_PER_BLOCK {
                        smeltery::interact(w, controller, blocks, chunks);
                    }
                    assert!(w.inventory.hotbar[0].is_none());
                    self.phase = Phase::Loading;
                }
            }
            Phase::Waiting | Phase::Done => {}
        }
        false
    }

    pub fn capture(&mut self) -> Option<std::path::PathBuf> {
        if let Some(review) = self.processors.as_mut() { return review.capture(); }
        if let Some(review) = self.assembler.as_mut() { return review.capture(); }
        let name = self.capture.take()?;
        let directory = std::env::var_os("VOXEL_CAPTURE_DIR")?;
        Some(std::path::PathBuf::from(directory).join(name))
    }
    pub fn done(&self) -> bool {
        if let Some(review) = self.processors.as_ref() { return review.done; }
        self.assembler.as_ref().map_or(self.phase == Phase::Done, |review| review.done())
    }
}
