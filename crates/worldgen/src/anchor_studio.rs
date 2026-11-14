//! Text building designs and the shared compiler used by the editor and city review.
use crate::architecture::{
    Palette, Roof, Windows, Wing,
    plan::{Edge, Entrance, Plan, Room, RoomRole, Side},
};
use anyhow::{Context, Result, bail, ensure};
use engine_content::{ALL_BLOCK_IDS, BlockId as B};
use engine_voxel::blueprint_format;
use glam::Vec3;
use std::sync::OnceLock;

pub const MAX_PLAN_BYTES: usize = 16 * 1024;

pub struct Design {
    pub role: String,
    pub seed: u32,
    pub plan: Plan,
    pub floors: usize,
    pub stairs: Option<[usize; 2]>,
    pub windows: Windows,
    pub age: f32,
}

fn number<T: std::str::FromStr>(word: &str) -> Result<T> {
    word.parse()
        .map_err(|_| anyhow::anyhow!("Invalid number: {word}"))
}

fn material(name: &str) -> Result<B> {
    ALL_BLOCK_IDS
        .iter()
        .copied()
        .find(|b| format!("{b:?}") == name && b.is_solid())
        .with_context(|| format!("Unknown or non-solid building material: {name}"))
}

impl Design {
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(source.len() <= MAX_PLAN_BYTES, "Design is too large");
        let mut lines = source
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        ensure!(
            lines.next() == Some("VXPLAN 1"),
            "Expected a VXPLAN 1 design"
        );
        let mut d = Self {
            role: "Workshop".into(),
            seed: 1,
            plan: Plan {
                bays: [6, 4],
                bay_metres: 4.0,
                height_metres: 4.0,
                rooms: Vec::new(),
                portals: Vec::new(),
                entrances: Vec::new(),
                mirror_x: false,
                roof: Roof::Gable,
                palette: Palette::STONE,
            },
            floors: 1,
            stairs: None,
            windows: Windows::Panes,
            age: 0.0,
        };
        let mut seen = std::collections::HashSet::new();
        for line in lines {
            let words = line.split_whitespace().collect::<Vec<_>>();
            let [key, ..] = words.as_slice() else {
                continue;
            };
            ensure!(
                matches!(*key, "room" | "door" | "entrance") || seen.insert(*key),
                "Duplicate setting: {key}"
            );
            match words.as_slice() {
                [
                    "role",
                    role @ ("Apartments" | "Clinic" | "Offices" | "Workshop" | "Relay"),
                ] => d.role = (*role).into(),
                ["size", x, z] => d.plan.bays = [number(x)?, number(z)?],
                ["bay", m] => d.plan.bay_metres = number(m)?,
                ["height", m] => d.plan.height_metres = number(m)?,
                ["floors", n] => d.floors = number(n)?,
                ["seed", n] => d.seed = number(n)?,
                ["age", n] => d.age = number(n)?,
                ["stairs", x, z] => d.stairs = Some([number(x)?, number(z)?]),
                ["roof", roof] => {
                    d.plan.roof = match *roof {
                        "Flat" => Roof::Flat,
                        "Gable" => Roof::Gable,
                        "Hip" => Roof::Hip,
                        "SteepHip" => Roof::SteepHip,
                        "Lean" => Roof::Lean,
                        "Glass" => Roof::Glass,
                        _ => bail!("Unknown roof: {roof}"),
                    }
                }
                ["windows", style] => {
                    d.windows = match *style {
                        "None" => Windows::None,
                        "Panes" => Windows::Panes,
                        "Slits" => Windows::Slits,
                        "Glazed" => Windows::Glazed,
                        _ => bail!("Unknown window style: {style}"),
                    }
                }
                ["palette", wall, frame, plinth, floor, roof, glass] => {
                    d.plan.palette = Palette {
                        wall: material(wall)?,
                        frame: material(frame)?,
                        plinth: material(plinth)?,
                        floor: material(floor)?,
                        roof: material(roof)?,
                        glass: material(glass)?,
                    }
                }
                ["room", role, x0, z0, x1, z1] => d.plan.rooms.push(Room {
                    role: match *role {
                        "Passage" => RoomRole::Passage,
                        "Hall" => RoomRole::Hall,
                        "Shrine" => RoomRole::Shrine,
                        "Library" => RoomRole::Library,
                        "Workshop" => RoomRole::Workshop,
                        "Store" => RoomRole::Store,
                        "Crypt" => RoomRole::Crypt,
                        "Living" => RoomRole::Living,
                        _ => bail!("Unknown room use: {role}"),
                    },
                    bounds: [number(x0)?, number(z0)?, number(x1)?, number(z1)?],
                }),
                ["door", axis @ ("X" | "Z"), x, z] => {
                    let (x, z) = (number(x)?, number(z)?);
                    d.plan.portals.push(if *axis == "X" {
                        Edge::X { x, z }
                    } else {
                        Edge::Z { x, z }
                    });
                }
                ["entrance", side, bay] => d.plan.entrances.push(Entrance {
                    side: match *side {
                        "Front" => Side::Front,
                        "Back" => Side::Back,
                        "Left" => Side::Left,
                        "Right" => Side::Right,
                        _ => bail!("Unknown entrance side: {side}"),
                    },
                    bay: number(bay)?,
                }),
                _ => bail!("Invalid design line: {line}"),
            }
        }
        ensure!(
            d.plan.bays.into_iter().all(|n| (1..=24).contains(&n)) && (1..=24).contains(&d.floors),
            "Use 1–24 bays per side and 1–24 floors"
        );
        ensure!(
            (3.6..=8.0).contains(&d.plan.bay_metres) && (3.6..=8.0).contains(&d.plan.height_metres),
            "Bay and floor sizes must be between 3.6 and 8 metres"
        );
        ensure!(
            (0.0..=0.6).contains(&d.age),
            "Weathering must be between 0 and 0.6"
        );
        d.compile(1.0, d.floors)?;
        Ok(d)
    }

    pub fn compile(&self, scale: f32, floors: usize) -> Result<Wing> {
        self.plan
            .compile_floors(
                [0.0, 0.0],
                self.seed,
                scale,
                floors,
                self.windows,
                self.stairs,
                self.age,
            )
            .map_err(anyhow::Error::msg)
    }

    pub fn roof_space(&self) -> f32 {
        match self.plan.roof {
            Roof::Flat => 2.3,
            Roof::Lean => 0.3 * self.plan.bays[1] as f32 * self.plan.bay_metres + 1.0,
            Roof::SteepHip => 9.0,
            _ => 6.5,
        }
    }

    /// Bakes the same sampled Wing as the city review. Preserve the blueprint cell order.
    pub fn blueprint(&self, scale: f32) -> Result<Vec<u8>> {
        ensure!(
            matches!(scale, 1.0 | 0.5 | 0.25),
            "Choose 1, 0.5, or 0.25 metres per block"
        );
        let wing = self.compile(scale, self.floors)?;
        let pad = (1.0 / scale).ceil();
        let dims = [
            (wing.width() / scale).ceil() + pad * 2.0,
            ((wing.top() + self.roof_space()) / scale).ceil(),
            (wing.depth() / scale).ceil() + pad * 2.0,
        ];
        ensure!(
            dims.iter().all(|&d| d <= blueprint_format::MAX_SIDE as f32),
            "Too large at this resolution. Choose bigger blocks or fewer bays/floors."
        );
        let [sx, sy, sz] = dims.map(|v| v as u16);
        let volume = usize::from(sx) * usize::from(sy) * usize::from(sz);
        ensure!(
            volume <= blueprint_format::MAX_VOLUME,
            "Too many blocks at this resolution. Choose bigger blocks or fewer bays/floors."
        );
        let mut cells = Vec::with_capacity(volume);
        let mut last = 0;
        for y in 0..sy {
            let start = cells.len();
            for z in 0..sz {
                for x in 0..sx {
                    let p = Vec3::new(
                        (f32::from(x) - pad + 0.5) * scale,
                        (f32::from(y) + 0.5) * scale,
                        (f32::from(z) - pad + 0.5) * scale,
                    );
                    cells.push(wing.sample(p, scale, 0.0).unwrap_or(B::Air));
                }
            }
            if cells[start..].iter().any(|b| *b != B::Air) {
                last = y + 1;
            }
        }
        ensure!(last > 0, "The design contains no sampled blocks");
        cells.truncate(usize::from(last) * usize::from(sx) * usize::from(sz));
        blueprint_format::encode([sx, last, sz], &cells).context("Invalid blueprint dimensions")
    }
}

static CITY_REVIEW: OnceLock<Design> = OnceLock::new();

/// Set by the application's explicit, isolated city review mode before generation starts.
pub fn install_city_review(design: Design) -> Result<()> {
    CITY_REVIEW
        .set(design)
        .map_err(|_| anyhow::anyhow!("An Anchor Studio design is already active"))
}

pub(crate) fn city_review(role: &str) -> Option<&'static Design> {
    CITY_REVIEW.get().filter(|design| design.role == role)
}

/// Bake the streets around a parcel using the live Anchor sampler in a scratch process.
pub fn city_blueprint(design: Design) -> Result<Vec<u8>> {
    use crate::{WorldSampleConfig, WorldType, ruins, sample_column};
    use engine_core::VoxelScalePreset;
    use glam::IVec3;

    let kind = if design.role == "Workshop" { ruins::Kind::Port } else { ruins::Kind::Offices };
    install_city_review(design)?;
    let cfg = WorldSampleConfig {
        seed: 424242, generator_version: engine_voxel::CURRENT_GENERATOR_VERSION,
        world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter, planet: None,
    };
    let (district, eye, aim) = [1450.0, 2500.0, 3900.0].into_iter().find_map(|radius| {
        ruins::near(&cfg, Vec3::ZERO, radius).into_iter().filter(|d| d.kind == kind)
            .filter_map(|d| {
                let (eye, aim) = d.review_view("studio", cfg.generator_version)?;
                Some((d, eye, aim))
            }).min_by(|(a, ..), (b, ..)| a.centre.length_squared().total_cmp(&b.centre.length_squared()))
    }).context("This building did not fit a city parcel. Try a narrower or shallower plan.")?;
    let sx = 88;
    let sz = 88;
    let sy = ((eye.y - district.datum + 49.0).ceil() as u16).clamp(54, 230);
    let origin = IVec3::new(aim.x.floor() as i32 - i32::from(sx / 2),
        district.datum.floor() as i32 - 8, aim.z.floor() as i32 - i32::from(sz / 2));
    let ground: Vec<_> = (0..sz).flat_map(|z| (0..sx).map(move |x| (x,z)))
        .map(|(x,z)| sample_column(&cfg, origin.x + i32::from(x), origin.z + i32::from(z)))
        .collect();
    let stride = usize::from(sx) * usize::from(sz);
    let mut cells = Vec::with_capacity(stride * usize::from(sy));
    let mut last = 0;
    for y in 0..sy {
        let start = cells.len();
        for (i, column) in ground.iter().enumerate() {
            let p = origin + IVec3::new((i % usize::from(sx)) as i32, i32::from(y),
                (i / usize::from(sx)) as i32);
            let terrain = cfg.world_y(column.height + 1);
            let block = ruins::block(&cfg, p, p.as_vec3() + Vec3::splat(0.5), terrain)
                .unwrap_or(if p.y <= column.height { column.surface } else { B::Air });
            cells.push(block);
        }
        if cells[start..].iter().any(|b| *b != B::Air) {
            last = y + 1;
        }
    }
    cells.truncate(stride * usize::from(last));
    blueprint_format::encode([sx, last, sz], &cells).context("Invalid city preview dimensions")
}
