use super::Vec3;

pub(super) fn point(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let mut point=from.lerp(to,t);
    // Interpolating identical heights can round into the supporting floor.
    point.y=from.y;
    point
}

#[cfg(test)] mod tests {
    use super::*;
    #[test]
    fn level_samples_keep_exact_height_and_grid_row() {
        for v in [1.0f32,0.5,0.25,0.125,0.1,0.01] {
            for row in [-4096,-1024,-37,-18,-1,0,1,18,37,1024,4096] {
                let y=row as f32*v;
                for (dx,dz) in [(2.0,0.0),(-2.0,0.0),(0.0,2.0),(2.5,-1.5)] {
                    let from=Vec3::new(-12.5,y,4.5);
                    let to=from+Vec3::new(dx,0.0,dz);
                    let steps=(from.distance(to)/0.1).ceil().max(1.0) as u32;
                    for i in 0..=steps {
                        let t=i as f32/steps as f32;
                        let p=point(from,to,t);
                        assert_eq!(p.y.to_bits(),y.to_bits());
                        assert_eq!((p.y/v).floor(),(y/v).floor());
                        assert_eq!(p.x.to_bits(),from.lerp(to,t).x.to_bits());
                        assert_eq!(p.z.to_bits(),from.lerp(to,t).z.to_bits());
                    }
                }
            }
        }
    }
    #[test]
    fn eighteen_metre_floor_regression() {
        let from=Vec3::new(27.5,18.0,4.5);
        let to=Vec3::new(29.5,18.0,4.5);
        let p=point(from,to,2.0/20.0);
        assert_eq!(p.y,18.0);
        assert_eq!(p.floor().y,18.0);
    }
}
