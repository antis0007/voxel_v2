//! Construction exclusion for the local player's base.
use crate::world::{BlockId, World};
use crate::world::inventory::{ArmorTier, EquipSlot, Item};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

const MAX_CLAIMS: usize = 32;
const CLAIM_WIDTH_M: f32 = 128.0;
const CLAIM_RISE_M: f32 = 64.0;
const CLAIM_ABOVE_M: f32 = 24.0;
const CLAIM_BELOW_M: f32 = 8.0;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Bounds {
    min: [i32; 3],
    max: [i32; 3],
}

impl Bounds {
    pub(crate) fn new(a: IVec3, b: IVec3) -> Self {
        Self {
            min: a.min(b).to_array(),
            max: a.max(b).to_array(),
        }
    }

    pub(crate) fn valid(self) -> bool {
        self.min.iter().zip(self.max).all(|(&lo, hi)| {
            lo <= hi && lo.unsigned_abs().max(hi.unsigned_abs()) < (1 << 28)
        })
    }

    fn overlaps(self, min: IVec3, max: IVec3) -> bool {
        IVec3::from(self.min).cmple(max).all() && IVec3::from(self.max).cmpge(min).all()
    }

    pub(crate) fn size(self) -> IVec3 {
        IVec3::from(self.max) - IVec3::from(self.min) + IVec3::ONE
    }

    fn edges(self) -> (Vec3, Vec3) {
        (IVec3::from(self.min).as_vec3(), (IVec3::from(self.max) + IVec3::ONE).as_vec3())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct State {
    pub(crate) bounds: Option<Bounds>,
    pub(crate) first_corner: Option<IVec3>,
    pub(super) stakes: Stakes,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Claim([[i32; 3]; 2]);

impl Claim {
    fn corners(self) -> [IVec3; 2] {
        self.0.map(IVec3::from)
    }

    fn bounds(self, scale: f32) -> Bounds {
        let [a, b] = self.corners();
        Bounds::new(
            a.min(b) - IVec3::Y * (CLAIM_BELOW_M / scale).ceil() as i32,
            a.max(b) + IVec3::Y * (CLAIM_ABOVE_M / scale).ceil() as i32,
        )
    }

    fn active(self, world: &World) -> bool {
        self.corners().into_iter().all(|cell| world.get(cell) == BlockId::ClaimStake)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct Stakes {
    claims: Vec<Claim>,
    pending: Option<[i32; 3]>,
}

impl Stakes {
    pub(super) fn used(&self) -> bool {
        self.pending.is_some() || !self.claims.is_empty()
    }

    pub(super) fn valid(&self) -> bool {
        let mut cells = std::collections::HashSet::new();
        self.claims.len() <= MAX_CLAIMS
            && self.claims.iter().all(|claim| {
                let [a, b] = claim.corners();
                Bounds::new(a, b).valid() && cells.insert(a) && cells.insert(b)
            })
            && self.pending.is_none_or(|p| {
                let p = IVec3::from(p);
                Bounds::new(p, p).valid() && cells.insert(p)
            })
    }
}

impl State {
    pub(crate) fn remove_stake(&mut self, cell: IVec3) {
        let key = cell.to_array();
        self.stakes.claims.retain(|claim| !claim.0.contains(&key));
        if self.stakes.pending == Some(key) {
            self.stakes.pending = None;
        }
    }
}

pub(crate) fn enabled(world: &World) -> bool {
    #[cfg(feature = "multiplayer")]
    if world.multiplayer.is_some() {
        return false;
    }
    world.ca_view.is_none()
}

pub(crate) fn stake_action(world: &World, cell: IVec3) -> &'static str {
    let key = cell.to_array();
    let stakes = &world.farming.base.stakes;
    if stakes.claims.iter().any(|claim| claim.0.contains(&key)) {
        return "[Sneak]+[F] Release Claim";
    }
    match stakes.pending {
        Some(p) if p == key => "[F] Cancel Corner",
        Some(_) => "[F] Confirm Claim",
        None => "[F] Mark Corner",
    }
}

pub(crate) fn use_stake(world: &mut World, cell: IVec3, sneak: bool) {
    let key = cell.to_array();
    let scale = world.voxel_scale.meters();
    let mut stakes = std::mem::take(&mut world.farming.base.stakes);
    stakes.claims.retain(|claim| claim.active(world));
    let message = if let Some(i) = stakes.claims.iter().position(|claim| claim.0.contains(&key)) {
        if sneak {
            stakes.claims.remove(i);
            "Claim Released"
        } else {
            "Claimed"
        }
    } else if stakes.pending == Some(key) {
        stakes.pending = None;
        "Corner Cleared"
    } else if let Some(first) = stakes.pending.filter(|&p| !sneak && world.get(p.into()) == BlockId::ClaimStake) {
        let origin = IVec3::from(first);
        let span = (cell.as_vec3() - origin.as_vec3()).abs() * scale;
        if !Bounds::new(origin, cell).valid()
            || span.x.max(span.z) > CLAIM_WIDTH_M || span.y > CLAIM_RISE_M
        {
            "Claim Too Large"
        } else if stakes.claims.len() >= MAX_CLAIMS {
            "Claim Limit"
        } else {
            stakes.claims.push(Claim([first, key]));
            stakes.pending = None;
            "Area Claimed"
        }
    } else if Bounds::new(cell, cell).valid() {
        stakes.pending = Some(key);
        "Corner Marked"
    } else {
        "Invalid Corner"
    };
    world.farming.base.stakes = stakes;
    world.expedition.say(message);
}

pub(crate) fn overlaps(world: &World, min: IVec3, max: IVec3) -> bool {
    if world.farming.base.bounds.is_some_and(|bounds| bounds.overlaps(min, max)) {
        return true;
    }
    let scale = world.voxel_scale.meters();
    if world.farming.base.stakes.claims.iter().any(|claim| {
        claim.bounds(scale).overlaps(min, max) && claim.active(world)
    }) {
        return true;
    }
    bed_bounds(world).is_some_and(|(bed, bounds)| {
        bounds.overlaps(min, max) && world.get(bed) == BlockId::Bed
    })
}

fn bed_bounds(world: &World) -> Option<(IVec3, Bounds)> {
    let spawn = world.adventure.realm_spawn();
    let scale = world.voxel_scale.meters();
    // Older saves record an overworld bed's position without a separate bed binding.
    let bed = spawn.binding.bed.or_else(|| {
        spawn.point.filter(|point| point.is_finite()).map(|point| {
            let cell = point / scale;
            IVec3::new(cell.x.floor() as i32, cell.y.round() as i32, cell.z.floor() as i32)
        })
    });
    let bed = bed.filter(|p| Bounds::new(*p, *p).valid())?;
    const BED_HORIZONTAL_METRES: f32 = 12.0;
    const BED_VERTICAL_METRES: f32 = 8.0;
    let horizontal = (BED_HORIZONTAL_METRES / scale).ceil() as i32;
    let vertical = (BED_VERTICAL_METRES / scale).ceil() as i32;
    let reach = IVec3::new(horizontal, vertical, horizontal);
    Some((bed, Bounds::new(bed - reach, bed + reach)))
}

pub(crate) fn overlay(world: &World, eye: Vec3, out: &mut Vec<crate::mesh::LineVertex>) {
    if !enabled(world) || !world.inventory.equipment[EquipSlot::Head as usize]
        .is_some_and(|s| s.item == Item::Armor(EquipSlot::Head, ArmorTier::Survey))
    {
        return;
    }
    let scale = world.voxel_scale.meters();
    let near = |bounds: Bounds| {
        let (min, max) = bounds.edges();
        (eye.clamp(min, max) - eye).length_squared() * scale * scale <= 96.0 * 96.0
    };
    let mut draw = |bounds: Bounds, color: [f32; 4]| {
        let (min, max) = bounds.edges();
        for p in crate::creative::blueprint::box_edges(min, max).into_iter().flatten() {
            let p = p + (eye - p).normalize_or_zero() * (0.01 / scale);
            out.push(crate::mesh::LineVertex { pos: p.to_array(), color });
        }
    };
    let state = &world.farming.base;
    if let Some(bounds) = state.bounds.filter(|&b| near(b)) {
        draw(bounds, [0.30, 0.88, 0.72, 0.85]);
    }
    for claim in &state.stakes.claims {
        let bounds = claim.bounds(scale);
        if near(bounds) && claim.active(world) {
            draw(bounds, [0.30, 0.88, 0.72, 0.75]);
            let [a, b] = claim.corners();
            let y = a.y.min(b.y);
            draw(Bounds::new(a.with_y(y), b.with_y(y)), [0.30, 0.88, 0.72, 0.95]);
        }
    }
    if let Some((bed, bounds)) = bed_bounds(world)
        && near(bounds) && world.get(bed) == BlockId::Bed
    {
        draw(bounds, [0.47, 0.65, 1.0, 0.6]);
    }
    if let Some(cell) = state.stakes.pending.map(IVec3::from) {
        let bounds = Bounds::new(cell, cell);
        if near(bounds) && world.get(cell) == BlockId::ClaimStake {
            draw(bounds, [1.0, 0.73, 0.31, 0.9]);
        }
    }
}
