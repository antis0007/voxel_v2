//! A finite, metered headrace. The same pulse drives the wheel in both
//! modes. Away from the site, only the unapplied water movement is batched.
use super::{
    BlockId as B, World,
    inventory::{FoodKind, Item, ItemStack},
};
use crate::player::Player;
use glam::IVec3;

#[path = "mill_review.rs"]
pub(crate) mod review;

const PULSE_TICKS: u32 = 12 * 120;
const FULL: u8 = 8;
const WHEAT: Item = Item::Block(B::WheatCrop);
const BREAD: Item = Item::Food(FoodKind::Bread);

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct Mill {
    origin: [i32; 3],
    passed: u8,
    published: u8,
    ticks: u32,
    wheat: u32,
    flour: u32,
    baked: u32,
    collected: u32,
    unattended_flour: u32,
    last_return: (u8, u32),
    low_open: bool,
    high_open: bool,
    wheel: bool,
}

impl Mill {
    fn origin(self) -> IVec3 {
        IVec3::from_array(self.origin)
    }
    fn head(self) -> IVec3 {
        self.origin() + IVec3::new(0, 2, 0)
    }
    fn tail(self) -> IVec3 {
        self.origin() + IVec3::new(7, 0, 0)
    }
    fn low(self) -> IVec3 {
        self.origin() + IVec3::new(3, 1, 0)
    }
    fn high(self) -> IVec3 {
        self.low() + IVec3::Y
    }
    fn wheel(self) -> IVec3 {
        self.origin() + IVec3::new(6, 0, 0)
    }

    pub(crate) fn valid(self, world: &World) -> bool {
        let cfg = world.sample_config();
        self.origin.iter().all(|p| p.unsigned_abs() < (1 << 27))
            && self.origin[1] > cfg.min_world_y()
            && self.origin[1] < cfg.max_world_y() - 4
            && world.world_type == super::WorldType::Planet
            && super::environment::State::enabled(world.generator_version)
            && self.published <= self.passed
            && self.passed <= FULL
            && self.ticks < PULSE_TICKS
            && self.wheat <= 24
            && self.flour <= 24
            && self.baked <= FULL.into()
            && self.collected <= self.baked
            && self.unattended_flour <= u32::from(self.passed - self.published) * 3
            && self.last_return.0 <= self.published
            && self.last_return.1 <= u32::from(self.last_return.0) * 3
            && self.flour + self.baked * 3 <= u32::from(self.passed) * 3
    }

    fn observe(&mut self, p: IVec3, block: B) {
        if p == self.low() {
            self.low_open = block == B::Air;
        }
        if p == self.high() {
            self.high_open = block == B::Air;
        }
        if p == self.wheel() {
            self.wheel = block == B::CopperBlock;
        }
    }

    fn runs(self) -> bool {
        self.passed < FULL && (self.low_open || (self.high_open && self.passed < FULL / 2))
    }

    fn advance(&mut self) {
        if !self.runs() {
            return;
        }
        self.ticks += 1;
        if self.ticks < PULSE_TICKS {
            return;
        }
        self.ticks = 0;
        self.passed += 1;
        if self.wheel {
            let milled = self.wheat.min(3);
            self.wheat -= milled;
            self.flour += milled;
        }
    }

    // The voxel cells contain the last materialized checkpoint. Outstanding
    // transfer debt is already included in passed and has already powered the wheel.
    fn publish(&mut self, world: &mut World) -> bool {
        if self.published == self.passed {
            return true;
        }
        let from = self.head();
        let to = self.tail();
        let moved = self.passed - self.published;
        let source = FULL - self.published;
        if ![from, to]
            .into_iter()
            .all(|p| world.admit_edit_chunk(super::chunk_coord_from_world_block(p)))
            || world.water_microlitres(from) != u64::from(source) * world.environment.quantum()
            || world.water_microlitres(to)
                != u64::from(self.published) * world.environment.quantum()
            || !world.apply_metered_water_pair(from, source - moved, to, self.passed, moved)
        {
            return false;
        }
        self.published = self.passed;
        true
    }
}

pub(super) fn is_wheel(world: &World, p: IVec3) -> bool {
    world.mill.is_some_and(|m| m.wheel && p == m.wheel())
}

pub(super) fn owns_water(world: &World, p: IVec3) -> bool {
    world.mill.is_some_and(|m| p == m.head() || p == m.tail())
}

pub(super) fn edited(world: &mut World, p: IVec3, block: B) {
    if let Some(mill) = &mut world.mill {
        mill.observe(p, block);
    }
}

pub(crate) fn tick(world: &mut World, player: &Player) {
    let Some(mut mill) = world.mill else {
        return;
    };
    let near = player.anchor.is_none()
        && (player.pos - mill.origin().as_vec3()).length_squared()
            <= (48.0 / world.voxel_scale.meters()).powi(2);
    if near {
        // Check before simulating a new local pulse. Deferred publication never
        // causes the background interval or its production to run twice.
        let unattended = mill.passed - mill.published;
        if !mill.publish(world) {
            return;
        }
        if unattended > 0 {
            mill.last_return = (unattended, std::mem::take(&mut mill.unattended_flour));
            world.expedition.say(format!(
                "While away: {unattended} water releases, {} flour milled",
                mill.last_return.1
            ));
        }
        let previous = mill;
        mill.advance();
        if !mill.publish(world) {
            mill = previous;
        }
    } else {
        let flour = mill.flour;
        mill.advance();
        mill.unattended_flour += mill.flour - flour;
    }
    world.mill = Some(mill);
}

fn commission(world: &mut World, player: &Player) -> String {
    if world.mill.is_some() {
        return "This planet already has a mill.".into();
    }
    if !world.rules.mode.creative_tools() || player.anchor.is_some() {
        return "Commission the waterworks from the home planet in Creative.".into();
    }
    let origin = player.pos.floor().as_ivec3() + IVec3::X * 4;
    let mut mill = Mill {
        origin: origin.to_array(),
        passed: 0,
        published: 0,
        ticks: 0,
        wheat: 0,
        flour: 0,
        baked: 0,
        collected: 0,
        unattended_flour: 0,
        last_return: (0, 0),
        low_open: false,
        high_open: false,
        wheel: true,
    };
    if !mill.valid(world) || world.ca_view.is_some() {
        return "Commission on a Planet world with exact water enabled.".into();
    }
    let mut layout = Vec::new();
    for x in 0..=7 {
        layout.push((origin + IVec3::new(x, -1, 0), B::StoneBricks));
    }
    for water in [mill.head(), mill.tail()] {
        for offset in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::NEG_Y] {
            layout.push((water + offset, B::StoneBricks));
        }
    }
    layout.retain(|(p, _)| *p != mill.wheel());
    layout.extend([
        (mill.low(), B::IronBlock),
        (mill.high(), B::IronBlock),
        (mill.wheel(), B::CopperBlock),
    ]);
    layout.sort_unstable_by_key(|(p, _)| p.to_array());
    layout.dedup_by_key(|(p, _)| *p);
    let water_cells = [mill.head(), mill.tail()];
    for p in layout.iter().map(|(p, _)| *p).chain(water_cells) {
        if !world.admit_edit_chunk(super::chunk_coord_from_world_block(p))
            || super::effects::admit(world, p).is_err()
        {
            return "Could not reserve the site. Try another location.".into();
        }
        let block = world.get(p);
        if !(block == B::Air || (p.y == origin.y - 1 && block.is_solid())) {
            return format!(
                "Site obstructed by {} at {p}. Use open ground or hover above it.",
                block.display_name()
            );
        }
    }
    // Use the existing solid ground as the foundation where available.
    layout.retain(|&(p, _)| world.get(p) == B::Air);
    if world.environment.water_capacity_ul(mill.tail())
        < world.environment.quantum() * u64::from(FULL)
    {
        return "The tail tank needs a dry site with room for the full head tank.".into();
    }
    let head = mill.head();
    let fill = world.environment.quantum() * u64::from(FULL);
    if !world.set_water_microlitres(head, fill) {
        return "The head tank could not be filled. Try another site.".into();
    }
    world.environment.ledger.edited_water_added_ul += fill;
    world.set_many(&layout);
    for p in [mill.low(), mill.high(), mill.wheel()] {
        mill.observe(p, world.get(p));
    }
    world.mill = Some(mill);
    format!(
        "Waterworks commissioned at {origin}. Break the lower iron plug to drain the head tank through the copper mill. /mill load adds wheat."
    )
}

pub(crate) fn command(world: &mut World, player: &Player, args: &str) -> String {
    if args == "build" {
        return commission(world, player);
    }
    let Some(mut mill) = world.mill else {
        return "In Creative, use /mill build on the home planet.".into();
    };
    let response = match args {
        "load" | "bake" | "collect" => {
            if player.anchor.is_some()
                || (player.pos - mill.wheel().as_vec3()).length_squared()
                    > (6.0 / world.voxel_scale.meters()).powi(2)
                || !world.rules.mode.interacts()
                || world.adventure.is_dead()
                || !mill.wheel
            {
                return "Stand beside the intact copper mill to handle its stock.".into();
            }
            match args {
                "load" => {
                    let count = (24 - mill.wheat).min(world.inventory.count_item(WHEAT));
                    if count == 0 || !world.inventory.remove_item(WHEAT, count) {
                        return "Bring harvested wheat, or wait for room in the hopper.".into();
                    }
                    mill.wheat += count;
                    world.sync_from_inventory();
                    format!("Loaded {count} wheat.")
                }
                "bake" => {
                    let count = mill.flour / 3;
                    mill.flour %= 3;
                    mill.baked += count;
                    format!("Baked {count} loaves from the mill flour.")
                }
                _ => {
                    let count = mill.baked - mill.collected;
                    if count == 0 {
                        return "No loaves waiting. Use /mill bake beside the mill.".into();
                    }
                    let (accepted, _) = world.inventory.try_insert(ItemStack::new(BREAD, count));
                    mill.collected += accepted;
                    world.sync_from_inventory();
                    format!("Collected {accepted} of {count} loaves.")
                }
            }
        }
        "" | "status" => {
            let state = if mill.passed == FULL {
                "head tank empty"
            } else if mill.runs() && !mill.wheel {
                "flowing, wheel missing"
            } else if mill.runs() {
                "flowing"
            } else {
                "water held by spillway"
            };
            let litres = world.environment.quantum() as f64 / 1_000_000.0;
            format!(
                "Mill at {} · {state} · head {:.1} L · tail {:.1} L · {} wheat · {} flour · {} loaves · last return: {} water releases, {} flour",
                mill.wheel(),
                f64::from(FULL - mill.passed) * litres,
                f64::from(mill.passed) * litres,
                mill.wheat,
                mill.flour,
                mill.baked - mill.collected,
                mill.last_return.0,
                mill.last_return.1
            )
        }
        _ => "/mill build | status | load | bake | collect".into(),
    };
    world.mill = Some(mill);
    response
}
