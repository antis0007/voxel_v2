use super::*;

fn tumbling(t: f32) -> BodyState {
    let position = Vec3::new(-70_000.25 + t * 3.0, 40.0 - t * t * 4.0, 12_345.5);
    BodyState::quantise(position, Quat::from_rotation_y(t), Vec3::new(3.0, -8.0 * t, 0.0), Vec3::Y, 7)
}

#[test]
fn deltas_rebuild_the_exact_integers() {
    for (a, b) in [(BodyState::default(), tumbling(0.5)), (tumbling(0.5), tumbling(0.53)), (tumbling(1.0), tumbling(1.0))] {
        let mut out = Vec::new();
        b.write_delta(&a, &mut out);
        let mut r = Reader::new(&out);
        assert_eq!(BodyState::read_delta(&a, &mut r), Ok(b));
        assert!(r.done());
    }
}

#[test]
fn a_resting_body_costs_one_byte_and_a_moving_one_stays_small() {
    let mut out = Vec::new();
    tumbling(1.0).write_delta(&tumbling(1.0), &mut out);
    assert_eq!(out.len(), 1);
    out.clear();
    // One 30 Hz snapshot of a falling, turning body.
    tumbling(1.0 + 1.0 / 30.0).write_delta(&tumbling(1.0), &mut out);
    assert!(out.len() <= 18, "{} bytes", out.len());
}

#[test]
fn quantisation_keeps_far_positions_to_half_a_step() {
    let p = Vec3::new(-1_000_000.5, 300.25, 999_999.75);
    let back = BodyState::quantise(p, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO, 0).position();
    assert!((back - p).abs().max_element() <= 0.0625, "{back}");
}

#[test]
fn dead_reckoning_follows_velocity_and_spin() {
    let s = BodyState::quantise(Vec3::ZERO, Quat::IDENTITY, Vec3::new(2.0, 0.0, -1.0), Vec3::Y * 1.5, 0);
    let (p, q) = s.extrapolate(0.5);
    assert!((p - Vec3::new(1.0, 0.0, -0.5)).length() < 1e-3);
    assert!(q.angle_between(Quat::from_rotation_y(0.75)) < 1e-2);
}

#[test]
fn unknown_mask_bits_and_short_input_are_refused() {
    assert!(BodyState::read_delta(&BodyState::default(), &mut Reader::new(&[0x80])).is_err());
    assert!(BodyState::read_delta(&BodyState::default(), &mut Reader::new(&[POSITION, 2])).is_err());
}

fn golden() -> BodyState {
    BodyState { position: [-70_000 * 256, 40 * 256 + 3, 1 << 40], rotation: 0xdead_beef, velocity: [-5, 6, 7], angular: [1, -1, 0], state: 9, frame: 0 }
}

#[test]
fn world_bodies_cost_the_same_bytes_as_before_frames() {
    // Bytes from the encoder before ADR 0040 added the frame field.
    let mut out = Vec::new();
    golden().write_delta(&BodyState::default(), &mut out);
    assert_eq!(out, [31, 255, 191, 139, 17, 134, 160, 1, 128, 128, 128, 128, 128, 64, 239, 190, 173, 222, 9, 12, 14, 2, 1, 0, 9]);
    out.clear();
    let moved = BodyState { position: [-70_000 * 256 - 9, 40 * 256, 1 << 40], ..golden() };
    golden().write_delta(&moved, &mut out);
    assert_eq!(out, [1, 18, 6, 0]);
}

#[test]
fn frame_round_trips_and_costs_bytes_only_when_it_changes() {
    let aboard = BodyState { frame: 7, ..golden() };
    let far = BodyState { frame: u32::MAX, ..golden() };
    for (a, b) in [(golden(), aboard), (aboard, golden()), (aboard, aboard), (BodyState::default(), far), (far, aboard)] {
        let mut out = Vec::new();
        b.write_delta(&a, &mut out);
        let mut r = Reader::new(&out);
        assert_eq!(BodyState::read_delta(&a, &mut r), Ok(b));
        assert!(r.done());
    }
    let mut out = Vec::new();
    aboard.write_delta(&golden(), &mut out);
    assert_eq!(out, [FRAME, 7]);
    out.clear();
    aboard.write_delta(&aboard, &mut out);
    assert_eq!(out, [0]);
}

#[test]
fn assembly_poses_far_out_and_across_chunk_edges_round_trip() {
    // 32-cell chunks at 256 steps per voxel: one chunk is 8192 steps.
    let chunk = 32 * 256;
    let edges = [-(1i64 << 50), -chunk - 1, -chunk, -1, 0, chunk - 1, chunk, 1 << 50, i64::MIN / 2, i64::MAX / 2];
    let mut base = BodyState::default();
    for (n, &x) in edges.iter().enumerate() {
        let pose = BodyState { position: [x, -x, x / 3], frame: 0, state: 1 << 31, ..golden() };
        let aboard = BodyState { position: [x % chunk, 5, -x % chunk], frame: n as u32 + 1, ..golden() };
        for s in [pose, aboard] {
            let mut out = Vec::new();
            s.write_delta(&base, &mut out);
            let mut r = Reader::new(&out);
            assert_eq!(BodyState::read_delta(&base, &mut r), Ok(s));
            assert!(r.done());
            base = s;
        }
    }
}
