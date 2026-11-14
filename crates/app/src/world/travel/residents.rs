//! Authored people attached to existing persistent resident IDs. Conversation
//! reads current world conditions; it never grants rewards or invents repair flags.
use super::{World, trades};

#[derive(serde::Deserialize)]
pub struct Record { pub title: String, pub text: String }
#[derive(serde::Deserialize)]
struct Voice {
    id:u32, name:String, ready:String, bench:String, water:String,
    following:String, home:String, route:String, works:String, rested:String,
    records:Vec<Record>,
    #[serde(default)]
    waterworks:Vec<String>,
}
static VOICES:std::sync::LazyLock<Vec<Voice>>=std::sync::LazyLock::new(|| {
    ron::from_str(include_str!("../../../../../assets/fernwake_residents.ron"))
        .unwrap_or_else(|error| {eprintln!("[residents] {error}");Vec::new()})
});
fn voice(id:u32)->Option<&'static Voice> {VOICES.iter().find(|v|v.id==id)}

pub fn records(id:u32)->&'static [Record] {
    voice(id).map_or(&[],|v|v.records.as_slice())
}

pub fn name(id: u32) -> Option<&'static str> {
    voice(id).map(|v|v.name.as_str())
}

pub fn conversation(world: &World, id: u32, issue: Option<trades::WorkplaceIssue>) -> Option<[&'static str; 2]> {
    let v=voice(id)?;
    if !engine_worldgen::fernwake::active(&world.sample_config()) { return None; }
    let work = match issue {
        Some(trades::WorkplaceIssue::Bench) => v.bench.as_str(),
        Some(trades::WorkplaceIssue::Water) => v.water.as_str(),
        Some(trades::WorkplaceIssue::Stockpile) => return None,
        None if world.travel.day==super::day(world)
            && world.travel.used.get(&(id,u16::MAX)).copied().unwrap_or(0)>0 => v.rested.as_str(),
        None => v.ready.as_str(),
    };
    let place = if world.travel.following == Some(id) {
        v.following.as_str()
    } else if world.travel.homes.contains_key(&id) {
        v.home.as_str()
    } else {
        if world.generator_version>=engine_worldgen::fernwake::waterworks::VERSION {v.works.as_str()}
        else {v.route.as_str()}
    };
    Some([work, place])
}

/// Local observations never treat generated fallback as repaired terrain.
pub fn waterworks(world:&World,id:u32)->Option<&'static str> {
    if id!=3 || !engine_worldgen::fernwake::active(&world.sample_config())
        || world.generator_version<engine_worldgen::fernwake::waterworks::VERSION {return None;}
    let cfg=world.sample_config();
    let offset=glam::Vec3::Y*engine_worldgen::habitats_region::floor(&cfg);
    let cell=|p:glam::Vec3|((p+offset)/world.voxel_scale.meters()).floor().as_ivec3();
    let stop=cell(engine_worldgen::fernwake::waterworks::STOP);
    let basin=cell(glam::Vec3::new(321.5,-0.5,54.5));
    if [stop,basin].iter().any(|p|!world.chunks.contains_key(&super::super::chunk_coord_from_world_block(*p))) {return None;}
    let state=if world.get(basin)==super::super::BlockId::Water {0}
        else if world.get(stop).is_solid() {1}else{2};
    voice(id)?.waterworks.get(state).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, WorldType};
    use engine_core::VoxelScalePreset;

    #[test]
    fn fernwake_conversation_tracks_repair_and_saved_home() {
        for seed in [1, 31, 9187] {
            let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, seed);
            world.generator_version = engine_worldgen::fernwake::VERSION;
            let (water, bench) = engine_worldgen::fernwake::workshop_cells(&world.sample_config(), 4).unwrap();
            let water = water.floor().as_ivec3();
            let bench = bench.floor().as_ivec3();
            world.set(water, BlockId::Water);
            world.set(bench, BlockId::Air);
            assert_eq!(trades::workplace_issue(&world, 4), Some(trades::WorkplaceIssue::Bench));
            let broken = conversation(&world, 4, trades::workplace_issue(&world, 4)).unwrap();
            world.set(bench, BlockId::Planks);
            assert_eq!(trades::workplace(&world, 4), None);
            let repaired = conversation(&world, 4, trades::workplace_issue(&world, 4)).unwrap();
            assert_ne!(broken[0], repaired[0]);
            world.set(water, BlockId::Air);
            assert_eq!(trades::workplace_issue(&world, 4), Some(trades::WorkplaceIssue::Water));
            assert_ne!(conversation(&world, 4, trades::workplace_issue(&world, 4)).unwrap()[0], repaired[0]);

            world.travel.homes.insert(4, glam::Vec3::new(8.0, 12.0, 16.0));
            world.travel.following = Some(4);
            let following = conversation(&world, 4, None).unwrap();
            // Homes persist; active following deliberately ends on reload.
            world.travel = super::super::sidecar::decode(&super::super::sidecar::encode(&world.travel)).unwrap();
            assert_eq!(world.travel.following, None);
            assert_ne!(conversation(&world, 4, None).unwrap()[1], following[1]);
            assert_ne!(conversation(&world, 4, None).unwrap()[1], repaired[1]);
            assert!(conversation(&world, 1024, None).is_none());
            world.generator_version = engine_worldgen::fernwake::VERSION - 1;
            assert!(conversation(&world, 4, None).is_none());
        }
    }
}
