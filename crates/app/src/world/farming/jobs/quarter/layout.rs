//! Bounded parcel and street planning for the opt-in construction quarter.
use super::{geometry, traditions, IVec3, Lot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
mod route;
pub(crate) mod infill;
pub(super) mod district;
pub(super) mod district_checks;
pub(super) mod checks;
pub(super) mod scene;

pub const X: i32 = 46;
pub const Z: i32 = 32;
pub const STOCKPILE: IVec3 = IVec3::new(3, 1, 11);
pub type Point = [i32; 2];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub diagonal_cost: u32,
    pub bend_cost: u32,
    pub terrain_cost: u32,
    pub frontage_jitter: i32,
    pub setback_jitter: i32,
}

pub fn profiles() -> Result<Vec<Profile>, String> {
    let rows: Vec<Profile> = ron::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/city_layouts.ron"))).map_err(|e| e.to_string())?;
    if rows.len() != 3 || rows.iter().any(|p| p.diagonal_cost > 100 || p.bend_cost > 100
        || p.terrain_cost > 100 || !(0..=5).contains(&p.frontage_jitter) || !(0..=4).contains(&p.setback_jitter)) {
        return Err("Invalid street planning profile".into());
    }
    Ok(rows)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parcel {
    pub origin: Point,
    pub turn: bool,
    pub tradition: usize,
    pub complexity: u8,
    pub variant: u32,
    #[serde(default, skip_serializing_if = "super::is_false")]
    pub roof_access: bool,
}

impl Parcel {
    pub fn lot(&self, base: IVec3, paid: bool) -> Lot {
        Lot { origin: (base + IVec3::new(self.origin[0], 1, self.origin[1])).to_array(),
            tradition: self.tradition, complexity: self.complexity, turn: self.turn,
            layer: 0, occupied: !paid, market_cover: false, frontage: paid, variant: self.variant, roof_access: self.roof_access }
    }
    pub fn door(&self) -> Point {
        let s = if self.turn { -1 } else { 1 };
        [self.origin[0] + 4 * s, self.origin[1] - 4 * s]
    }
    pub fn bounds(&self) -> [i32; 4] {
        let (x,z) = (self.origin[0], self.origin[1]);
        if self.turn { [x - 11, z - 16, x + 2, z + 3] } else { [x - 2, z - 3, x + 11, z + 16] }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub version: u8,
    pub profile: u8,
    pub seed: u32,
    pub parcels: Vec<Parcel>,
    pub roads: Vec<Vec<Point>>,
    pub expansions: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design: Option<district::Design>,
}

impl Plan {
    pub fn valid(&self) -> bool {
        (self.version == 1 && self.design.is_none() || self.version == 2 && self.design.is_some()) && self.profile < 3 && self.parcels.len() == 6
            && self.roads.len() == 9 && self.expansions <= (if self.version == 2 {380_000}else{180_000})
            && self.parcels.iter().all(|p| p.tradition < 3 && (1..=3).contains(&p.complexity)
                && inside(p.origin) && (self.version==2 || !p.roof_access))
            && self.roads.iter().all(|r| !r.is_empty() && r.len() <= 300 && r.iter().all(|&p| inside(p))
                && r.windows(2).all(|s| (s[0][0]-s[1][0]).abs().max((s[0][1]-s[1][1]).abs()) == 1))
            && self.design.as_ref().is_none_or(|_|district::valid(self))
    }
    pub fn diagonal_steps(&self) -> usize {
        self.roads.iter().flat_map(|r| r.windows(2)).filter(|s| s[0][0] != s[1][0] && s[0][1] != s[1][1]).count()
    }
    pub fn paving(&self) -> BTreeSet<Point> {
        if self.design.is_some() {return district::paving(self);}
        let mut out = BTreeSet::new();
        for (i, road) in self.roads.iter().enumerate() {
            let radius = if i == 0 { 2 } else { 1 };
            for &[x,z] in road { for dx in -radius..=radius { for dz in -radius..=radius { out.insert([x+dx,z+dz]); } } }
        }
        out
    }
}

pub fn inside([x,z]: Point) -> bool { x > -X && x < X && z > -Z && z < Z }

pub fn seed(world_seed: u32) -> u32 {
    std::env::var("VOXEL_CITY_LAYOUT_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(world_seed)
}

pub fn selected() -> Result<Option<u8>, String> {
    match std::env::var("VOXEL_CITY_LAYOUT").ok().as_deref() {
        None | Some("") | Some("Legacy") => Ok(None),
        Some("Ordered") => Ok(Some(0)), Some("Mixed") => Ok(Some(1)), Some("Organic") => Ok(Some(2)),
        _ => Err("Unknown city layout profile".into()),
    }
}

/// Elevation and exclusions come from the surveyed scene, in metres.
pub fn generate(seed: u32, profile: u8, style: usize, complexity: u8,
    height: &impl Fn(Point) -> Option<i32>) -> Result<Plan, String> {
    let rows = profiles()?;
    let p = rows.get(profile as usize).ok_or("Unknown planning profile")?;
    let styles=traditions()?;
    let mut parcels: Vec<Parcel> = Vec::new();
    let anchors = [(9,8,false),(-20,8,false),(-37,8,false),(-8,-8,true),(17,-8,true),(38,-8,true)];
    for (slot, (x,z,turn)) in anchors.into_iter().enumerate() {
        let mut chosen = None;
        for attempt in 0..64u32 {
            let h = engine_core::hash::hash_u32(seed ^ (slot as u32 * 7919) ^ attempt.wrapping_mul(104729));
            let jitter = |salt: u32, radius: i32| (engine_core::hash::hash_u32(h ^ salt) % (2*radius+1) as u32) as i32 - radius;
            let q = Parcel { origin: [x+jitter(19,p.frontage_jitter),z+jitter(43,p.setback_jitter)], turn,
                tradition: if slot == 0 { style } else { (style + slot % 2) % 3 },
                complexity: if slot == 0 { complexity } else { 1 + (h % 3) as u8 }, variant:h, roof_access:false };
            let b = q.bounds();
            let fits = b[0] > -X+1 && b[2] < X-1 && b[1] > -Z+1 && b[3] < Z-1
                && (b[2] < -3 || b[0] > 3)
                && !parcels.iter().any(|old| overlaps(b,old.bounds()))
                && (b[0]..=b[2]).all(|x| (b[1]..=b[3]).all(|z| height([x,z]) == Some(0)));
            let supplied = slot != 0 || {
                let lot=q.lot(IVec3::ZERO,true);
                geometry::plan(&lot,&styles[lot.tradition]).iter().all(|&(cell,_)|
                    (geometry::at(&lot,cell)-STOCKPILE).as_vec3().length_squared() <= super::super::RANGE.powi(2))
            };
            if fits && supplied { chosen=Some(q); break; }
        }
        parcels.push(chosen.ok_or_else(|| format!("No level parcel for slot {slot}"))?);
    }
    let mut occupied=BTreeSet::from([[STOCKPILE.x,STOCKPILE.z]]);
    for parcel in &parcels {
        let lot=parcel.lot(IVec3::ZERO,false);
        for (cell,_) in geometry::plan(&lot,&styles[lot.tradition]) {
            let at=geometry::at(&lot,cell); occupied.insert([at.x,at.z]);
        }
    }
    let clear = |[x,z]: Point| inside([x,z]) && height([x,z]).is_some_and(|h|
        (-1..=1).all(|dx| (-1..=1).all(|dz| !occupied.contains(&[x+dx,z+dz]) && height([x+dx,z+dz]) == Some(h))));
    let mut roads=vec![(-28..=28).map(|z|[0,z]).collect()];
    let mut expansions=0;
    let mut links: Vec<_> = parcels.iter().map(|q|(q.door(),[0,0])).collect();
    links.extend([(parcels[2].door(),parcels[1].door()),(parcels[5].door(),parcels[3].door())]);
    for (a,b) in links {
        let (road, count)=route::find(a,b,p,height,&clear)?;
        expansions+=count; roads.push(road);
    }
    let plan=Plan {version:1,profile,seed,parcels,roads,expansions,design:None};
    if !plan.valid() { return Err("Invalid generated neighborhood".into()); }
    Ok(plan)
}

fn overlaps(a:[i32;4],b:[i32;4])->bool { a[0]<=b[2] && b[0]<=a[2] && a[1]<=b[3] && b[1]<=a[3] }
