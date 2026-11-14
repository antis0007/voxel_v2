use super::*;
use crate::world::{factory::{self, Kind, industry::{boiler, turbine::condensate}}, inventory::{Item, ItemStack}, industry::STEAM};

#[test]
fn fired_boiler_disk_checkpoint_preserves_paid_fuel_water_and_outlets_at_all_scales() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("voxel-fired-boiler-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let mut w = World::with_seed(WorldType::Superflat, scale, 701);
        w.regions = super::super::regions::RegionStream::open(dir.join("regions/test-world"), Default::default());
        let p = IVec3::new(-16, 64, -16);
        let n = factory::fixture_place(&mut w, p, Kind::Generator).unwrap();
        n.mode = boiler::MODE; n.water = 1; n.input[3] = 16;
        n.gas_vent_side = 1; n.gas_vent_species = (STEAM + 1) as u8;
        let size = factory::step(&w); w.factory.tick(0.25, size);
        let n = w.factory.nodes.get_mut(&p).unwrap(); n.enabled = false;
        let expected = (condensate(n), n.burn, n.fuel_resource, n.exhaust, n.input);
        assert_eq!(n.burn, 368);
        w.inventory.hotbar[0] = Some(ItemStack::new(Item::Bucket(0), 1)); w.sync_from_inventory();
        let (view, _) = w.capture_edits().unwrap().unwrap().persist().unwrap();
        let mut footer = session_footer(&w, SAVE_VERSION); w.activity.snapshot().encode(&mut footer);
        let bytes = manifest(session_header(&w, &Player::new(), &DayNightCycle::default()).unwrap(), &view, &footer, 1).unwrap();
        let path = dir.join("boiler.vxw"); fs::write(&path, bytes).unwrap();
        let mut loaded = load_exact(&path, &path).unwrap().world;
        assert_eq!(loaded.voxel_scale, scale); let n = &loaded.factory.nodes[&p];
        assert_eq!((n.mode, n.enabled, n.gas_vent_side, n.gas_vent_species), (boiler::MODE, false, 1, (STEAM + 1) as u8));
        assert_eq!((condensate(n), n.burn, n.fuel_resource, n.exhaust, n.input), expected);
        factory::action(&mut loaded, p.as_vec3(), factory::Action::SetRecipe(p, 0));
        assert_eq!(loaded.factory.nodes[&p].mode, boiler::MODE);
        factory::action(&mut loaded, p.as_vec3(), factory::Action::Toggle(p));
        loaded.factory.tick(0.25, size);
        let n = &loaded.factory.nodes[&p]; assert_eq!(n.burn, 336);
        assert_eq!(condensate(n) + u64::from(n.exhaust[STEAM]), u64::from(factory::industry::WATER_UNIT_MG));
        assert_eq!(n.input[3], 0);
        drop(loaded); drop(w); drop(view);
        let absolute = dir.canonicalize().unwrap(); assert!(absolute.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(absolute).unwrap();
    }
}
