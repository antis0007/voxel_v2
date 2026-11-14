//! Fault-bounded subsidence at a shield summit. Distances and throws are metres.
use super::{BlockId, DVec2, DVec3, SEA, Shaped, noise_at, smooth};

// Covers the outer fault polygon and the maximum displacement of its trace.
pub(super) const REACH_M: f64 = 950.0;

struct Block {
    centre: DVec2,
    faults: [(DVec2, f64); 8],
    throw_m: f64,
    scarp_m: f64,
}

impl Block {
    fn new(seed: u32, centre: DVec2, axes: DVec2, throw_m: f64, scarp_m: f64) -> Self {
        let roll = |i| f64::from(crate::hash_u32(seed.wrapping_add(i))) / f64::from(u32::MAX);
        let turn = (roll(91) - 0.5) * 0.68;
        let faults = std::array::from_fn(|i| {
            let angle =
                turn + i as f64 * std::f64::consts::FRAC_PI_4 + (roll(i as u32 * 37) - 0.5) * 0.36;
            let (sine, cosine) = angle.sin_cos();
            let normal = DVec2::new(cosine, sine);
            let support = (normal * axes).length() * (0.87 + roll(i as u32 * 43 + 7) * 0.22);
            (normal, support)
        });
        Self {
            centre,
            faults,
            throw_m,
            scarp_m,
        }
    }

    fn distance(&self, p: DVec2) -> f64 {
        let local = p - self.centre;
        self.faults
            .iter()
            .map(|&(normal, support)| local.dot(normal) - support)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn inside(&self, distance: f64) -> f64 {
        1.0 - smooth(-self.scarp_m, 0.0, distance)
    }
}

pub(super) struct Collapse {
    rim: Block,
    bench: Block,
    pit: Block,
    tilt: DVec2,
    vent: DVec2,
    seed: u32,
}

impl Collapse {
    pub(super) fn new(seed: u32, deep: bool) -> Self {
        let polarity = if seed & 1 == 0 { 1.0 } else { -1.0 };
        Self {
            rim: Block::new(
                seed,
                DVec2::new(-35.0, 8.0),
                DVec2::new(645.0, 509.0),
                if deep { 34.0 } else { 23.0 },
                if deep { 21.0 } else { 33.0 },
            ),
            bench: Block::new(
                seed ^ 0x1f32,
                DVec2::new(32.0, polarity * 58.0),
                DVec2::new(508.0, 392.0),
                if deep { 19.0 } else { 13.0 },
                if deep { 22.0 } else { 27.0 },
            ),
            pit: Block::new(
                seed ^ 0x8719,
                DVec2::new(93.0, polarity * 63.0),
                DVec2::new(404.0, 316.0),
                if deep { 45.0 } else { 38.0 },
                if deep { 28.0 } else { 36.0 },
            ),
            tilt: DVec2::new(-0.010, polarity * -0.017),
            vent: DVec2::new(167.0, polarity * 106.0),
            seed,
        }
    }

    pub(super) fn shape(&self, p: DVec2, body: DVec3, summit_m: f64, out: &mut Shaped) {
        // Fault trace offsets operate in metres. Broad headwall retreat and
        // smaller joints break the straight faces, with different traces on
        // each block instead of equally spaced terraces.
        let retreat = |seed| {
            (noise_at(body / 164.0, seed) - 0.5) * 92.0
                + (noise_at(body / 52.0, seed ^ 0x2fd7) - 0.5) * 28.0
        };
        let joint = retreat(self.seed);
        let rim_distance = self.rim.distance(p) + joint;
        if rim_distance > 12.0 {
            return;
        }
        let rim = self.rim.inside(rim_distance);
        let bench = self
            .bench
            .inside(self.bench.distance(p) + retreat(self.seed ^ 0x8ac1));
        let pit = self
            .pit
            .inside(self.pit.distance(p) + 0.65 * retreat(self.seed ^ 0x16ef));
        let tilted_floor =
            summit_m + p.dot(self.tilt) + (noise_at(body / 139.0, self.seed ^ 0x3e41) - 0.5) * 5.0;
        let fissure_distance = ((p - self.vent) / DVec2::new(99.0, 44.0)).length() + joint / 320.0;
        let fissure = 1.0 - smooth(0.32, 1.0, fissure_distance);
        let floor = tilted_floor
            - self.rim.throw_m
            - bench * (self.bench.throw_m + pit * (self.pit.throw_m + 10.0 * fissure));
        out.metres += (floor - out.metres).min(0.0) * rim;
        out.active_caldera |= rim_distance < 0.0;

        let wall_grade = |weight: f64, block: &Block| {
            6.0 * weight * (1.0 - weight) * block.throw_m / block.scarp_m
        };
        let slope = wall_grade(rim, &self.rim)
            .max(wall_grade(bench, &self.bench) * rim)
            .max(wall_grade(pit, &self.pit) * rim * bench);
        out.slope = out.slope.max(slope);
        out.heat = out
            .heat
            .max(rim * (0.23 + bench * pit * (0.20 + 0.55 * fissure)));
        // Local fissure ponding leaves the rest of the collapse floor exposed.
        let head = summit_m + self.vent.dot(self.tilt)
            - self.rim.throw_m
            - self.bench.throw_m
            - self.pit.throw_m
            - 5.0;
        if rim * bench * pit > 0.98 && fissure > 0.12 && head > SEA && out.metres < head {
            out.lake = Some((head, BlockId::Lava));
        } else if rim > 0.05 {
            out.crust = Some(if slope > 0.46 || bench > 0.78 {
                BlockId::Basalt
            } else {
                BlockId::Ash
            });
        }
    }
}
