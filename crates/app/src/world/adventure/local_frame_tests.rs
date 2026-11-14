use super::*;

const UPS: [Vec3; 5] = [Vec3::Y, Vec3::X, Vec3::NEG_X, Vec3::NEG_Y, Vec3::Z];

#[test]
fn facing_holds_where_the_shell_floor_is_a_wall() {
    // Player separated from the enemy along world Y: XZ flattening read zero here.
    for up in UPS {
        let side = if up.dot(Vec3::Y).abs() > 0.5 { Vec3::X } else { Vec3::Y };
        let d = side * 3.0 + up * 0.4;
        let yaw = local_yaw(up, along_floor(up, d));
        let forward = yaw_forward(up, yaw);
        assert!(forward.dot(up).abs() < 1e-4, "{up:?}: forward leaves the floor");
        let facing = forward.dot(along_floor(up, d).normalize_or_zero());
        assert!(facing > 0.99, "{up:?}: facing {facing}");
    }
}

#[test]
fn floor_projection_matches_xz_flattening_on_level_ground() {
    let v = Vec3::new(1.5, -2.0, 0.25);
    assert_eq!(along_floor(Vec3::Y, v), Vec3::new(1.5, 0.0, 0.25));
    assert_eq!(yaw_forward(Vec3::Y, 0.7), Vec3::new(-0.7f32.sin(), 0.0, -0.7f32.cos()));
}

#[test]
fn floor_projection_drops_only_the_up_component() {
    for up in UPS {
        let v = Vec3::new(0.3, 2.0, -1.1);
        let p = along_floor(up, v);
        assert!(p.dot(up).abs() < 1e-5);
        assert!((p + up * v.dot(up) - v).length() < 1e-5);
    }
}
