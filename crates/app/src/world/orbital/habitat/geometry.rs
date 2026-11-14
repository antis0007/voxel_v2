use super::*;
use engine_worldgen::station::{self, Plan};

const LEGACY_MIN: IVec3 = IVec3::new(2, -3, -9);
const LEGACY_SIZE: IVec3 = IVec3::new(29, 18, 18);
pub const LEGACY_CELLS: usize = 29 * 18 * 18;

/// One-metre gas cells of a habitat, in metres local to its origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub min: IVec3,
    pub size: IVec3,
}

impl Default for Grid {
    fn default() -> Self { Self { min: LEGACY_MIN, size: LEGACY_SIZE } }
}

impl Grid {
    /// The drum box grown to hold `plan` with one vacuum cell around it.
    fn with_wing(plan: &Plan) -> Self {
        let min = LEGACY_MIN.min(wing_origin() + plan.min - IVec3::ONE);
        let max = (LEGACY_MIN + LEGACY_SIZE).max(wing_origin() + plan.max + IVec3::splat(2));
        Self { min, size: max - min }
    }
    pub fn cells(&self) -> usize { (self.size.x * self.size.y * self.size.z) as usize }
    pub fn valid(&self) -> bool {
        self.size.cmpge(IVec3::ONE).all() && self.size.max_element() <= 192
            && self.min.abs().max_element() <= 256 && self.cells() <= 160_000
    }
    pub fn index(&self, p: IVec3) -> Option<usize> {
        let p = p - self.min;
        (p.cmpge(IVec3::ZERO).all() && p.cmplt(self.size).all())
            .then(|| (p.x + self.size.x * (p.z + self.size.z * p.y)) as usize)
    }
    pub fn local(&self, i: usize) -> IVec3 {
        let i = i as i32;
        self.min + IVec3::new(i % self.size.x, i / (self.size.x * self.size.z),
            (i / self.size.x) % self.size.z)
    }
}

/// Habitat-local metre cell of the wing plan's origin: flush past the endcap.
pub(super) fn wing_origin() -> IVec3 {
    IVec3::new(11 + config().length_m + config().shell_m, 0, 0)
}

fn wing_seed(world_seed: u32, cell: glam::IVec2) -> u32 {
    engine_core::hash::hash_u32(world_seed ^ engine_core::hash::hash_u32(
        (cell.x as u32).wrapping_mul(0x9e37_79b9) ^ (cell.y as u32).wrapping_mul(0x85eb_ca6b)))
}

fn legacy_box(local: Vec3) -> bool {
    local.cmpge(LEGACY_MIN.as_vec3()).all() && local.cmplt((LEGACY_MIN + LEGACY_SIZE).as_vec3()).all()
}

/// Grows a drum-only habitat saved before the wing: same gas, bigger grid, wing built next.
pub(super) fn retrofit(world_seed: u32, h: &mut Habitat) {
    if h.wing.is_some() || !h.ready || h.gas.len() != h.grid.cells() { return; }
    let seed = wing_seed(world_seed, h.cell);
    let grid = Grid::with_wing(&station::plan(seed));
    let mut gas = vec![[-1.0; 2]; grid.cells()];
    for (i, q) in h.gas.iter().enumerate() {
        if let Some(j) = grid.index(h.grid.local(i)) { gas[j] = *q; }
    }
    h.gas = gas;
    h.grid = grid;
    h.wing = Some(seed);
    h.ready = false;
    h.retrofit = true;
    h.build_cursor = 0;
    h.volumes.clear();
    h.faces.clear();
    h.dirty.clear();
}

/// Walk-through cut in the drum's far endcap, lined up with the wing hatch.
fn wing_doorway(p: Vec3) -> bool {
    let end = 11.0 + config().length_m as f32;
    (end..end + config().shell_m as f32).contains(&p.x)
        && (0.0..station::HEIGHT as f32).contains(&p.y) && (-1.0..1.0).contains(&p.z)
}

/// Whether a wing gas cell starts at station pressure.
pub(super) fn wing_pressurised(plan: &Plan, cell: IVec3) -> bool {
    plan.pressurised(cell - wing_origin()) || wing_doorway(cell.as_vec3() + Vec3::splat(0.5))
}

impl Habitat {
    pub fn index(&self, p: IVec3) -> Option<usize> { self.grid.index(p) }
    pub fn near(&self, p: Vec3) -> bool {
        p.cmpge(self.grid.min.as_vec3()).all()
            && p.cmplt((self.grid.min + self.grid.size).as_vec3()).all()
    }
    pub fn center(&self) -> Vec3 { self.origin.as_vec3() + Vec3::Y * config().radius_m as f32 }
}

pub fn register(world: &mut World, cell: glam::IVec2, crown: Vec3, berth: Vec3) -> bool {
    if world.orbital.habitats.habitats.iter().any(|h| h.cell == cell) { return true; }
    if world.orbital.habitats.habitats.len() >= 16 { return false; }
    let tank = config().tank_pressure_pa * config().tank_volume_m3;
    let seed = wing_seed(world.seed, cell);
    world.orbital.habitats.habitats.push(Habitat {
        grid: Grid::with_wing(&station::plan(seed)), wing: Some(seed),
        residents: Vec::new(), populated: false, retrofit: false,
        origin: crown.round().as_ivec3(), berth, cell, build_cursor: 0, ready: false,
        gas: Vec::new(), tank: [tank * 0.21, tank * 0.79],
        cabin: [config().pressure_pa * cabin_volume() * 0.21,
            config().pressure_pa * cabin_volume() * 0.79], escaped: [0.0; 2],
        cycle: -1, doors_open: [false; 2], volumes: Vec::new(), faces: Vec::new(), dirty: BTreeSet::new(),
    });
    true
}

pub fn allows_cell(world: &World, cell: IVec3) -> bool {
    let m = (cell.as_vec3() * world.voxel_scale.meters()).floor().as_ivec3();
    world.orbital.habitats.habitats.iter().any(|h| h.index(m - h.origin).is_some())
        || world.orbital.habitats.building.is_some_and(|(o, grid)| grid.index(m - o).is_some())
}

pub fn protects(world: &World, cell: IVec3) -> bool { allows_cell(world, cell) }

pub fn acceleration(world: &World, position: Vec3) -> Option<Vec3> {
    world.orbital.habitats.habitats.iter().find_map(|h| {
        let p = position - h.origin.as_vec3();
        if !h.ready || !h.near(p) { return None; }
        // The wing is tether-fixed like the entrance passage; only the drum spins.
        let wing = h.wing.is_some() && p.x >= 11.0 + config().length_m as f32;
        if p.x < 11.0 || wing { return Some(Vec3::NEG_Y * 9.81); }
        let radial = position - h.center();
        // The axis is X; retaining momentum is the cylinder controller's job.
        Some(Vec3::new(0.0, radial.y, radial.z) * (9.81 / config().radius_m as f32))
    })
}

pub(super) fn material(wing: Option<&Plan>, p: Vec3) -> B {
    if let Some(b) = super::super::cargo::security::material(p) { return b; }
    if let Some(plan) = wing {
        if wing_doorway(p) { return B::Air; }
        if let Some(b) = station::sealed_block(plan, p.floor().as_ivec3() - wing_origin()) {
            return b;
        }
    }
    let c = config();
    let r = c.radius_m as f32;
    let end = 11.0 + c.length_m as f32;
    let radial = (glam::Vec2::new(p.y - r, p.z)).length();
    let cylinder = (9.0..end + c.shell_m as f32).contains(&p.x)
        && radial < r + c.shell_m as f32;
    let hollow = (11.0..end).contains(&p.x) && radial < r;
    let passage = (THROAT_X_M..11.0).contains(&p.x) && (0.0..3.0).contains(&p.y) && p.z.abs() < 1.5;
    let tunnel = (THROAT_X_M..11.0).contains(&p.x) && (-1.0..4.0).contains(&p.y) && p.z.abs() < 2.5;
    let hatch = HATCH_X.contains(&(p.x.floor() as i32));
    if passage {
        if hatch { return B::SteelBlock; }
        return B::Air;
    }
    if hollow { return B::Air; }
    if cylinder || tunnel {
        if hatch && (0.0..4.0).contains(&p.y) && p.z.abs() < 2.5 {
            return B::CopperBlock;
        }
        if p.y > 11.0 && (p.x.floor() as i32).rem_euclid(4) == 0
            && radial < r + 1.0 && p.z.abs() < 1.0 { return B::GlowCap; }
        return B::SteelBlock;
    }
    B::Air
}

pub(super) fn build(world: &mut World, h: &mut Habitat) {
    if h.ready { return; }
    let scale = world.voxel_scale.meters();
    let n = scale.recip().round() as i32;
    let size = h.grid.size * n;
    let plan = h.wing.map(station::plan);
    let retrofit = h.retrofit;
    let total = (size.x * size.y * size.z) as usize;
    let end = (h.build_cursor + 2048).min(total);
    let base = (h.origin + h.grid.min) * n;
    let mut edits = Vec::new();
    for i in h.build_cursor..end {
        let p = IVec3::new(i as i32 % size.x, i as i32 / (size.x * size.z),
            (i as i32 / size.x) % size.z);
        let local = (p.as_vec3() + Vec3::splat(0.5)) * scale + h.grid.min.as_vec3();
        let cell = base + p;
        if retrofit {
            // The drum is already built and may carry the player's repairs; only the
            // endcap doorway and cells outside it are written, never over existing blocks.
            if wing_doorway(local) {
                if world.get(cell) != B::Air { edits.push((cell, B::Air)); }
                continue;
            }
            let drum = legacy_box(local) && local.x < wing_origin().x as f32;
            if drum || world.get(cell) != B::Air { continue; }
        }
        let block = material(plan.as_ref(), local);
        if block != B::Air { edits.push((cell, block)); }
    }
    world.set_many_mechanism(&edits);
    // A rejected write must not become a permanently missing hull cell.
    if edits.iter().any(|&(p, b)| world.get(p) != b) { return; }
    h.build_cursor = end;
    if end == total {
        h.ready = true;
        if h.retrofit { h.retrofit = false; } else { h.gas = vec![[-1.0; 2]; h.grid.cells()]; }
        h.dirty.extend(0..h.grid.cells());
    }
}
