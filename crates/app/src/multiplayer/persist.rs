//! Periodic host saves are skipped when nothing a save writes has changed.
use super::*;

impl Session {
    pub(super) fn save_periodic(&mut self, world: &mut World, player: &Player, day: &DayNightCycle) -> Result<()> {
        if self.sidecar.is_some() {
            return self.write_profiles(true);
        }
        let mark = self.building_save_mark(world, player, day).ok();
        if mark.is_some_and(|mark| save_unchanged(false, mark, (self.saved_revision, self.saved_profiles))) {
            self.profiles_dirty = false;
            return Ok(());
        }
        self.save(world, player, day)?;
        if let Some((_, profiles)) = mark {
            self.saved_profiles = profiles;
        }
        Ok(())
    }

    /// Crafting and combat can change without incrementing the block revision.
    fn building_save_mark(&self, world: &World, player: &Player, day: &DayNightCycle) -> Result<(u64, u64)> {
        let mut text = ron::to_string(&self.collected_profiles()?)?;
        let gameplay = gameplay::capture(world, player, day, 0, Vec::new())?;
        text.push_str(&engine_net::transfer::checksum(&gameplay).to_string());
        Ok((world.revision(), engine_net::transfer::checksum(text.as_bytes())))
    }
}

/// Whether a periodic save would write what the last one wrote. A sidecar holds only
/// profiles, since the hosted world saves through its own storage.
pub(super) fn save_unchanged(sidecar: bool, now: (u64, u64), saved: (u64, u64)) -> bool {
    now.1 == saved.1 && (sidecar || now.0 == saved.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_world_skips_the_periodic_save() {
        assert!(save_unchanged(false, (7, 42), (7, 42)));
        assert!(!save_unchanged(false, (8, 42), (7, 42)), "world edited");
        assert!(!save_unchanged(false, (7, 43), (7, 42)), "a profile changed");
        assert!(save_unchanged(true, (8, 42), (7, 42)), "sidecar ignores the world revision");
        assert!(!save_unchanged(true, (7, 43), (7, 42)));
    }
}
