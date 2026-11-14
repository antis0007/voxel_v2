//! Portal entities use metres and double precision, independently of voxel scale.
//! Nether regions share a dimension identity while retaining their parent planet.
use anyhow::{Result, ensure};
use glam::{DQuat, DVec2, DVec3};
use serde::{Deserialize, Serialize};

pub mod gates;
pub mod authored;
#[cfg(test)]
mod gates_tests;
pub mod realm;
pub mod sidecar;
pub mod traverse;
pub mod views;

pub const MAX_PORTALS: usize = 128;
pub const MAX_TRAVERSALS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Realm {
    Planet(String),
    Nether { planet: String },
}
impl Realm {
    pub fn planet(&self) -> &str {
        match self {
            Self::Planet(id) | Self::Nether { planet: id } => id,
        }
    }
    pub fn validate(&self) -> Result<()> {
        let id = self.planet();
        ensure!(
            !id.is_empty()
                && id.len() <= 96
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "Invalid planet identity"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub center: DVec3,
    /// Local X/Y are aperture axes; local +Z is the entrance side.
    pub rotation: DQuat,
    pub half_size: DVec2,
}
impl Frame {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.center.is_finite() && self.center.abs().max_element() <= 1_000_000.0,
            "Portal position out of range"
        );
        ensure!(
            self.rotation.is_finite() && (self.rotation.length_squared() - 1.0).abs() < 1e-6,
            "Invalid portal rotation"
        );
        ensure!(
            self.half_size.is_finite()
                && self.half_size.min_element() >= 0.25
                && self.half_size.max_element() <= 32.0,
            "Invalid portal aperture"
        );
        Ok(())
    }
    pub fn normal(self) -> DVec3 {
        self.rotation * DVec3::Z
    }
    pub fn vertical(self) -> bool {
        (self.rotation * DVec3::Y).distance_squared(DVec3::Y) < 1e-10
    }
    pub fn local(self, p: DVec3) -> DVec3 {
        self.rotation.conjugate() * (p - self.center)
    }
    pub fn corners(self) -> [DVec3; 4] {
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| {
            self.center
                + self.rotation * DVec3::new(x * self.half_size.x, y * self.half_size.y, 0.0)
        })
    }
    /// Positive on the destination side retained by front clipping.
    pub fn clip_distance(self, p: DVec3) -> f64 {
        (p - self.center).dot(self.normal())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Portal {
    pub id: u64,
    pub source: Realm,
    pub destination: Realm,
    pub entrance: Frame,
    pub exit: Frame,
}

impl Portal {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.id != 0, "Invalid portal identity");
        self.source.validate()?;
        self.destination.validate()?;
        self.entrance.validate()?;
        self.exit.validate()?;
        // The player controller currently has fixed gravity and body scale.
        // Do not silently accept transforms it cannot traverse correctly.
        ensure!(
            (self.entrance.half_size - self.exit.half_size)
                .abs()
                .max_element()
                < 1e-6,
            "Portal scaling is not supported"
        );
        Ok(())
    }
    pub fn rotation(&self) -> DQuat {
        self.exit.rotation
            * DQuat::from_rotation_y(std::f64::consts::PI)
            * self.entrance.rotation.conjugate()
    }
    pub fn point(&self, p: DVec3) -> DVec3 {
        self.exit.center + self.rotation() * (p - self.entrance.center)
    }
    pub fn vector(&self, v: DVec3) -> DVec3 {
        self.rotation() * v
    }
    pub fn reverse(&self, id: u64) -> Self {
        Self {
            id,
            source: self.destination.clone(),
            destination: self.source.clone(),
            entrance: self.exit,
            exit: self.entrance,
        }
    }
    /// First plane intersection, bounded by the aperture. Uses the actual swept
    /// camera segment, so fast motion cannot jump over a narrow portal trigger.
    pub fn intersect(&self, start: DVec3, end: DVec3, margin: DVec2) -> Option<f64> {
        if !margin.is_finite() || margin.min_element() < 0.0 {
            return None;
        }
        let a = self.entrance.local(start);
        let b = self.entrance.local(end);
        if !a.is_finite() || !b.is_finite() || a.z <= 0.0 || b.z > 0.0 {
            return None;
        }
        let fraction = a.z / (a.z - b.z);
        let at = a.lerp(b, fraction);
        let aperture = self.entrance.half_size - margin;
        (aperture.min_element() > 0.0 && at.x.abs() <= aperture.x && at.y.abs() <= aperture.y)
            .then_some(fraction)
    }
    pub fn upright(&self) -> bool {
        self.vector(DVec3::Y).distance_squared(DVec3::Y) < 1e-10
    }
}

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Network {
    pub portals: Vec<Portal>,
}
impl Network {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.portals.len() <= MAX_PORTALS, "Portal limit exceeded");
        let mut ids = std::collections::HashSet::new();
        for portal in &self.portals {
            portal.validate()?;
            ensure!(ids.insert(portal.id), "Duplicate portal identity");
        }
        Ok(())
    }
    pub fn first_crossing(
        &self,
        realm: &Realm,
        start: DVec3,
        end: DVec3,
        margin: DVec2,
    ) -> Option<(&Portal, f64)> {
        self.portals
            .iter()
            .filter(|p| &p.source == realm)
            .filter_map(|p| p.intersect(start, end, margin).map(|t| (p, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.id.cmp(&b.0.id)))
    }
    /// Publish both directions together; a rejected pair leaves the live graph
    /// untouched. Callers must persist the resulting graph with its worlds.
    pub fn link(&mut self, portal: Portal, reverse_id: u64) -> Result<()> {
        let mut candidate = self.clone();
        candidate.portals.push(portal.reverse(reverse_id));
        candidate.portals.push(portal);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Plan an upright body crossing without changing either world's state.
    /// Readiness and swept destination collision are deliberately one required
    /// callback: unknown terrain must never be interpreted as empty space.
    /// The controller commits this result before choosing its render camera.
    pub fn transit(
        &self,
        realm: &Realm,
        start: DVec3,
        end: DVec3,
        velocity: DVec3,
        orientation: DQuat,
        body_half_size: DVec2,
        mut admits: impl FnMut(&Realm, DVec3, DVec3, DVec2) -> bool,
    ) -> Transit {
        if !start.is_finite()
            || !end.is_finite()
            || !velocity.is_finite()
            || !orientation.is_finite()
            || (orientation.length_squared() - 1.0).abs() > 1e-6
            || !body_half_size.is_finite()
            || body_half_size.min_element() <= 0.0
        {
            return Transit::Blocked;
        }
        let mut realm = realm.clone();
        let mut start = start;
        let mut end = end;
        let mut velocity = velocity;
        let mut orientation = orientation;
        let mut last = None;
        for depth in 0..=MAX_TRAVERSALS {
            // Detect the centre first: a body too wide for the aperture must
            // stop at the plane, not pass through an invisible gate.
            let crossing = self.first_crossing(&realm, start, end, DVec2::ZERO);
            let segment_end = crossing.map_or(end, |(_, t)| start.lerp(end, t));
            if last.is_some() && !admits(&realm, start, segment_end, body_half_size) {
                return Transit::Blocked;
            }
            let Some((portal, _)) = crossing else {
                return last.map_or(Transit::Unchanged, |portal_id| {
                    Transit::Crossed(Crossing {
                        portal_id,
                        position: end,
                        velocity,
                        orientation,
                    })
                });
            };
            if depth == MAX_TRAVERSALS
                || !portal.upright()
                || !portal.entrance.vertical()
                || !portal.exit.vertical()
                || portal.intersect(start, end, body_half_size).is_none()
            {
                return Transit::Blocked;
            }
            // Transform the residual, not just the point at the exit. This
            // preserves speed even when one step crosses several openings.
            start = portal.point(segment_end);
            end = portal.point(end);
            velocity = portal.vector(velocity);
            orientation = portal.rotation() * orientation;
            realm = portal.destination.clone();
            last = Some(portal.id);
        }
        Transit::Blocked
    }
}

#[derive(Clone, Debug)]
pub struct Crossing {
    pub portal_id: u64,
    pub position: DVec3,
    pub velocity: DVec3,
    pub orientation: DQuat,
}

#[derive(Clone, Debug)]
pub enum Transit {
    Unchanged,
    Blocked,
    Crossed(Crossing),
}
