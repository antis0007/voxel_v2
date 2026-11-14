//! Fixed spherical sampling locations for evolving historical weather.
use super::Mesh;
use engine_core::cube_sphere::atlas;
use glam::DVec3;

const REACHES: [f64; 4] = [2200.0, 800.0, 2400.0, 6500.0];
type Stencil = ([usize; 4], [f64; 4]);

// Geometry only: the spherical mesh and wind directions are fixed during a solve.
// Heights must remain live because tectonics and erosion change them every step.
// Keep stencil indices, weights, and accumulation order identical to Mesh::sample.
pub(super) struct WeatherSamples { nodes: Vec<[Stencil; 4]> }

fn direction(dir: DVec3, reach: f64) -> DVec3 {
    let east = DVec3::Y.cross(dir).normalize_or_zero();
    let wind = if dir.y.abs() > 0.42 && dir.y.abs() < 0.82 { east } else { -east };
    (dir - wind * (reach / atlas::RADIUS_M)).normalize()
}

impl WeatherSamples {
    pub(super) fn new(mesh: &Mesh) -> Self {
        Self { nodes: mesh.directions.iter().map(|&dir|
            REACHES.map(|reach| mesh.stencil(direction(dir, reach)))).collect() }
    }

    pub(super) fn sample(&self, height: &[f64], node: usize, reach: usize) -> f64 {
        let (ids, weights) = self.nodes[node][reach];
        (0..4).map(|i| height[ids[i]] * weights[i]).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_weather_samples_are_exact() {
        for side in [4, 17, 84] {
            let mesh = Mesh::with_side(side);
            let cached = WeatherSamples::new(&mesh);
            for field in 0..3 {
                let height: Vec<_> = (0..mesh.directions.len()).map(|i|
                    match field { 0 => 12.0, 1 => (i as f64 * 0.31).sin() * 200.0,
                        _ => if i % 3 == 0 { -100.0 } else { 500.0 } }).collect();
                for (i, &dir) in mesh.directions.iter().enumerate() {
                    let east = DVec3::Y.cross(dir).normalize_or_zero();
                    let wind = if dir.y.abs() > 0.42 && dir.y.abs() < 0.82 { east } else { -east };
                    for (r, reach) in [2200.0, 800.0, 2400.0, 6500.0].into_iter().enumerate() {
                        let source = (dir - wind * (reach / atlas::RADIUS_M)).normalize();
                        assert_eq!(cached.sample(&height, i, r).to_bits(), mesh.sample(&height, source).to_bits());
                    }
                }
            }
        }
    }
}
