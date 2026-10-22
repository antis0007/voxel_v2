//! Electrical units and battery banks.
//!
//! The factory simulates in fixed 0.25 s ticks and counts energy in integer
//! units. One unit is 250 J, which makes the bookkeeping read directly in SI:
//! a flow of one unit per tick is exactly 1 kW, and 14.4 units are 1 Wh.
//! Every number the interface shows (W, kW, Wh, kWh) comes from here.
use super::{Factory, Kind};
use glam::IVec3;

pub const JOULES_PER_UNIT: f64 = 250.0;
pub const TICK_SECONDS: f64 = 0.25;
/// Units in one watt-hour (3600 J / 250 J).
pub const UNITS_PER_WH: f64 = 3600.0 / JOULES_PER_UNIT;
/// Every consumer keeps a small local buffer it fills from the grid.
pub const CONSUMER_BUFFER: u32 = 64;
/// Generator output buffer (500 kJ).
pub const GENERATOR_BUFFER: u32 = 2000;

/// Battery capacity by tier: 1 kWh, 4 kWh, 16 kWh.
pub fn battery_capacity(tier: u32) -> u32 {
    (UNITS_PER_WH * 1000.0) as u32 * 4u32.pow(tier.min(2))
}

/// Charge and discharge limit per tick by tier: 4 kW, 16 kW, 64 kW.
pub fn battery_rate(tier: u32) -> u32 {
    4 * 4u32.pow(tier.min(2))
}

pub fn battery_tier_name(tier: u32) -> &'static str {
    match tier {
        0 => "Lead-acid",
        1 => "Nickel-iron",
        _ => "Resonant cell",
    }
}

/// Watts for a flow of `units` per tick.
pub fn watts(units_per_tick: f64) -> f64 {
    units_per_tick * JOULES_PER_UNIT / TICK_SECONDS
}

/// `16 kW`, `850 W`, `1.25 MW`.
pub fn fmt_power(units_per_tick: f64) -> String {
    let w = watts(units_per_tick);
    let a = w.abs();
    if a >= 1.0e6 {
        format!("{:.2} MW", w / 1.0e6)
    } else if a >= 1.0e3 {
        let kw = w / 1.0e3;
        if (kw - kw.round()).abs() < 0.05 {
            format!("{kw:.0} kW")
        } else {
            format!("{kw:.1} kW")
        }
    } else {
        format!("{w:.0} W")
    }
}

/// `12.5 Wh`, `3.40 kWh`.
pub fn fmt_energy(units: f64) -> String {
    let wh = units / UNITS_PER_WH;
    if wh.abs() >= 1000.0 {
        format!("{:.2} kWh", wh / 1000.0)
    } else if wh.abs() >= 10.0 {
        format!("{wh:.0} Wh")
    } else {
        format!("{wh:.1} Wh")
    }
}

/// `100 kJ`, `1.2 MJ`, for a batch or a fuel portion.
pub fn fmt_joules(units: f64) -> String {
    let j = units * JOULES_PER_UNIT;
    if j >= 1.0e6 {
        format!("{:.2} MJ", j / 1.0e6)
    } else if j >= 1.0e3 {
        format!("{:.0} kJ", j / 1.0e3)
    } else {
        format!("{j:.0} J")
    }
}

/// Combustion fuels for the coal generator, in the order it prefers them.
/// `(resource, portion taken, units released)`. A portion is 1/4 of an item.
/// Biomass burner (generator mode 2): 200 kJ per item of biomass at 12 kW.
pub const BIOMASS_FUEL: (usize, u32, u32) = (13, 16, 200);
pub const BIOMASS_RATE: u32 = 12;

pub const COMBUSTION_FUELS: [(usize, u32, u32); 4] = [
    (super::COKE, 16, 800),
    (3, 16, 400),
    (super::ETHANOL, 16, 600),
    (super::CREOSOTE, 16, 300),
];
/// Largest portion any combustion fuel releases; bounds a saved `burn`.
pub const MAX_COMBUSTION_BURN: u32 = 800;

/// Battery behaviour, stored in `Node::mode`.
pub const BATTERY_MODES: [&str; 4] = [
    "Balanced: stores surplus and covers shortfalls",
    "Charge only: holds a reserve, never discharges",
    "Discharge only: drains into the grid, never charges",
    "Mana font: within 6 m, turns stored charge into the wielder's mana",
];


/// Measured state of one cable network, refreshed every tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct NetStats {
    /// Units generated this tick (kW).
    pub generation: u32,
    /// Units drawn by machines this tick (kW).
    pub load: u32,
    /// Net battery flow this tick; positive while charging.
    pub battery_flow: i32,
    pub stored: u64,
    pub capacity: u64,
    pub machines: u16,
    pub generators: u16,
    /// Storage ports: batteries, transformers, matrix ports.
    pub batteries: u16,
    /// Cable tier and the units it carried this tick.
    pub tier: u32,
    pub throughput: u32,
}

impl NetStats {
    /// Seconds of the present load the stored charge would cover, if the
    /// grid is running a deficit.
    pub fn runtime_seconds(&self) -> Option<f64> {
        let deficit = self.load as i64 - self.generation as i64;
        (deficit > 0 && self.stored > 0)
            .then(|| self.stored as f64 / deficit as f64 * TICK_SECONDS)
    }
}

/// Transformer throughput by tier: 64 kW, 512 kW, 4 MW. It buffers two
/// ticks' worth, enough to carry energy between two cable networks.
pub fn transformer_rate(tier: u32) -> u32 {
    64 * 8u32.pow(tier.min(2))
}

/// Cable tiers. Cables of different tiers do not join; a transformer (or any
/// storage touching both) carries energy across.
pub fn cable_capacity(tier: u32) -> u32 {
    match tier {
        0 => 256,
        1 => 2000,
        _ => 16000,
    }
}
pub fn cable_tier_name(tier: u32) -> &'static str {
    match tier {
        0 => "Copper cable",
        1 => "HV armoured cable",
        _ => "Resonant superconductor",
    }
}

/// A storage port's view of its energy pool.
#[derive(Clone, Copy)]
struct Store {
    pool: IVec3,
    charge: bool,
    discharge: bool,
}

impl Factory {
    fn store_of(&self, p: IVec3) -> Option<Store> {
        let n = &self.nodes[&p];
        if !n.enabled {
            return None;
        }
        match n.kind {
            Kind::Battery => Some(Store {
                pool: p,
                charge: matches!(n.mode, 0 | 1 | 3),
                discharge: matches!(n.mode, 0 | 2),
            }),
            Kind::Transformer => Some(Store {
                pool: p,
                charge: true,
                discharge: true,
            }),
            Kind::InductionPort if n.formed => n.pool.map(|pool| Store {
                pool,
                charge: n.mode == 0,
                discharge: n.mode == 1,
            }),
            _ => None,
        }
    }

    /// Capacity and per-tick rate of an energy pool.
    pub fn pool_limits(&self, pool: IVec3) -> (u32, u32) {
        let n = &self.nodes[&pool];
        match n.kind {
            Kind::Battery => {
                let rate = battery_rate(n.upgrade);
                let limit = if n.target > 0 { n.target.min(rate) } else { rate };
                (battery_capacity(n.upgrade), limit)
            }
            Kind::Transformer => (transformer_rate(n.upgrade) * 2, transformer_rate(n.upgrade)),
            Kind::InductionPort | Kind::InductionCell => (n.store_cap, n.store_rate),
            _ => (0, 0),
        }
    }

    /// Moves energy over each cable network for one tick: generators feed
    /// machines, storage covers what generators cannot, and any surplus
    /// charges storage. A network carries at most its cable tier's capacity.
    /// Consumer priority rotates so an undersized grid still lets every
    /// machine work some of the time. Each storage pool has one rate
    /// allowance per tick, shared by every network it touches.
    pub(super) fn distribute_power(&mut self) {
        let mut allowance: std::collections::HashMap<IVec3, u32> = Default::default();
        let mut stats = Vec::with_capacity(self.power.len());
        for ni in 0..self.power.len() {
            let ports = self.power[ni].ports.clone();
            let mut budget = cable_capacity(self.power[ni].tier);
            let mut net = NetStats {
                tier: self.power[ni].tier,
                ..NetStats::default()
            };
            if ports.is_empty() {
                stats.push(net);
                continue;
            }
            let stores: Vec<(IVec3, Store)> = ports
                .iter()
                .filter_map(|&p| self.store_of(p).map(|s| (p, s)))
                .collect();
            for (_, s) in &stores {
                let rate = self.pool_limits(s.pool).1;
                allowance.entry(s.pool).or_insert(rate);
            }
            let mut moved = false;
            for k in 0..ports.len() {
                if budget == 0 {
                    break;
                }
                let p = ports[(k + self.cursor) % ports.len()];
                let n = &self.nodes[&p];
                if !n.enabled || !n.kind.powered() {
                    continue;
                }
                let mut need = CONSUMER_BUFFER.saturating_sub(n.energy).min(budget);
                let mut received = 0;
                for g in &ports {
                    if need == 0 {
                        break;
                    }
                    let source = self.nodes.get_mut(g).unwrap();
                    if !source.enabled || !source.kind.generator() {
                        continue;
                    }
                    let transfer = need.min(source.energy);
                    source.energy -= transfer;
                    need -= transfer;
                    received += transfer;
                }
                for &(port, s) in &stores {
                    if need == 0 {
                        break;
                    }
                    if !s.discharge {
                        continue;
                    }
                    let left = allowance.get_mut(&s.pool).unwrap();
                    let pool = self.nodes.get_mut(&s.pool).unwrap();
                    let transfer = need.min(pool.energy).min(*left);
                    pool.energy -= transfer;
                    self.stats.storage(transfer, false, self.nodes[&s.pool].kind == Kind::Transformer);
                    *left -= transfer;
                    need -= transfer;
                    received += transfer;
                    self.nodes.get_mut(&port).unwrap().flow -= transfer as i32;
                }
                let consumer = self.nodes.get_mut(&p).unwrap();
                consumer.energy += received;
                consumer.flow -= received as i32;
                budget -= received;
                net.load += received;
                moved |= received > 0;
            }
            // Surplus: whatever generators still hold tops up storage.
            for &(port, s) in &stores {
                if !s.charge || budget == 0 {
                    continue;
                }
                let cap = self.pool_limits(s.pool).0;
                let left = allowance.get_mut(&s.pool).unwrap();
                let mut room = cap
                    .saturating_sub(self.nodes[&s.pool].energy)
                    .min(*left)
                    .min(budget);
                for g in &ports {
                    if room == 0 {
                        break;
                    }
                    let source = self.nodes.get_mut(g).unwrap();
                    if !source.enabled || !source.kind.generator() {
                        continue;
                    }
                    let transfer = room.min(source.energy);
                    source.energy -= transfer;
                    room -= transfer;
                    *left -= transfer;
                    budget -= transfer;
                    self.nodes.get_mut(&s.pool).unwrap().energy += transfer;
                    self.stats.storage(transfer, true, self.nodes[&s.pool].kind == Kind::Transformer);
                    self.nodes.get_mut(&port).unwrap().flow += transfer as i32;
                    moved |= transfer > 0;
                }
            }
            let mut pools = Vec::new();
            for p in &ports {
                let n = &self.nodes[p];
                if n.kind.generator() {
                    net.generators += 1;
                    net.generation += n.flow.max(0) as u32;
                } else if n.kind.stores_power() {
                    net.batteries += 1;
                    net.battery_flow += n.flow;
                    if let Some(s) = self.store_of(*p) {
                        if !pools.contains(&s.pool) {
                            pools.push(s.pool);
                        }
                    }
                } else if n.kind.powered() {
                    net.machines += 1;
                }
            }
            for pool in pools {
                net.stored += self.nodes[&pool].energy as u64;
                net.capacity += self.pool_limits(pool).0 as u64;
            }
            net.throughput = cable_capacity(net.tier) - budget;
            self.stats.delivery(net.load);
            if moved {
                for c in self.power[ni].conduits.clone() {
                    self.nodes.get_mut(&c).unwrap().activity = 1.0;
                }
            }
            stats.push(net);
        }
        // Storage status lines, once every network has had its turn.
        let storage: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind.stores_power())
            .map(|(p, _)| *p)
            .collect();
        for p in storage {
            let pool = self.store_of(p).map(|s| s.pool).unwrap_or(p);
            let (cap, _) = self.pool_limits(pool);
            let energy = self.nodes[&pool].energy;
            let n = self.nodes.get_mut(&p).unwrap();
            n.status = if n.kind == Kind::InductionPort && !n.formed {
                "Matrix incomplete"
            } else if !n.enabled {
                "Paused"
            } else if n.flow > 0 {
                "Charging"
            } else if n.flow < 0 {
                "Discharging"
            } else if energy >= cap && cap > 0 {
                "Full"
            } else if energy == 0 {
                "Empty"
            } else {
                "Holding"
            };
        }
        self.power_stats = stats;
    }

    /// Stored energy and capacity of whatever pool a storage assembly uses.
    pub fn pool_of(&self, p: IVec3) -> Option<(u32, u32, u32)> {
        let n = self.nodes.get(&p)?;
        let pool = match n.kind {
            Kind::InductionPort => n.pool?,
            k if k.stores_power() => p,
            _ => return None,
        };
        let (cap, rate) = self.pool_limits(pool);
        Some((self.nodes[&pool].energy, cap, rate))
    }

    /// The measured cable network a machine, battery or cable belongs to.
    pub fn grid(&self, cell: IVec3) -> Option<NetStats> {
        self.power_net
            .get(&cell)
            .and_then(|&i| self.power_stats.get(i))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::factory::Node;

    fn grid(parts: &[(i32, Kind)]) -> Factory {
        let mut f = Factory::default();
        for &(x, kind) in parts {
            f.nodes.insert(IVec3::new(x, 0, 0), Node::new(kind));
            f.nodes.insert(IVec3::new(x, 1, 0), Node::new(Kind::Cable));
        }
        f
    }

    #[test]
    fn surplus_charges_a_battery_and_the_battery_covers_a_deficit() {
        let mut f = grid(&[(0, Kind::Generator), (1, Kind::Battery), (2, Kind::Furnace)]);
        f.nodes.get_mut(&IVec3::ZERO).unwrap().input[3] = 64 * 8;
        for _ in 0..40 {
            f.tick(0.25, 1);
        }
        let stored = f.nodes[&IVec3::X].energy;
        assert!(stored > 0, "battery never charged");
        let grid = f.grid(IVec3::X).unwrap();
        assert_eq!(grid.batteries, 1);
        assert_eq!(grid.generators, 1);
        // Take the generator offline: the furnace now runs on the battery.
        f.nodes.get_mut(&IVec3::ZERO).unwrap().enabled = false;
        f.nodes.get_mut(&IVec3::new(2, 0, 0)).unwrap().energy = 0;
        f.tick(0.25, 1);
        assert!(f.nodes[&IVec3::X].energy < stored);
        assert!(f.nodes[&IVec3::new(2, 0, 0)].energy > 0);
        assert!(f.nodes[&IVec3::X].flow < 0);
    }

    #[test]
    fn a_transformer_carries_power_between_cable_tiers() {
        // Generator on copper cable at x 0..1; furnace on HV cable at x 3;
        // the transformer at x 2 touches both lines from below.
        let mut f = Factory::default();
        f.nodes.insert(IVec3::new(0, 0, 0), Node::new(Kind::Generator));
        f.nodes.get_mut(&IVec3::ZERO).unwrap().input[3] = 64 * 8;
        for x in 0..=2 {
            f.nodes.insert(IVec3::new(x, 1, 0), Node::new(Kind::Cable));
        }
        f.nodes.insert(IVec3::new(2, 0, 0), Node::new(Kind::Transformer));
        let mut hv = Node::new(Kind::Cable);
        hv.upgrade = 1;
        f.nodes.insert(IVec3::new(2, -1, 0), hv.clone());
        f.nodes.insert(IVec3::new(3, -1, 0), hv);
        f.nodes.insert(IVec3::new(3, -2, 0), Node::new(Kind::Furnace));
        for _ in 0..12 {
            f.tick(0.25, 1);
        }
        assert!(f.nodes[&IVec3::new(3, -2, 0)].energy > 0, "no power crossed");
        let copper = f.grid(IVec3::new(0, 1, 0)).unwrap();
        let steel = f.grid(IVec3::new(3, -1, 0)).unwrap();
        assert_eq!((copper.tier, steel.tier), (0, 1));
    }

    #[test]
    fn a_copper_line_caps_its_throughput() {
        let mut f = Factory::default();
        f.nodes.insert(IVec3::ZERO, Node::new(Kind::Generator));
        f.nodes.get_mut(&IVec3::ZERO).unwrap().energy = GENERATOR_BUFFER;
        f.nodes.insert(IVec3::Y, Node::new(Kind::Cable));
        for x in 1..=9 {
            f.nodes.insert(IVec3::new(x, 1, 0), Node::new(Kind::Cable));
            f.nodes.insert(IVec3::new(x, 0, 0), Node::new(Kind::Crusher));
        }
        f.tick(0.25, 1);
        let g = f.grid(IVec3::Y).unwrap();
        assert!(g.load <= cable_capacity(0));
        // Nine empty 64-unit buffers want 576; copper carries 256.
        assert_eq!(g.load, cable_capacity(0));
    }

    #[test]
    fn an_induction_matrix_stores_through_its_ports() {
        let mut f = Factory::default();
        let port = IVec3::new(0, 1, 1);
        crate::world::factory::induction::tests::build(&mut f, IVec3::ZERO, 4, port);
        f.nodes.insert(IVec3::new(-1, 1, 1), Node::new(Kind::Cable));
        f.nodes.insert(IVec3::new(-1, 0, 1), Node::new(Kind::Generator));
        f.nodes.get_mut(&IVec3::new(-1, 0, 1)).unwrap().input[3] = 64 * 4;
        for _ in 0..20 {
            f.tick(0.25, 1);
        }
        let (stored, cap, rate) = f.pool_of(port).unwrap();
        assert!(stored > 0);
        assert_eq!(cap, 7 * crate::world::factory::induction::CELL_CAPACITY);
        assert_eq!(rate, crate::world::factory::induction::PROVIDER_RATE);
    }

    #[test]
    fn charge_only_batteries_hold_their_reserve() {
        let mut f = grid(&[(0, Kind::Battery), (1, Kind::Furnace)]);
        let b = f.nodes.get_mut(&IVec3::ZERO).unwrap();
        b.mode = 1;
        b.energy = 1000;
        f.tick(0.25, 1);
        assert_eq!(f.nodes[&IVec3::ZERO].energy, 1000);
        assert_eq!(f.nodes[&IVec3::X].energy, 0);
    }

    #[test]
    fn one_unit_per_tick_is_a_kilowatt() {
        assert_eq!(watts(1.0), 1000.0);
        assert_eq!(fmt_power(16.0), "16 kW");
        assert_eq!(fmt_power(0.5), "500 W");
    }

    #[test]
    fn production_measures_work_instead_of_buffer_fills_or_duplicate_networks() {
        let mut f = grid(&[(0, Kind::Generator), (1, Kind::Battery), (2, Kind::Assembler)]);
        let generator = IVec3::ZERO;
        let machine = IVec3::new(2, 0, 0);
        f.nodes.get_mut(&generator).unwrap().input[13] = 64;
        f.nodes.get_mut(&generator).unwrap().mode = 2;
        // A second isolated cable also touches the generator. Its source
        // membership must not count another 12 kW of production.
        f.nodes.insert(IVec3::NEG_Y, Node::new(Kind::Cable));
        let initial: u64 = f.nodes.values().map(|n| n.energy as u64).sum();
        for _ in 0..4 { f.tick(0.25, 1); }
        let (s, _) = f.stats.window(false);
        assert_eq!(s.generated, 48);
        assert_eq!(s.drawn, 0); // no recipe; idle buffers still fill
        assert!(f.nodes[&machine].energy > 0);
        assert!(s.delivered > 0);
        let stored: u64 = f.nodes.values().map(|n| n.energy as u64).sum();
        assert_eq!(initial + s.generated, stored + s.drawn);
    }

    #[test]
    fn battery_tiers_are_one_four_sixteen_kwh() {
        assert_eq!(fmt_energy(battery_capacity(0) as f64), "1.00 kWh");
        assert_eq!(fmt_energy(battery_capacity(1) as f64), "4.00 kWh");
        assert_eq!(fmt_energy(battery_capacity(2) as f64), "16.00 kWh");
        assert_eq!(fmt_power(battery_rate(2) as f64), "64 kW");
    }

    #[test]
    fn energy_formats_in_watt_hours() {
        assert_eq!(fmt_energy(14.4), "1.0 Wh");
        assert_eq!(fmt_joules(400.0), "100 kJ");
    }
}
