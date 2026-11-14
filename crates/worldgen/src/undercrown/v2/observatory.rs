//! Finite water and a lower receiving basin beside the observatory.
use crate::BlockId as B;
use glam::{IVec3, Vec3};

pub const VERSION: u32 = 60;
pub const ENCOUNTER_VERSION: u32 = 63;
/// Reserved authored identity within the existing encounter record encoding.
/// These grid coordinates lie beyond the supported saved actor domain.
pub const ENCOUNTER_ID: IVec3 = IVec3::new(99_999, 99_999, 5);

#[derive(Clone, Copy)]
pub struct Gallery {
    origin: Vec3,
}

fn inside(p: Vec3, lo: Vec3, hi: Vec3) -> bool {
    p.cmpge(lo).all() && p.cmplt(hi).all()
}

impl Gallery {
    pub(super) fn new(anchor: Vec3) -> Self {
        Self {
            origin: anchor + Vec3::new(108.0, -15.0, -42.0),
        }
    }

    pub fn lens_contacts(self, voxel_m: f32) -> [Vec3; 4] {
        [(4.5, 5.5), (7.5, 5.5), (5.5, 4.5), (5.5, 7.5)]
            .map(|(x, z)| self.origin + Vec3::new(x, voxel_m * 0.5, z))
    }

    pub fn reliquary(self) -> Vec3 {
        self.origin + Vec3::new(6.0, 0.0, 1.25)
    }

    pub fn guardian(self) -> Vec3 {
        self.origin + Vec3::new(8.0, 0.0, 6.5)
    }

    pub fn review_view(self) -> (Vec3, Vec3) {
        (
            self.origin + Vec3::new(1.5, 1.7, 6.0),
            self.origin + Vec3::new(7.0, 0.8, 6.0),
        )
    }

    pub fn bounds(self) -> (Vec3, Vec3) {
        (
            self.origin + Vec3::new(-1.0, -3.0, -1.0),
            self.origin + Vec3::new(23.0, 10.0, 13.0),
        )
    }

    pub fn sample(self, m: Vec3) -> Option<B> {
        let p = m - self.origin;
        let room = inside(p, Vec3::new(0.0, -1.0, 0.0), Vec3::new(13.0, 10.0, 12.0));
        let basin = inside(p, Vec3::new(14.0, -3.0, 2.0), Vec3::new(23.0, 4.0, 10.0));
        let channel = inside(p, Vec3::new(10.0, -2.0, 4.0), Vec3::new(16.0, 2.0, 8.0));
        if !room && !basin && !channel {
            return None;
        }

        // A breakable stone plug separates the pool from the lower outlet.
        // No mechanism bit removes water or changes the source/sink geometry.
        if inside(p, Vec3::new(11.0, -1.0, 5.0), Vec3::new(16.0, 1.0, 7.0)) {
            return Some(B::Air);
        }
        if basin {
            return Some(
                if inside(p, Vec3::new(15.0, -2.0, 3.0), Vec3::new(22.0, 3.0, 9.0)) {
                    B::Air
                } else {
                    B::StoneBricks
                },
            );
        }
        if channel && !room {
            return Some(B::StoneBricks);
        }
        if channel && p.y < 0.0 {
            return Some(B::StoneBricks);
        }

        let pool = inside(p, Vec3::new(3.0, 0.0, 3.0), Vec3::new(10.0, 1.0, 9.0));
        let rim = inside(p, Vec3::new(2.0, 0.0, 2.0), Vec3::new(11.0, 1.0, 10.0));
        let pedestal = inside(p, Vec3::new(5.4, 0.0, 5.4), Vec3::new(6.6, 0.6, 6.6));
        if pedestal {
            return Some(B::SilverBlock);
        }
        if pool {
            return Some(B::Water);
        }
        if rim {
            return Some(B::StoneBricks);
        }

        if inside(p, Vec3::new(2.0, -1.0, 2.0), Vec3::new(11.0, 0.0, 10.0)) {
            return Some(B::StoneBricks);
        }
        None
    }
}
