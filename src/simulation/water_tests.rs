use super::*;
use crate::{fluid::VoxelScalePreset, world::WorldType};

fn chamber() -> World {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut cells = Vec::new();
    for y in 100..116 { for z in 4..13 { for x in 4..17 {
        cells.push((IVec3::new(x,y,z), B::Stone));
    }}}
    world.set_many(&cells);
    world
}
fn tick(world: &mut World, p: IVec3) -> usize {
    step(&mut Residency::default(), world, p, (Vec3::ZERO, Vec3::ONE),
        &mut HashSet::new(), &mut HashSet::new(), &mut HashSet::new())
}
fn mass(world: &World) -> u32 {
    let mut total=0;
    for y in 100..116 { for z in 4..13 { for x in 4..17 {
        total += world.water_volume(IVec3::new(x,y,z)) as u32;
    }}}
    total
}
#[test]
fn last_water_quantum_drains_down_a_ledge_without_mass_loss() {
    let mut world=chamber();
    let p=IVec3::new(6,104,8);
    let outlet=p+IVec3::X-IVec3::Y;
    world.set_many(&[(p+IVec3::X,B::Air),(outlet,B::Air)]);
    world.set_water_volume(p,1);
    assert_eq!(tick(&mut world,p),2);
    assert_eq!(world.water_volume(p),0);
    assert_eq!(world.water_volume(outlet),1);
    assert_eq!(mass(&world),1);
}
#[test]
fn connected_water_pushes_up_an_outlet_below_its_head() {
    let mut world=chamber();
    for x in 6..=10 { world.set_water_volume(IVec3::new(x,102,8),8); }
    for y in 103..=106 { world.set_water_volume(IVec3::new(6,y,8),8); }
    let outlet=IVec3::new(10,103,8);
    world.set_many(&[(outlet,B::Air),(IVec3::new(6,107,8),B::Air)]);
    let before=mass(&world);
    assert_eq!(tick(&mut world,IVec3::new(6,102,8)),2);
    assert_eq!(world.water_volume(outlet),2);
    assert_eq!(mass(&world),before);
}
#[test]
fn pressure_does_not_cross_a_solid_wall() {
    let mut world=chamber();
    for y in 102..=106 { world.set_water_volume(IVec3::new(6,y,8),8); }
    let outlet=IVec3::new(8,103,8);
    world.set_many(&[(outlet,B::Air),(IVec3::new(6,107,8),B::Air)]);
    let before=mass(&world);
    assert_eq!(tick(&mut world,IVec3::new(6,102,8)),0);
    assert_eq!(world.water_volume(outlet),0);
    assert_eq!(mass(&world),before);
}
