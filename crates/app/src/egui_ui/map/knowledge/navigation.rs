//! Constant-space movement evidence between grounded positions. No terrain reads.
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default)]
pub struct Motion {
    pub grounded: bool,
    pub flying: bool,
    pub swimming: bool,
    pub assisted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Travel {
    Walk,
    Traverse,
    Descend,
    Ascend,
    Swim,
    Fly,
    Assisted,
    Portal,
}

impl Travel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Walk => "Walked",
            Self::Traverse => "Traversed",
            Self::Descend => "Descent",
            Self::Ascend => "Ascent",
            Self::Swim => "Swam",
            Self::Fly => "Flew",
            Self::Assisted => "Assisted",
            Self::Portal => "Last crossed",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub at: Instant,
    pub point: Vec3,
    pub up: Vec3,
    pub scope: u64,
    pub layer: Option<u32>,
    pub motion: Motion,
}

pub(super) struct Leg {
    pub from: u32,
    pub to: u32,
    pub entry: Vec3,
    pub exit: Vec3,
    pub travel: Travel,
    pub metres: f32,
}

#[derive(Default)]
pub(super) struct Navigation {
    last: Option<Sample>,
    departure: Option<Sample>,
    airborne: bool,
    flying: bool,
    swimming: bool,
    assisted: bool,
    uncertain: bool,
    metres: f32,
}

impl Navigation {
    pub fn observe(&mut self, sample: Sample) -> Option<Leg> {
        let continuous = self.last.is_some_and(|last| {
            sample.scope == last.scope
                && sample.at.duration_since(last.at) <= Duration::from_secs(1)
                && sample.point.distance_squared(last.point) <= 64.0
                && sample.up.dot(last.up) > 0.98
        }) && self.departure.is_none_or(|start| {
            sample.at.duration_since(start.at) <= Duration::from_secs(120)
                && sample.up.dot(start.up) > 0.98
        });
        if !continuous {
            *self = Self::default();
        }
        if let Some(last) = self.last {
            self.metres += sample.point.distance(last.point);
            // Sparse samples can miss an entire jump between grounded poses.
            // Keep that history, but never advertise it as confirmed walking.
            self.uncertain |= sample.at.duration_since(last.at) > Duration::from_millis(250)
                || sample.point.distance_squared(last.point) > 2.25;
        }
        // Bound even an uninterrupted long flight. Its eventual landing must
        // not fabricate a short usable connection to a remote departure.
        if self.metres > 512.0 {
            *self = Self::default();
        }
        self.airborne |= !sample.motion.grounded;
        self.flying |= sample.motion.flying;
        self.swimming |= sample.motion.swimming;
        self.assisted |= sample.motion.assisted;
        self.last = Some(sample);
        if !sample.motion.grounded {
            return None;
        }
        let leg = self.departure.and_then(|start| {
            let (from, to) = (start.layer?, sample.layer?);
            if from == to || start.scope != sample.scope {
                return None;
            }
            let rise = (sample.point - start.point).dot(start.up);
            let travel = if self.flying {
                Travel::Fly
            } else if self.swimming {
                Travel::Swim
            } else if self.assisted {
                Travel::Assisted
            } else if !self.airborne && !self.uncertain {
                Travel::Walk
            } else if rise < -0.5 {
                Travel::Descend
            } else if rise > 0.5 {
                Travel::Ascend
            } else {
                Travel::Traverse
            };
            Some(Leg {
                from,
                to,
                entry: start.point,
                exit: sample.point,
                travel,
                metres: self.metres,
            })
        });
        *self = Self {
            last: Some(sample),
            departure: sample.layer.map(|_| sample),
            flying: sample.motion.flying,
            swimming: sample.motion.swimming,
            assisted: sample.motion.assisted,
            ..Self::default()
        };
        leg
    }
}
