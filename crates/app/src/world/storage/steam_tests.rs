use super::*;
use crate::world::{factory::{self, Kind, industry::turbine}, inventory::{Item, ItemStack}, industry::STEAM};

#[test]
fn turbine_disk_checkpoint_preserves_condensate_regions_and_bottles_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("voxel-steam-save-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let mut w = World::with_seed(WorldType::Superflat, scale, 701);
        w.regions = super::super::regions::RegionStream::open(dir.join("regions/test-world"), Default::default());
        let p = IVec3::new(-16, 64, -16);
        let n = factory::fixture_place(&mut w, p, Kind::GasGenerator).unwrap();
        n.mode = 1; n.enabled = false; n.water = 7; n.separator_water_mg = 25_000;
        n.exhaust[STEAM] = 500; n.energy = 13;
        let expected_mass = turbine::condensate(n);
        w.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1));
        w.inventory.pack[0] = Some(ItemStack::new(Item::Bucket(1152), 1));
        w.sync_from_inventory();
        let (view, _) = w.capture_edits().unwrap().unwrap().persist().unwrap();
        let mut footer = session_footer(&w, SAVE_VERSION);
        w.activity.snapshot().encode(&mut footer);
        let bytes = manifest(session_header(&w, &Player::new(), &DayNightCycle::default()).unwrap(), &view, &footer, 1).unwrap();
        let path = dir.join("steam.vxw");
        fs::write(&path, bytes).unwrap();
        let mut loaded = load_exact(&path, &path).unwrap().world;
        assert_eq!(loaded.voxel_scale, scale);
        let n = &loaded.factory.nodes[&p];
        assert_eq!((n.mode, n.enabled, n.energy, n.exhaust[STEAM]), (1, false, 13, 500));
        assert_eq!(turbine::condensate(n), expected_mass);
        assert_eq!(loaded.inventory.pack[0], w.inventory.pack[0]);
        factory::action(&mut loaded, p.as_vec3(), factory::Action::SetRecipe(p, 0));
        factory::action(&mut loaded, p.as_vec3(), factory::Action::Drain(p));
        assert_eq!(turbine::condensate(&loaded.factory.nodes[&p]), 0);
        assert_eq!(loaded.inventory.hotbar[0].as_ref().unwrap().item, Item::Bucket(expected_mass as u32));
        assert_eq!(loaded.inventory.pack[0], w.inventory.pack[0]);
        drop(loaded); drop(w); drop(view);
        let absolute = dir.canonicalize().unwrap();
        assert!(absolute.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(absolute).unwrap();
    }
}
