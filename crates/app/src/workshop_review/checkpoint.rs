use super::*;
#[derive(Default)]
pub(super) struct Checkpoint { requested: bool, inventory: Vec<u8>, resources: Vec<u32>, position: Vec3, pub reloaded: bool }
impl Checkpoint {
    pub fn tick(&mut self, w: &mut World, p: &mut Player, day: &mut world::DayNightCycle,
        saves: &mut world::storage::SaveService, mined: &[IVec3], distance: f32,
        grounded: u32, airborne: u32, collision: bool, cast_seen: bool) -> bool {
        if !self.requested {
            world::storage::set_autosave_enabled(false);
            self.inventory.clear();
            w.inventory.encode_versioned(&mut self.inventory, world::storage::SAVE_VERSION);
            self.resources = w.expedition.resources.to_vec();
            self.position = p.pos;
            saves.request(true);
            saves.tick(Some(w), p, day);
            self.requested = true;
            return false;
        }
        saves.tick(Some(w), p, day);
        if saves.busy() { return false; }
        if saves.failed {
            eprintln!("[workshop-loop] FAIL checkpoint {:?}", saves.status);
            return true;
        }
        match world::storage::load_review_checkpoint() {
            Ok(loaded) => {
                let mut inventory = Vec::new();
                loaded.world.inventory.encode_versioned(&mut inventory, world::storage::SAVE_VERSION);
                let inventory_equal = inventory == self.inventory;
                let resources_equal = loaded.world.expedition.resources.as_slice() == self.resources;
                let records_equal = loaded.world.adventure.records == w.adventure.records;
                let wand_equal = loaded.world.adventure.wand == w.adventure.wand
                    && loaded.world.adventure.loadout.slots == w.adventure.loadout.slots;
                let terrain_equal = mined.iter().all(|&cell| loaded.world.get(cell) == w.get(cell));
                let position_equal = loaded.player.pos.distance(self.position) < 0.001;
                let returned = world::workshop::returned(&loaded.world.adventure);
                let (lo, hi) = world::player_aabb(loaded.player.pos, loaded.world.voxel_scale, loaded.player.scale, loaded.player.crouch_t);
                let no_penetration = !world::collides_world(&loaded.world, lo + Vec3::splat(0.002), hi - Vec3::splat(0.002));
                let passed = inventory_equal && resources_equal && records_equal && wand_equal && terrain_equal
                    && position_equal && returned && no_penetration && cast_seen && grounded > 120 && airborne > 0 && distance > 100.0;
                println!("[workshop-loop] {} inventory_equal={inventory_equal} resources_equal={resources_equal} records_equal={records_equal} wand_equal={wand_equal} terrain_equal={terrain_equal} position_equal={position_equal} returned={returned} no_penetration={no_penetration} cast_seen={cast_seen} distance_m={distance:.2} grounded_ticks={grounded} airborne_ticks={airborne} blocked_step_jump={collision}", if passed {"PASS"} else {"FAIL"});
                *w = loaded.world;
                *p = loaded.player;
                *day = loaded.day_night;
                self.reloaded = true;
                println!("[workshop-loop] live session replaced by disk checkpoint; no duplicate reward applied");
            }
            Err(error) => eprintln!("[workshop-loop] FAIL load={error:#}"),
        }
        true
    }
}
