//! Which body's sphere of influence holds a point, and the pull it exerts.
use crate::data::BodyKind;
use crate::kepler::G;
use crate::system::{BodyId, Snapshot, System};
use crate::transfer::soi_radius;
use glam::DVec3;

impl System {
    /// Laplace sphere of influence radius in metres; `None` for the root and barycenters.
    pub fn soi(&self, b: BodyId) -> Option<f64> {
        if matches!(self.kind(b), BodyKind::Barycenter) {
            return None;
        }
        let a = self.elements(b)?.a;
        Some(soi_radius(a, self.mass(b), self.mass(self.parent(b)?)))
    }

    /// Body with the smallest sphere of influence holding `point` (system frame, metres).
    ///
    /// Falls back to the root. Spheres are not nested by parent: home's own
    /// is smaller than its moon's orbit, so the smallest containing sphere wins.
    pub fn dominant_body(&self, snapshot: &Snapshot, point: DVec3) -> BodyId {
        (1..self.len() as u16)
            .map(BodyId)
            .filter_map(|b| Some((b, self.soi(b)?)))
            .filter(|&(b, r)| (point - snapshot.position(b)).length() < r)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(BodyId(0), |(b, _)| b)
    }

    /// Gravitational acceleration toward `body` at `point`, m/s², system frame.
    ///
    /// Inside the body the pull falls linearly to zero at the centre.
    pub fn pull(&self, snapshot: &Snapshot, body: BodyId, point: DVec3) -> DVec3 {
        let delta = snapshot.position(body) - point;
        let r = delta.length().max(1.0);
        let radius = self.radius(body).max(1.0);
        let mu = G * self.mass(body);
        let magnitude = if r >= radius { mu / (r * r) } else { mu * r / (radius * radius * radius) };
        delta / r * magnitude
    }
}

#[cfg(test)]
mod tests {
    use crate::system::{BodyId, Snapshot, System};
    use glam::DVec3;

    fn compile() -> System {
        let text = include_str!("../../../assets/star_systems.ron");
        System::compile(&crate::parse(text).unwrap().systems[0], 1.0 / 120.0).unwrap()
    }
    fn id(system: &System, name: &str) -> BodyId {
        system.id(name).unwrap()
    }

    #[test]
    fn dominant_body_follows_the_spheres_of_influence() {
        let system = compile();
        let (home, moon, star) = (id(&system, "home"), id(&system, "moon"), id(&system, "star"));
        let mut s = Snapshot::default();
        system.evaluate(0, &mut s);
        let at = |body, offset: f64| s.position(body) + DVec3::new(offset, 0.0, 0.0);
        let moon_soi = system.soi(moon).unwrap();
        assert!(moon_soi > 4_000.0 && moon_soi < 20_000.0, "moon soi {moon_soi}");
        assert_eq!(system.dominant_body(&s, at(moon, moon_soi * 0.5)), moon);
        assert_eq!(system.dominant_body(&s, at(home, 1_000.0)), home);
        assert_eq!(system.dominant_body(&s, at(home, system.soi(home).unwrap() * 1.5)), star);
    }

    #[test]
    fn pull_points_at_the_body_and_matches_surface_gravity() {
        let system = compile();
        let moon = id(&system, "moon");
        let mut s = Snapshot::default();
        system.evaluate(0, &mut s);
        let p = s.position(moon) + DVec3::new(0.0, system.radius(moon), 0.0);
        let g = system.pull(&s, moon, p);
        assert!(g.y < 0.0 && g.x.abs() < 1e-9);
        assert!((g.length() - system.surface_gravity(moon)).abs() < 1e-9);
    }
}
