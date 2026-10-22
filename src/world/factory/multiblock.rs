//! Railcraft-style brick multiblocks: the coke oven and the blast furnace.
//!
//! Each is a controller assembly (the oven door or the furnace tuyere) set in
//! the middle of one wall of a brick shell. Bricks are ordinary one-metre
//! assemblies, so a structure is checked against factory nodes and costs the
//! same at every voxel scale. Formation is recomputed only when the factory
//! topology changes. Pipes connect to the controller's exposed face.
//!
//! Neither structure uses electricity. They run on heat: the coke oven bakes
//! coal at about 1000 °C, and the blast furnace needs coke to reach the
//! 1500 °C that melts iron. Temperature is kept in `Node::burn` (°C).
use super::{CAPACITY, COKE, CREOSOTE, Factory, Kind, Node, recipes};
use glam::IVec3;
use std::collections::HashSet;

pub const AMBIENT_C: u32 = 20;
pub const COKE_OVEN_WORKING_C: u32 = 900;
pub const COKE_OVEN_MAX_C: u32 = 1050;
pub const BLAST_MAX_C: u32 = 1650;
/// Creosote recovered per coal coked (a quarter item).
pub const CREOSOTE_PER_BATCH: u32 = 16;

const HORIZONTAL: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Brick count a complete shell needs.
pub fn bricks_needed(kind: Kind) -> u16 {
    match kind {
        Kind::CokeOven => 26,
        Kind::BlastFurnace => 35,
        _ => 0,
    }
}
pub fn brick_kind(kind: Kind) -> Option<Kind> {
    match kind {
        Kind::CokeOven => Some(Kind::CokeBrick),
        Kind::BlastFurnace => Some(Kind::BlastBrick),
        _ => None,
    }
}

/// Cells of the shell when the controller at `p` faces outward along `-into`.
/// The coke oven is a 3 × 3 × 3 cube with the door in the middle of a wall;
/// the blast furnace is 3 wide, 4 tall and 3 deep with the tuyere on the
/// second course.
pub fn shell(kind: Kind, p: IVec3, into: IVec3, size: i32) -> Vec<IVec3> {
    let centre = p + into * size;
    let (low, high) = if kind == Kind::BlastFurnace { (-1, 2) } else { (-1, 1) };
    let mut cells = Vec::new();
    for y in low..=high {
        for z in -1..=1 {
            for x in -1..=1 {
                let c = centre + IVec3::new(x, y, z) * size;
                if c != p {
                    cells.push(c);
                }
            }
        }
    }
    cells
}

/// Seconds of the controller's working temperature target.
pub fn working_temperature(kind: Kind, mode: u32) -> u32 {
    match (kind, mode) {
        (Kind::CokeOven, _) => COKE_OVEN_WORKING_C,
        (Kind::BlastFurnace, 1) => 1400,
        _ => 1500,
    }
}

pub fn temperature(n: &Node) -> u32 {
    n.burn.max(AMBIENT_C)
}

impl Factory {
    /// Recompute every controller's shell. Bricks belong to at most one
    /// structure; controllers are visited in a fixed order so the claim is
    /// deterministic.
    pub(super) fn form_multiblocks(&mut self, size: i32) {
        let mut controllers: Vec<IVec3> = self
            .nodes
            .iter()
            .filter(|(_, n)| brick_kind(n.kind).is_some())
            .map(|(p, _)| *p)
            .collect();
        controllers.sort_by_key(|p| (p.x, p.y, p.z));
        let mut claimed: HashSet<IVec3> = HashSet::new();
        for p in controllers {
            let kind = self.nodes[&p].kind;
            let brick = brick_kind(kind).unwrap();
            let mut best = (0u16, IVec3::NEG_Z);
            for into in HORIZONTAL {
                let count = shell(kind, p, into, size)
                    .iter()
                    .filter(|c| {
                        !claimed.contains(*c)
                            && self.nodes.get(*c).is_some_and(|n| n.kind == brick)
                    })
                    .count() as u16;
                if count > best.0 {
                    best = (count, into);
                }
            }
            let needed = bricks_needed(kind);
            let formed = best.0 == needed;
            if formed {
                claimed.extend(shell(kind, p, best.1, size));
            }
            let n = self.nodes.get_mut(&p).unwrap();
            n.formed = formed;
            n.structure = (best.0, needed);
            n.facing = -best.1;
        }
        // Bricks show whether they are part of a formed shell.
        for (p, n) in self.nodes.iter_mut() {
            if matches!(n.kind, Kind::CokeBrick | Kind::BlastBrick) {
                n.formed = claimed.contains(p);
            }
        }
    }

    /// One 0.25 s tick of every coke oven and blast furnace.
    pub(super) fn run_multiblocks(&mut self) {
        for n in self.nodes.values_mut() {
            match n.kind {
                Kind::CokeOven => coke_oven(n),
                Kind::BlastFurnace => blast_furnace(n),
                _ => {}
            }
        }
    }
}

fn cool(n: &mut Node, by: u32) {
    n.burn = n.burn.saturating_sub(by).max(AMBIENT_C);
}

fn coke_oven(n: &mut Node) {
    if !n.formed {
        cool(n, 8);
        n.progress = 0;
        n.recipe = None;
        n.status = "Incomplete shell";
        return;
    }
    if !n.enabled {
        cool(n, 4);
        return;
    }
    let process = recipes::process(Kind::CokeOven, 0).unwrap();
    let (coal, need) = process.inputs[0];
    let charged = n.input[coal] >= need || n.recipe.is_some();
    // A charged oven heats itself by partially burning the coal's volatiles.
    if charged {
        n.burn = (temperature(n) + 15).min(COKE_OVEN_MAX_C);
    } else {
        cool(n, 3);
    }
    if !charged {
        n.status = "Needs coal";
        return;
    }
    if temperature(n) < COKE_OVEN_WORKING_C {
        n.status = "Heating";
        return;
    }
    let (out, amount) = process.output;
    if n.output[out] + amount > CAPACITY || n.output[CREOSOTE] + CREOSOTE_PER_BATCH > CAPACITY {
        n.status = "Output blocked";
        return;
    }
    if n.recipe.is_none() {
        n.input[coal] -= need;
        n.recipe = Some(0);
        n.progress = 0;
    }
    n.progress += 1;
    n.status = "Coking";
    if n.progress >= process.ticks {
        n.output[out] += amount;
        n.output[CREOSOTE] += CREOSOTE_PER_BATCH;
        n.progress = 0;
        n.recipe = None;
    }
}

fn blast_furnace(n: &mut Node) {
    if !n.formed {
        cool(n, 12);
        n.progress = 0;
        n.recipe = None;
        n.status = "Incomplete shell";
        return;
    }
    if !n.enabled {
        cool(n, 6);
        return;
    }
    // Coke is both fuel and reductant. A little burns every two seconds to
    // hold the hearth temperature; more is charged with each batch.
    let fuelled = n.input[COKE] > 0;
    if fuelled {
        n.burn = (temperature(n) + 25).min(BLAST_MAX_C);
        n.work_clock += 0.25;
        if n.work_clock >= 2.0 {
            n.work_clock -= 2.0;
            n.input[COKE] -= 1;
        }
    } else {
        cool(n, 10);
    }
    let process = recipes::process(Kind::BlastFurnace, n.mode as usize).unwrap();
    if temperature(n) < working_temperature(Kind::BlastFurnace, n.mode) {
        n.status = if fuelled { "Heating" } else { "Needs coke" };
        if !fuelled {
            n.progress = 0;
            n.recipe = None;
        }
        return;
    }
    let (out, amount) = process.output;
    if n.output[out] + amount > CAPACITY {
        n.status = "Output blocked";
        return;
    }
    if n.recipe.is_none() {
        if !process.inputs.iter().all(|&(i, k)| n.input[i] >= k) {
            n.status = "Needs charge";
            return;
        }
        for &(i, k) in &process.inputs {
            n.input[i] -= k;
        }
        n.recipe = Some(n.mode as usize);
        n.progress = 0;
    }
    n.progress += 1;
    n.status = "Smelting";
    if n.progress >= process.ticks {
        n.output[out] += amount;
        n.progress = 0;
        n.recipe = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(kind: Kind, into: IVec3) -> (Factory, IVec3) {
        let mut f = Factory::default();
        let p = IVec3::new(0, 10, 0);
        f.nodes.insert(p, Node::new(kind));
        for c in shell(kind, p, into, 1) {
            f.nodes.insert(c, Node::new(brick_kind(kind).unwrap()));
        }
        (f, p)
    }

    #[test]
    fn complete_shells_form_in_any_orientation() {
        for into in HORIZONTAL {
            for kind in [Kind::CokeOven, Kind::BlastFurnace] {
                let (mut f, p) = build(kind, into);
                f.form_multiblocks(1);
                assert!(f.nodes[&p].formed, "{kind:?} {into}");
                assert_eq!(f.nodes[&p].facing, -into);
            }
        }
    }

    #[test]
    fn a_missing_brick_breaks_the_shell() {
        let (mut f, p) = build(Kind::CokeOven, IVec3::X);
        let gone = shell(Kind::CokeOven, p, IVec3::X, 1)[5];
        f.nodes.remove(&gone);
        f.form_multiblocks(1);
        assert!(!f.nodes[&p].formed);
        assert_eq!(f.nodes[&p].structure, (25, 26));
    }

    #[test]
    fn coke_oven_turns_coal_into_coke_and_creosote() {
        let (mut f, p) = build(Kind::CokeOven, IVec3::Z);
        f.form_multiblocks(1);
        f.nodes.get_mut(&p).unwrap().input[3] = 128;
        for _ in 0..400 {
            f.run_multiblocks();
        }
        let n = &f.nodes[&p];
        assert_eq!(n.output[COKE], 128);
        assert_eq!(n.output[CREOSOTE], 2 * CREOSOTE_PER_BATCH);
        assert_eq!(n.input[3], 0);
    }

    #[test]
    fn blast_furnace_needs_heat_before_it_smelts_iron() {
        let (mut f, p) = build(Kind::BlastFurnace, IVec3::NEG_X);
        f.form_multiblocks(1);
        let n = f.nodes.get_mut(&p).unwrap();
        n.input[2] = 128;
        n.input[COKE] = 128;
        f.run_multiblocks();
        assert_eq!(f.nodes[&p].status, "Heating");
        for _ in 0..600 {
            f.run_multiblocks();
        }
        let n = &f.nodes[&p];
        assert!(n.output[8] >= 192, "iron out {}", n.output[8]);
        assert!(temperature(n) >= 1500);
    }
}
