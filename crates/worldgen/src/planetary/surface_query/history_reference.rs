//! Frozen V13 history implementation; no SurfaceQuery path.
use super::*;
impl World {
    pub(in crate::planetary) fn history_query_reference(&self, cfg: &WorldSampleConfig, dir: DVec3) -> Option<drift::history::RockColumn> {
        if self.history_top.is_empty() { return None; }
        let key = ((cfg.seed, cfg.planet_fingerprint(), cfg.generator_version), dir.to_array().map(f64::to_bits));
        if let Some(column) = HISTORY_ROCK.with(|cache| cache.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, c)| *c)) {
            return Some(column);
        }
        let (mut beds, weathered_myr) = self.plates.history_column(dir)?;
        let (local, weights) = self.mesh.stencil(dir);
        let sample = |field: &[f64]| (0..4).map(|i| field[local[i]] * weights[i]).sum::<f64>();
        let raw_top = sample(&self.history_top);
        beds.fit(raw_top, fit_mountain_height);
        let historical_top = fit_mountain_height(raw_top);
        let top = sample(&self.height);
        let edifice = if self.edifice_lift.is_empty() { 0.0 } else { sample(&self.edifice_lift) };
        let lava_m = if edifice > 12.0 {
            (top - fit_mountain_height(self.volcanic_base(dir))).max(0.0)
        } else { 0.0 };
        let sediment_top = top - lava_m;
        let carbonate = sample(&self.carbonate);
        if sediment_top < historical_top {
            beds.trim(historical_top - sediment_top);
        } else {
            beds.deposit(history_duration_myr(cfg), sediment_top - historical_top, sample(&self.rain),
                sample(&self.temperature), carbonate, false, sediment_top < SEA + 2.0);
        }
        let column = drift::history::RockColumn { beds, top, lava_m, carbonate, weathered_myr };
        HISTORY_ROCK.with(|cache| *cache.borrow_mut() = Some((key, column)));
        Some(column)
    }

}
