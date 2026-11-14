//! Diagonal steps require clearance through both adjacent corners.
use super::{B, IVec3, World, known};
use engine_core::fast_hash::FastMap;
mod search;
pub(super) use search::{Pending, Route, route, service};

#[derive(Clone, Copy)]
struct Footing { stand: bool, paved: bool }

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp { version: Option<u64>, resident: bool }

fn stamp(world: &World, at: IVec3) -> Stamp {
    let coord = crate::world::chunk_coord_from_world_block(at);
    Stamp { version: world.regions.source_version(coord), resident: world.chunks.contains_key(&coord) }
}

#[derive(Clone, Copy)]
struct Cached {
    world: u64,
    cfg: engine_voxel::WorldSampleConfig,
    at: IVec3,
    floor: Stamp,
    head: Stamp,
    footing: Footing,
}

// Direct mapping bounds both lookup work and storage, including during edit storms.
// Cells are revalidated against their own chunks, so a distant edit does not flush
// every resident's navigation. Cache identity never affects path tie breaking.
const CACHE_CELLS: usize = 8192;
struct Shared {
    cells: Box<[Option<Cached>]>,
    hits: u64,
    misses: u64,
}

impl Default for Shared {
    fn default() -> Self {
        Self { cells: vec![None; CACHE_CELLS].into_boxed_slice(), hits: 0, misses: 0 }
    }
}

thread_local! {
    static SHARED: std::cell::RefCell<Shared> = std::cell::RefCell::new(Shared::default());
}

pub(super) fn cache_counts() -> (u64, u64) {
    SHARED.with(|cache| { let cache = cache.borrow(); (cache.hits, cache.misses) })
}

/// Reuses immutable footing checks within one bounded path search.
pub(super) struct Query<'a> {
    world: &'a World,
    scale: f32,
    cells: FastMap<IVec3, Footing>,
    missing: Vec<IVec3>,
}

impl<'a> Query<'a> {
    pub(super) fn new(world: &'a World, scale: f32) -> Self {
        Self { world, scale, cells: FastMap::default(), missing: Vec::new() }
    }

    fn footing(&mut self, at: IVec3) -> Footing {
        let w = self.world;
        let height = (1.8 / self.scale).ceil() as i32;
        for cell in (-1..height).step_by(crate::world::CHUNK_SIZE as usize)
            .map(|dy|at+IVec3::Y*dy).chain([at+IVec3::Y*(height-1)]) {
            if w.resident_block(cell).is_none() { self.missing.push(cell); }
        }
        *self.cells.entry(at).or_insert_with(|| {
            let floor_stamp = stamp(w, at - IVec3::Y);
            let head_stamp = stamp(w, at + IVec3::Y * (height - 1));
            let slot = engine_core::hash::hash_u32(at.x as u32
                ^ (at.y as u32).wrapping_mul(0x9e37_79b9)
                ^ (at.z as u32).wrapping_mul(0x85eb_ca6b)) as usize % CACHE_CELLS;
            SHARED.with(|shared| {
                let mut shared = shared.borrow_mut();
                if let Some(cached) = shared.cells[slot]
                    && height <= crate::world::CHUNK_SIZE
                    && cached.world == w.regions.instance_id && cached.cfg == w.sample_config()
                    && cached.at == at && cached.floor == floor_stamp && cached.head == head_stamp {
                    shared.hits = shared.hits.saturating_add(1);
                    return cached.footing;
                }
                let floor = known(w, at - IVec3::Y);
                let footing = Footing {
                    stand: floor.is_some_and(B::is_solid)
                        && (0..height).all(|dy| known(w, at + IVec3::Y * dy) == Some(B::Air)),
                    paved: floor == Some(B::Cobblestone),
                };
                shared.misses = shared.misses.saturating_add(1);
                shared.cells[slot] = Some(Cached { world: w.regions.instance_id, cfg: w.sample_config(),
                    at, floor: floor_stamp, head: head_stamp, footing });
                footing
            })
        })
    }

    pub(super) fn stand(&mut self, at: IVec3) -> bool { self.footing(at).stand }
    pub(super) fn paved(&mut self, at: IVec3) -> bool { self.footing(at).paved }

    pub(super) fn diagonal_clear(&mut self, from: IVec3, to: IVec3) -> bool {
        from.y == to.y
            && self.stand(IVec3::new(to.x, from.y, from.z))
            && self.stand(IVec3::new(from.x, from.y, to.z))
            && diagonal_body_clear(self.world, from, to, self.scale)
    }
}

pub(super) const DIRECTIONS: [IVec3; 8] = [IVec3::X, IVec3::NEG_X,
    IVec3::Z, IVec3::NEG_Z, IVec3::new(1,0,1), IVec3::new(-1,0,1),
    IVec3::new(1,0,-1), IVec3::new(-1,0,-1)];

pub(super) fn diagonal_clear(w:&World,from:IVec3,to:IVec3,v:f32)->bool {
    if from.y!=to.y {return false;}
    if !super::stand(w,IVec3::new(to.x,from.y,from.z),v)
        || !super::stand(w,IVec3::new(from.x,from.y,to.z),v) {return false;}
    diagonal_body_clear(w, from, to, v)
}

fn diagonal_body_clear(w:&World,from:IVec3,to:IVec3,v:f32)->bool {
    // A conservative swept body also covers fine voxels wider than one cell.
    let a=super::metres(from,v);let b=super::metres(to,v);
    let lo=((a.min(b)-super::Vec3::new(0.4,0.0,0.4))/v).floor().as_ivec3();
    let hi=((a.max(b)+super::Vec3::new(0.4,1.8,0.4)-super::Vec3::splat(0.001))/v).floor().as_ivec3();
    (lo.x..=hi.x).all(|x|(lo.y..=hi.y).all(|y|(lo.z..=hi.z)
        .all(|z|known(w,IVec3::new(x,y,z))==Some(B::Air))))
}

pub(super) fn work_point(w:&World,at:IVec3,v:f32)->Option<super::Vec3> {
    let mut point=super::metres(at,v);
    let below=at-IVec3::Y;
    let block=known(w,below)?;
    if !engine_content::shaped::is_shaped(block) {return Some(point);}
    let centre=point/v;
    let radius=0.4/v;
    let links=engine_content::shaped::links(block,below,|p|known(w,p).unwrap_or(B::Air));
    let top=engine_content::shaped::boxes(block,below,links).into_iter()
        .filter(|(lo,hi)|centre.x+radius>lo.x&&centre.x-radius<hi.x
            &&centre.z+radius>lo.z&&centre.z-radius<hi.z)
        .map(|(_,hi)|hi.y).max_by(f32::total_cmp)?;
    point.y=top*v;
    let lo=point/v-super::Vec3::new(radius,0.0,radius);
    let hi=point/v+super::Vec3::new(radius,1.8/v,radius);
    let first=crate::world::chunk_coord_from_world_block(lo.floor().as_ivec3());
    let last=crate::world::chunk_coord_from_world_block(hi.floor().as_ivec3());
    if !(first.x..=last.x).all(|x|(first.y..=last.y).all(|y|(first.z..=last.z)
        .all(|z|w.chunks.contains_key(&IVec3::new(x,y,z))))) {return None;}
    (!crate::world::collision::collides_world(w,lo,hi)).then_some(point)
}
