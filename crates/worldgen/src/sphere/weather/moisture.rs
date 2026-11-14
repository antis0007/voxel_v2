//! Temperature relaxation, evaporation, condensation and cloud lifetime.

use super::{DT, WeatherSim};

/// Humidity the air holds at normalised temperature `t`.
pub(super) fn saturation(t: f64) -> f64 {
    0.04 + 0.96 * t * t
}

/// Ocean evaporation per time unit at full dryness and temperature 1.
const EVAPORATION: f64 = 0.35;
const LAND_EVAPORATION: f64 = 0.08;
/// Fraction of the excess over saturation that condenses per time unit.
const CONDENSE_RATE: f64 = 0.8;
/// Cloud water made per unit of condensed humidity.
const CLOUD_YIELD: f64 = 2.0;
/// Cloud lifetime in still air, model time units.
const CLOUD_LIFE: f64 = 4.0;
/// Convergence to lift: a divergence of 0.04 per time unit counts as full lift.
const LIFT_GAIN: f64 = 40.0;
const OROGRAPHIC_GAIN: f64 = 1.0;
const OCEAN_HEAT_LAG: f64 = 30.0;
const LAND_HEAT_LAG: f64 = 8.0;

impl WeatherSim {
    pub(super) fn step_moisture(&mut self) {
        let wet = 0.5 + self.params.wetness;
        for i in 0..self.ops.len() {
            let land = f64::from(self.surface.land[i]);
            let lag = OCEAN_HEAT_LAG + (LAND_HEAT_LAG - OCEAN_HEAT_LAG) * land;
            self.temp[i] += DT * (self.temp_eq[i] - self.temp[i]) / lag;
            let t = self.temp[i].clamp(0.0, 1.0);
            let q_sat = saturation(t);

            // Lift from convergence and from wind pushed up sloping ground; subsidence is negative.
            let upslope = self.v[i].dot(self.slope[i]) * OROGRAPHIC_GAIN;
            let lift = (-self.div[i] * LIFT_GAIN + upslope).clamp(-1.0, 1.0);
            let q_eff = q_sat * (1.0 - 0.6 * lift);

            let soil = f64::from(self.surface.land_moisture[i]).clamp(0.0, 1.0);
            let source = EVAPORATION * (1.0 - land) + LAND_EVAPORATION * land * (0.55 + 0.9 * soil);
            let q = &mut self.humidity[i];
            *q += DT * wet * source * (0.2 + t) * (1.0 - *q / q_sat).max(0.0);
            let condensed = (*q - q_eff).max(0.0) * CONDENSE_RATE * DT;
            *q -= condensed;

            let life = CLOUD_LIFE / (1.0 + 2.0 * (-lift).max(0.0));
            let c = &mut self.cloud[i];
            *c += condensed * CLOUD_YIELD - DT * *c / life;
            *c = c.clamp(0.0, 2.0);
        }
    }
}
