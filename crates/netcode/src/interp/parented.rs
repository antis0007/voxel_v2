//! Playback of bodies parented to an assembly (ADR 0040).
//!
//! A passenger's snapshots are in its carrier's grid, so blending them in world
//! space would cut chords across a turning hull. Both buffers are sampled at the
//! carrier's playback time and composed. A passenger buffer must hold snapshots of
//! one frame only; the caller starts a new one when the frame changes.
use super::SnapshotBuffer;
use crate::carrier::Pose;

/// World pose of `passenger` (local to `carrier`) at the carrier's playback time.
pub fn world_pose(carrier: &SnapshotBuffer<Pose>, passenger: &SnapshotBuffer<Pose>) -> Option<Pose> {
    let time = carrier.playback_time()?;
    Some(carrier.sample_at(time)?.compose(&passenger.sample_at(time)?))
}

#[cfg(test)]
mod tests {
    use super::super::Playback;
    use super::*;
    use crate::link::{Impairment, Link};
    use glam::{DVec3, Quat};

    /// Hull spinning at 6 rad/s far from the origin, drifting along x.
    fn hull(t: f64) -> Pose {
        Pose { position: DVec3::new(-70_000.0 + 8.0 * t, 12.0, 41_000.0), rotation: Quat::from_rotation_y(6.0 * t as f32) }
    }

    fn rider(t: f64) -> Pose {
        Pose { position: DVec3::new(20.0, 0.0, 0.5 * t.sin()), rotation: Quat::IDENTITY }
    }

    #[test]
    fn a_passenger_follows_a_spinning_hull_without_cutting_chords() {
        let mut link = Link::new(Impairment::BROADBAND, 3);
        let (mut carrier, mut local) = (SnapshotBuffer::new(Playback::DEFAULT), SnapshotBuffer::new(Playback::DEFAULT));
        let mut world = SnapshotBuffer::new(Playback::DEFAULT);
        let (mut next, mut now) = (0.0f64, 0.0f64);
        let (mut parented, mut naive) = (0.0f64, 0.0f64);
        while now < 10.0 {
            while next <= now {
                link.send((next * 1e6) as u64, next.to_le_bytes().to_vec());
                next += 1.0 / 30.0;
            }
            while let Some(p) = link.poll((now * 1e6) as u64) {
                let server = f64::from_le_bytes(p[..8].try_into().unwrap());
                carrier.push(now, server, hull(server));
                local.push(now, server, rider(server));
                world.push(now, server, hull(server).compose(&rider(server)));
            }
            carrier.advance(now);
            local.advance(now);
            world.advance(now);
            if now > 2.0 {
                let at = carrier.playback_time().unwrap();
                let truth = hull(at).compose(&rider(at)).position;
                parented = parented.max((world_pose(&carrier, &local).unwrap().position - truth).length());
                naive = naive.max((world.sample_at(at).unwrap().position - truth).length());
            }
            now += 1.0 / 144.0;
        }
        assert!(parented < 0.1, "parented error {parented}");
        assert!(naive > parented * 2.0, "naive {naive} parented {parented}");
    }
}
