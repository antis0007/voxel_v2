//! Drainage solved over a whole cube-sphere planet, all six faces at once.
//!
//! The single-face solve in `geo` (ADR 0038) has map edges to drain into; a
//! closed sphere has none, so base level comes from the planet class: an ocean,
//! retained basins, or both. Nothing reads this module during generation yet:
//! there is no generator gate and no save field, so existing worlds are unchanged.
//!
//! Determinism: positions come from `Projection::Spherified.to_sphere`, which
//! is `n + s u + t v` scaled per axis by square roots of polynomials and then
//! normalized. glam computes `dot` and `normalize` without fused multiply-add,
//! so the forward map uses only `+ - * /` and `sqrt`, all correctly rounded in
//! IEEE 754. `Projection::Tangent` calls `tan` and is not used. The height
//! noise adds `floor`, which is exact. The flood breaks ties by cell index.

mod class;
mod flood;
mod grid;
mod noise;
mod weather;
#[cfg(test)]
mod tests;

pub use class::{OCEAN_MIN_SHARE, PlanetClass, PlanetParams};
pub use flood::{Drainage, solve, solve_heights};
pub use grid::{PROJECTION, SphereGrid};
pub use noise::{elevation, height_m};
pub use weather::{CloudCover, DT as WEATHER_DT, Surface, WeatherParams, WeatherSim};
pub use weather::clouds::CloudSky;
pub use weather::storms::Kind as StormKind;
