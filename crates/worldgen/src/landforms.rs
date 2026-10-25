//! Generator 39: continuous drainage relief and deliberate openings into rock.
//! Metre-space fields are shared by resident and horizon columns. No recursive
//! terrain queries; expensive horizontal terms are cached in CaveColumn.
use crate::{WorldSampleConfig, hash2, smoothstep, value_noise2};

const CELL: f32 = 112.0;

#[derive(Clone, Copy, Default)]
pub(crate) struct Opening {
    // Local coordinates of a sloping, flared tunnel. Zero radius disables it.
    along: f32,
    across: f32,
    radius: f32,
    sink: f32,
}

pub(crate) fn opening(cfg: &WorldSampleConfig, x: f32, z: f32) -> Opening {
    if cfg.generator_version < 39 {
        return Opening::default();
    }
    // Centres stay 30 m inside their cell; the complete mouth and tunnel fit
    // there too, so only one candidate is needed and cell edges stay untouched.
    let ix = (x / CELL).floor() as i32;
    let iz = (z / CELL).floor() as i32;
    let h = |salt| hash2(ix, iz, cfg.seed.wrapping_add(salt));
    if h(12001)
        < if cfg.generator_version >= 40 {
            0.80
        } else {
            0.27
        }
    {
        return Opening::default();
    }
    let cx = (ix as f32 + 0.5) * CELL + (h(12003) - 0.5) * 42.0;
    let cz = (iz as f32 + 0.5) * CELL + (h(12007) - 0.5) * 42.0;
    if cx.hypot(cz) < 175.0 {
        return Opening::default();
    }
    let (dx, dz) = (x - cx, z - cz);
    if dx.abs() > 30.0 || dz.abs() > 30.0 {
        return Opening::default();
    }
    let (sin, cos) = (h(12011) * std::f32::consts::TAU).sin_cos();
    Opening {
        along: dx * cos + dz * sin,
        across: -dx * sin + dz * cos,
        radius: 2.8 + h(12013) * 2.0,
        sink: if cfg.generator_version < 40 && h(12017) > 0.68 {
            1.0
        } else {
            0.0
        },
    }
}

impl Opening {
    fn depression(self) -> f32 {
        if self.radius == 0.0 {
            return 0.0;
        }
        if self.sink > 0.0 {
            let r = (self.along / 1.25).hypot(self.across) / (self.radius * 2.8);
            // Broad weathered lip, steep inner bowl; no discontinuous rim.
            return (1.0 - smoothstep(0.12, 1.0, r)) * 10.0;
        }
        let channel = 1.0 - smoothstep(self.radius, self.radius * 2.8, self.across.abs());
        channel * (1.0 - smoothstep(0.0, 18.0, self.along.abs())) * 3.5
    }

    pub(crate) fn carves(self, depth: f32, above_water: f32) -> bool {
        if self.radius == 0.0 || above_water < 11.0 {
            return false;
        }
        if self.sink > 0.0 {
            // The neck opens to a larger chamber, rather than a blind surface pit.
            let radial = (self.along / 1.25).hypot(self.across);
            let radius = self.radius * (0.8 + smoothstep(7.0, 15.0, depth) * 1.7);
            return depth < 23.0 && radial < radius * (1.0 - smoothstep(18.0, 23.0, depth));
        }
        let t = self.along;
        if !(-4.0..26.0).contains(&t) {
            return false;
        }
        // Mouth, descending passage, then a chamber that intersects the deeper
        // natural cave field. Horizontal bend gives a readable dark threshold.
        let bend = (t * 0.12).sin() * 1.8;
        let centre_depth = 1.0 + t.max(0.0) * 0.65;
        let cap = smoothstep(-4.0, 0.0, t) * (1.0 - smoothstep(21.0, 26.0, t));
        let radius = self.radius * cap * (1.0 + smoothstep(9.0, 19.0, t) * 0.65);
        radius > 0.0
            && ((self.across - bend) / radius).powi(2)
                + ((depth - centre_depth) / (radius * 0.85)).powi(2)
                < 1.0
    }
}

pub(crate) fn relief(cfg: &WorldSampleConfig, x: f32, z: f32, height: f32) -> (f32, f32) {
    let gate = smoothstep(130.0, 320.0, x.hypot(z)) * smoothstep(19.0, 42.0, height);
    if gate == 0.0 {
        return (0.0, 0.0);
    }
    let seed = cfg.seed;
    let warp = (value_noise2(x / 270.0, z / 270.0, seed.wrapping_add(12101)) - 0.5) * 95.0;
    let channel = (value_noise2(
        (x + warp) / 200.0,
        (z - warp) / 200.0,
        seed.wrapping_add(12103),
    ) - 0.5)
        .abs();
    let mountain = smoothstep(55.0, 150.0, height);
    // Broad valley shoulders surround a much narrower rock gorge. A second
    // field opens/closes the gorge gradually, leaving passes between cliffs.
    let valley = 1.0 - smoothstep(0.02, 0.22, channel);
    let gorge = 1.0 - smoothstep(0.006, 0.035, channel);
    let exposure = smoothstep(
        0.39,
        0.64,
        value_noise2(x / 330.0, z / 330.0, seed.wrapping_add(12107)),
    );
    let spine = smoothstep(0.15, 0.34, channel);
    let peaks = value_noise2((x + warp) / 85.0, z / 85.0, seed.wrapping_add(12109));
    let uplift = spine * mountain * (10.0 + 24.0 * peaks);
    let incision = if cfg.generator_version >= 40 {
        // Broad lowland valleys, with narrow gorges confined to mountain rock.
        valley * (3.0 + mountain * 25.0) + gorge * exposure * mountain * 24.0
    } else {
        valley * (5.0 + mountain * 25.0) + gorge * exposure * (9.0 + mountain * 19.0)
    };
    let hollow = opening(cfg, x, z).depression();
    let delta = (uplift - incision - hollow) * gate;
    // Drainage cannot gouge new sea-level water stripes inland.
    let result = (height + delta).max(17.0) - height;
    (
        result,
        gate * (valley * 0.12 + gorge * exposure * 0.65 + spine * mountain * 0.2),
    )
}
