//! Opt-in disposable native fixture; never used by ordinary gameplay.
use super::*;

pub fn prepare(world: &mut World, player: &crate::player::Player) -> IVec3 {
    let m=world.voxel_scale.meters();
    let anchor=(player.eye(world.voxel_scale)+player.forward_dir()*2.5/m).floor().as_ivec3();
    let other=anchor+IVec3::X*3;
    let copper=Item::Resource(6);
    for p in [anchor,other] { world.set(p,BlockId::Chest); }
    let mut a=[None;ops::SLOTS];
    a[0]=Some(ItemStack{item:copper,count:34});
    a[1]=Some(ItemStack{item:Item::Block(BlockId::Stone),count:184});
    world.block_data.set_chest(anchor,a);
    let mut b=[None;ops::SLOTS];
    b[0]=Some(ItemStack{item:copper,count:171});
    b[1]=Some(ItemStack{item:copper,count:900});
    b[2]=Some(ItemStack{item:Item::Block(BlockId::Stone),count:221});
    world.block_data.set_chest(other,b);
    world.inventory=super::super::inventory::SurvivalInventory::empty();
    world.inventory.voxel_m=m;
    world.inventory.pack[0]=Some(ItemStack{item:copper,count:50});
    world.inventory.pack[2]=Some(ItemStack{item:Item::Block(BlockId::Stone),count:107});
    let found=discover(world,(anchor.as_vec3()+Vec3::splat(0.5))*m,RADIUS_M,Some(anchor));
    let before=index(world,&found).stock.iter().find(|s|s.item==copper).unwrap().total;
    assert_eq!(deposit(world,&found,Reach::Existing),157);
    assert_eq!(withdraw(world,&found,copper,50),50);
    let freed=consolidate_preview(world,&found);
    assert_eq!(consolidate(world,&found),freed);
    assert_eq!(index(world,&found).stock.iter().find(|s|s.item==copper).unwrap().total,before);
    let bytes=block_data::sidecar::encode(&world.block_data);
    world.block_data=block_data::sidecar::decode(&bytes).expect("native storage roundtrip");
    if std::env::var_os("VOXEL_REVIEW_SLOT_FLOW").is_some() {
        use block_data::{ChestAction, SlotGesture};
        use super::super::inventory::SlotAddr;
        let eye = player.eye(world.voxel_scale) * m;
        let saved = world.inventory.clone();
        let saved_slots = world.block_data.chest(anchor);
        let mut slots = saved_slots.clone();
        let original = ItemStack::new(copper, 8);
        slots[0] = Some(original);
        slots[4] = None;
        world.block_data.set_chest(anchor, slots);
        world.inventory.pack[5] = None;
        let total = world.inventory.count_item(original.item) + world.block_data.chest(anchor)
            .iter().flatten().filter(|s| s.item == original.item).map(|s| s.count).sum::<u32>();
        for action in [
            ChestAction::Chest(0, SlotGesture::Split),
            ChestAction::Chest(4, SlotGesture::Split),
            ChestAction::Player(SlotAddr::Pack(5), SlotGesture::Stack),
            ChestAction::Player(SlotAddr::Pack(5), SlotGesture::Transfer),
            ChestAction::Chest(4, SlotGesture::Transfer),
        ] {
            commands::apply(world, anchor, eye, commands::Action::Chest(action)).expect("host chest command");
        }
        assert!(world.inventory.cursor.is_none() && world.block_data.chest(anchor)[4].is_none());
        assert_eq!(world.inventory.count_item(original.item) + world.block_data.chest(anchor)
            .iter().flatten().filter(|s| s.item == original.item).map(|s| s.count).sum::<u32>(), total);
        assert!(commands::apply(world, anchor, eye + Vec3::Y * 100.0,
            commands::Action::Chest(ChestAction::TakeAll)).is_err());
        world.inventory = saved;
        world.block_data.set_chest(anchor, saved_slots);
        println!("[slot-flow-native] chest split place-one cursor quick-transfer conservation reach=ok");
        station_flow(world.voxel_scale);
    }
    if std::env::var_os("VOXEL_REVIEW_SCANNER_CRAFT").is_some() {
        let mut slots = world.block_data.chest(anchor);
        slots[5] = Some(ItemStack { item: Item::Block(BlockId::Clay), count: 20 });
        slots[6] = Some(ItemStack { item: Item::Block(BlockId::Gravel), count: 20 });
        world.block_data.set_chest(anchor, slots);
        policy::set(world, anchor, policy::Policy { reserve: 2, ..Default::default() });
        let id = super::super::inventory::all_recipes().iter().position(|r| r.name == "Seared Brick").unwrap();
        let output = commands::apply(world, anchor, player.eye(world.voxel_scale)*m, commands::Action::CraftBatch(id,3)).unwrap();
        assert_eq!(output, 12);
        assert_eq!(world.inventory.count_item(Item::Block(BlockId::SearedBrick)),12);
        assert_eq!(world.block_data.chest(anchor)[5].unwrap().count,14);
        assert_eq!(world.block_data.chest(anchor)[6].unwrap().count,14);
        println!("[scanner-craft-native] batches=3 output=12 clay=14 gravel=14 reserve=2");
    }
    world.sync_from_inventory();
    println!("[scanner-native] anchor={anchor:?} containers={} copper={before} deposit=157 withdraw=50 freed={freed} sidecar=ok",found.len());
    anchor
}

fn station_flow(scale: crate::fluid::VoxelScalePreset) {
    use super::super::{inventory::{SlotAddr, SurvivalInventory}, progression::Slot, stations};
    use crate::world::WorldType;
    let mut world = World::with_seed(WorldType::Superflat, scale, 31);
    world.inventory = SurvivalInventory::empty();
    world.progression.xp = 100;
    let a = SlotAddr::CraftGrid(0);
    let b = SlotAddr::CraftGrid(1);
    let sword = Item::Weapon(2);
    let max = crate::world::progression::table::max_durability(sword).unwrap();
    world.inventory.hotbar[0] = Some(ItemStack::new(sword, 1));
    world.progression.wear.reconcile(&world.inventory);
    world.progression.wear.spend(&mut world.inventory, Slot::Hotbar(0), max / 2);
    world.inventory.click_slot(SlotAddr::Hotbar(0));
    world.inventory.click_slot(a);
    world.progression.wear.reconcile(&world.inventory);
    assert_eq!(stations::durable(&world, a), Some((sword, max - max / 2, max)));
    world.inventory.craft_grid[1] = Some(ItemStack::new(Item::Resource(15), 8));
    assert!(stations::use_anvil(&mut world, a, b));
    assert_eq!(stations::durable(&world, a), Some((sword, max, max)));
    assert!(stations::salvage_in_slot(&mut world, a));
    assert_ne!(world.inventory.slot(a).unwrap().item, sword);
    world.inventory.return_transient_to_pack();
    let row = &stations::table::STATIONS.stonecutter[0];
    world.inventory.craft_grid[0] = Some(ItemStack::new(row.input, 3));
    stations::cut_from_slot(&mut world, a, 0, false);
    assert_eq!(world.inventory.cursor, Some(ItemStack::new(row.output, row.count)));
    stations::cut_from_slot(&mut world, a, 0, true);
    assert!(world.inventory.slot(a).is_none());
    let stand = IVec3::new(8, 16, 8);
    world.set(stand, BlockId::BrewingStand);
    world.inventory.cursor = Some(ItemStack::new(Item::Resource(18), 2));
    stations::brewing::edit_slot(&mut world, stand, 0, true);
    assert_eq!(world.block_data.stand(stand).base.unwrap().count, 1);
    assert_eq!(world.inventory.cursor.unwrap().count, 1);
    stations::brewing::edit_slot(&mut world, stand, 0, false);
    assert_eq!(world.block_data.stand(stand).base.unwrap().count, 2);
    println!("[slot-flow-native] anvil repair wear=ok grindstone salvage=ok stonecutter cursor+batch=ok brewing split+merge=ok");
}
