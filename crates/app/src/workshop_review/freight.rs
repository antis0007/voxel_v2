//! Fixed-seed continuation using the live inventory, machines, lift and hatches.
use super::*;
use glam::Vec3Swizzles;
use std::collections::VecDeque;
use world::{BlockId as B, factory::{self as f, Action as A, Kind as K},
    inventory::{Item, SlotAddr}, orbital::{cargo, express, habitat}};
mod navigation;

pub(super) enum Progress { Pending, Failed, Complete }

#[derive(Clone, Copy)]
enum Task {
    Walk(Vec3, f32), Loot(IVec3), Smelt, Craft(Item, u32), Place(K, IVec3),
    Machine(A), WaitStock(usize, u32), Board, Dock, StationWalk(f32),
    Hatch(usize, bool), Order, Descend, Ground, Collect, Install,
}

impl Task {
    fn label(self) -> &'static str {
        match self {
            Self::Walk(..) => "walk", Self::Loot(_) => "salvage", Self::Smelt => "smelt",
            Self::Craft(..) => "craft", Self::Place(..) => "place", Self::Machine(_) => "configure",
            Self::WaitStock(..) => "deliver to storage", Self::Board => "board", Self::Dock => "dock",
            Self::StationWalk(_) => "station walk", Self::Hatch(..) => "cycle hatch", Self::Order => "order freight",
            Self::Descend => "descend", Self::Ground => "arrive at terminal", Self::Collect => "collect mithril",
            Self::Install => "install mithril",
        }
    }
}

pub(super) struct Route {
    supply: cargo::Supply,
    tasks: VecDeque<Task>,
    started: f32,
    last_probe: (f32, f32),
    detours: u32,
    crown: Vec3,
    bank: IVec3,
    detour_path: VecDeque<Vec3>,
    detour_step_at: f32,
}

const GEN: IVec3 = IVec3::new(-3, 37, -32);
const CABLE: IVec3 = IVec3::new(-2, 37, -32);
const ALLOY: IVec3 = IVec3::new(-1, 37, -32);
const PIPE: IVec3 = IVec3::new(0, 37, -32);
const STOCK: IVec3 = IVec3::new(1, 37, -32);
const BANK: IVec3 = IVec3::new(2, 37, -32);
const CULT: IVec3 = IVec3::new(0, 37, -31);
const BENCH: Vec3 = Vec3::new(-0.8, 37.0, -33.0);
const YARD_TURN: Vec3 = Vec3::new(-0.72, 37.0, -32.55);
const YARD_EXIT: Vec3 = Vec3::new(4.5, 37.0, -32.55);
// Surveyed from solid footing and head clearance on the review seed.
// The last point is on the axial approach within boarding range.
const MERIDIAN_APPROACH: [Vec3; 17] = [
    Vec3::new(8.5, 37.0, -31.5),
    Vec3::new(188.5, 37.0, -211.5),
    Vec3::new(354.5, 38.0, -371.5),
    Vec3::new(423.5, 38.0, -409.5),
    Vec3::new(603.5, 44.0, -409.5),
    Vec3::new(611.5, 45.0, -406.5),
    Vec3::new(747.5, 40.0, -395.5),
    Vec3::new(900.5, 55.0, -359.5),
    Vec3::new(958.5, 86.0, -346.5),
    Vec3::new(972.5, 91.0, -347.5),
    Vec3::new(996.5, 86.0, -359.5),
    Vec3::new(1038.5, 80.0, -455.5),
    Vec3::new(1068.5, 63.0, -595.5),
    Vec3::new(1068.5, 56.0, -617.5),
    Vec3::new(1077.5, 52.0, -628.5),
    Vec3::new(1086.5, 43.0, -808.5),
    Vec3::new(1100.5, 43.0, -839.5),
];

impl Route {
    pub(super) fn from_env() -> Option<Self> {
        let supply = match std::env::var("VOXEL_WORKSHOP_FREIGHT").ok()?.as_str() {
            "steel" => cargo::Supply::Steel,
            "ethanol" => cargo::Supply::Fuel,
            _ => return None,
        };
        Some(Self { supply, tasks: VecDeque::new(), started: 0.0,
            last_probe: (0.0, 0.0), detours: 0, crown: Vec3::ZERO, bank: BANK,
            detour_path: VecDeque::new(), detour_step_at: 0.0 })
    }


    fn plan(&mut self, w: &World, now: f32) {
        use Task::*;
        self.started = now;
        self.bank = (0..=2).map(|dx| BANK + IVec3::X * dx).find(|&cell| {
            w.get(cell) == B::Air && w.get(cell - IVec3::Y).is_solid()
        }).unwrap_or(BANK);
        self.tasks.extend([
            Loot(IVec3::new(-4, 37, -45)),
            Walk(Vec3::new(0.5, 36.0, -42.5), 0.35),
            Walk(Vec3::new(0.5, 36.0, -43.5), 0.35),
            Loot(IVec3::new(2, 37, -48)),
            Walk(Vec3::new(0.5, 36.0, -42.5), 0.35),
            Walk(Vec3::new(3.5, 37.0, -42.5), 0.35),
            Walk(Vec3::new(3.5, 37.0, -40.5), 0.35),
            Walk(Vec3::new(6.5, 37.0, -40.5), 0.35),
            Walk(Vec3::new(6.5, 37.0, -43.5), 0.35),
            Loot(IVec3::new(6, 37, -48)),
            Walk(Vec3::new(6.5, 37.0, -40.5), 0.35),
            Walk(Vec3::new(3.5, 37.0, -40.5), 0.35),
            Walk(Vec3::new(3.5, 37.0, -42.5), 0.35),
            Walk(Vec3::new(0.5, 36.0, -42.5), 0.35),
            Walk(Vec3::new(1.4, 37.0, -42.0), 0.45),
            Smelt,
            Walk(Vec3::new(0.28, 37.0, -38.51), 0.35), Walk(BENCH, 0.65),
            Place(K::Generator, GEN), Place(K::Cable, CABLE),
            Place(K::AlloyKiln, ALLOY), Place(K::Pipe, PIPE), Place(K::Storage, STOCK),
            Place(K::Battery, self.bank), Machine(A::UpgradeMachine(self.bank)),
        ]);
        if matches!(self.supply, cargo::Supply::Steel) {
            self.tasks.extend([
                Machine(A::SetRecipe(ALLOY, 1)), Machine(A::BufferMove(ALLOY, 3, 128, true)),
                Machine(A::BufferMove(GEN, 3, u32::MAX, true)), Machine(A::Load(ALLOY)),
                Walk(YARD_TURN, 0.45), Walk(YARD_EXIT, 0.45),
                WaitStock(15, 256), Machine(A::BufferMove(STOCK, 15, 256, false)),
            ]);
        } else {
            self.tasks.extend([
                Machine(A::BufferMove(GEN, 3, u32::MAX, true)),
                Craft(Item::Resource(16), 1), Machine(A::Load(ALLOY)),
                WaitStock(14, 256), Machine(A::BufferMove(STOCK, 14, 256, false)),
                Craft(Item::Resource(13), 9),
                Place(K::Cable, IVec3::new(-2, 37, -31)),
                Place(K::Cable, IVec3::new(-1, 37, -31)), Place(K::Cultivator, CULT),
                Machine(A::SetTarget(CULT, f::fuel_order::REQUEST)), Machine(A::Load(CULT)),
                Walk(YARD_TURN, 0.45), Walk(YARD_EXIT, 0.45),
                WaitStock(f::ETHANOL, 256), Machine(A::BufferMove(STOCK, f::ETHANOL, 256, false)),
            ]);
        }
        self.tasks.push_back(Walk(YARD_EXIT, 0.45));
        self.tasks.extend(MERIDIAN_APPROACH.map(|point| Walk(point, 0.75)));
        self.tasks.extend([
            Board, Dock,
            StationWalk(2.25), Hatch(0, true), StationWalk(7.5), Hatch(1, true),
            StationWalk(12.75), Order, StationWalk(7.5), Hatch(0, true),
            StationWalk(2.25), Hatch(0, false), Descend, Ground, Collect,
        ]);
        self.tasks.extend(MERIDIAN_APPROACH.into_iter().rev().map(|point| Walk(point, 0.75)));
        self.tasks.extend([
            Walk(YARD_EXIT, 0.45), Walk(YARD_TURN, 0.45), Walk(BENCH, 0.65), Install,
        ]);
        println!("[workshop-freight] route={}", self.supply.cost().item.name());
    }

    pub(super) fn input(&mut self, w: &World, p: &mut Player, i: &mut Input, now: f32) {
        let scale = w.voxel_scale.meters();
        let feet = p.pos * scale;
        let target = match self.tasks.front().copied() {
            Some(Task::Walk(target, radius)) => (target, radius),
            Some(Task::StationWalk(x)) => (self.crown + Vec3::X * x, 0.35),
            _ => return,
        };
        let delta = (target.0 - feet) * Vec3::new(1.0, 0.0, 1.0);
        let distance = delta.length();
        if distance <= target.1 { return; }
        if now - self.last_probe.0 > 1.25 {
            if self.last_probe.1 - distance > 0.7 {
                self.last_probe.1 = distance;
                self.detours = 0;
            } else {
                self.detours += 1;
            }
            self.last_probe.0 = now;
            if self.detours > 1 && self.detour_path.is_empty() && p.on_ground
                && matches!(self.tasks.front(), Some(Task::Walk(..)))
            {
                self.detour_path = navigation::detour(w, p, target.0 / scale);
                self.detour_step_at = now;
            }
        }
        while self.detour_path.front().is_some_and(|point| (*point * scale - feet).xz().length() < 0.32) {
            self.detour_path.pop_front();
            self.detour_step_at = now;
        }
        if now - self.detour_step_at > 2.5 {
            self.detour_path.clear();
        }
        if let Some(point) = self.detour_path.front().map(|point| *point * scale) {
            aim(p, Vec3::new(point.x, feet.y + 1.62, point.z), scale);
            i.forward = true;
            i.jump = p.swimming || (p.on_ground && point.y > feet.y + 0.5);
            return;
        }
        let turn = [0.0, 0.65, -0.65, 1.57, -1.57, 2.35, -2.35][self.detours as usize % 7];
        aim(p, feet + delta + Vec3::Y * 1.62, scale);
        if matches!(self.tasks.front(), Some(Task::Walk(..))) { p.yaw += turn; }
        i.forward = true;
        i.sprint = distance > 18.0;
        i.jump = p.swimming || p.underwater
            || (p.on_ground && self.detours > 0 && matches!(self.tasks.front(), Some(Task::Walk(..))));
        i.sneak = distance < 1.0 && !i.jump;
    }

    pub(super) fn tick(&mut self, w: &mut World, p: &mut Player, now: f32,
        blocks: &mut HashSet<IVec3>, chunks: &mut HashSet<IVec3>) -> Progress {
        use Task::*;
        if self.started == 0.0 { self.plan(w, now); }
        let Some(task) = self.tasks.front().copied() else { return Progress::Complete };
        let scale = w.voxel_scale.meters();
        let feet = p.pos * scale;
        let horizontal = |target: Vec3| (feet - target).xz().length();
        let timeout = if matches!(task, Walk(..) | Dock | Ground | Collect) { 500.0 } else { 60.0 };
        if now - self.started > timeout || (self.detours > 18 && self.detour_path.is_empty()) || w.adventure.is_dead() {
            eprintln!("[workshop-freight] FAIL {} feet={feet:.2} ground={} swimming={} hp={:.1} resources={:?} status={}",
                task.label(), p.on_ground, p.swimming, w.adventure.health, w.expedition.resources, w.expedition.message);
            return Progress::Failed;
        }
        if w.adventure.health < 45.0 && w.eat_food(world::inventory::FoodKind::FungalRation, None) {
            println!("[workshop-freight] ate ration feet={feet:.2}");
        }
        let complete = match task {
            Walk(target, radius) => horizontal(target) <= radius,
            StationWalk(x) => horizontal(self.crown + Vec3::X * x) < 0.35,
            Loot(cell) => {
                aim(p, cell.as_vec3() + Vec3::splat(0.5), scale);
                if world::raycast_block(w, p.eye(w.voxel_scale), p.forward_dir(), 5.0 / scale)
                    .is_some_and(|(hit, _)| hit == cell) && world::containers::open(w, cell) {
                    for slot in 0..world::containers::SLOTS { w.inventory.shift_click_slot(SlotAddr::Container(slot)); }
                    world::containers::flush(w);
                    w.inventory.container = None;
                    w.sync_from_inventory();
                    true
                } else { false }
            }
            Smelt => {
                let cell = IVec3::new(4, 37, -40);
                let before = w.expedition.resources;
                aim(p, cell.as_vec3() + Vec3::splat(0.5), scale);
                if let Some((hit, _)) = world::raycast_block(w, p.eye(w.voxel_scale), p.forward_dir(), 6.0 / scale)
                    && w.get(hit) == B::Smelter
                {
                    world::expedition::interact(w, hit, p.pos, blocks, chunks);
                    before == w.expedition.resources
                } else { false }
            }
            Craft(item, count) => {
                if w.inventory.count_item(item) >= count { true }
                else {
                    let bench = f::at_bench(w, p.pos);
                    let recipe = world::inventory::all_recipes().iter().find(|r| r.output.item == item
                        && (item != Item::Resource(13) || r.ingredients.iter().any(|&(i, _)| i == Item::Block(B::Planks))));
                    if let Some(recipe) = recipe && w.inventory.stage_recipe_into_grid(recipe, bench) {
                        w.inventory.craft_from_grid(true, bench);
                        w.sync_from_inventory();
                    }
                    false
                }
            }
            Place(kind, at) => {
                let item = Item::Block(kind.block());
                if w.inventory.count_item(item) == 0 { f::action(w, p.pos, A::Craft(kind)); }
                if (p.eye(w.voxel_scale) - at.as_vec3()).length() < 6.0 {
                    f::place(w, at, kind.block(), p, Some(item), blocks, chunks);
                }
                w.factory.nodes.get(&at).is_some_and(|node| node.kind == kind)
            }
            Machine(action) => {
                f::action(w, p.pos, action);
                match action { A::UpgradeMachine(at) => w.factory.nodes[&at].upgrade == 1, _ => true }
            }
            WaitStock(item, amount) => w.factory.nodes[&STOCK].input[item] >= amount,
            Board => express::board(w, p),
            Dock => {
                if let Some(ride) = w.orbital.express.ride.filter(|r| r.docked() && express::port_ready(w, r)) {
                    self.crown = (ride.berth_m + glam::DVec3::Y * ride.profile.length_m).as_vec3();
                    true
                } else { false }
            }
            Hatch(door, open) => {
                let h = w.orbital.habitats.habitats.iter().find(|h| h.origin.as_vec3().distance(self.crown) < 1.0);
                if h.is_some_and(|h| h.doors_open[door] == open && h.cycle < 0) { true }
                else {
                    aim(p, self.crown + Vec3::new(habitat::HATCH_X[door] as f32 + 0.5, 1.5, 0.0), scale);
                    habitat::interact(w, p);
                    false
                }
            }
            Order => {
                aim(p, self.crown + Vec3::new(13.0, 1.2, 2.0), scale);
                cargo::order_cell(w, p, self.supply, false)
            }
            Descend => express::descend(w),
            Ground => w.orbital.express.ride.is_none(),
            Collect => cargo::collect(w, p) && cargo::cell_ready(w),
            Install => {
                f::action(w, p.pos, A::InstallMithril(self.bank));
                self.installed(w)
            }
        };
        if complete {
            println!("[workshop-freight] {} elapsed={:.2} feet={feet:.2}", task.label(), now - self.started);
            self.tasks.pop_front();
            self.started = now;
            self.detours = 0;
            self.detour_path.clear();
            self.last_probe = (now, f32::INFINITY);
        }
        Progress::Pending
    }

    fn installed(&self, w: &World) -> bool {
        w.factory.nodes.get(&self.bank).is_some_and(|n| n.upgrade == f::power::MITHRIL_TIER)
            && w.inventory.count_item(cargo::CELL) == 0 && !cargo::cell_ready(w)
            && world::workshop::automated(&w.adventure)
    }

    pub(super) fn check_reload(&self, w: &World) {
        if self.installed(w) { println!("[workshop-freight] PASS {} · Mithril bank reloaded", self.supply.cost().item.name()); }
        else { eprintln!("[workshop-freight] FAIL Mithril bank or supply receipt missing after reload"); }
    }
}
