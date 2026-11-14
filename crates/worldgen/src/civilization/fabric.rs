use super::*;
use crate::ColumnSample;

const ROOM_HALF: Vec2 = Vec2::new(6.0, 4.0);
const ROOF: f32 = 4.0;

fn lot_center(lot: u32) -> Vec2 {
    let lane_order = [0.0, -13.0, 13.0];
    Vec2::new(
        lane_order[(lot / 2) as usize],
        if lot.is_multiple_of(2) { -9.0 } else { 9.0 },
    )
}

fn home_access(lot: u32) -> crate::public_works::AccessWay {
    let entrance = lot_center(lot);
    crate::public_works::AccessWay { entrance, street: Vec2::new(entrance.x, 0.0), half_width: 1.0 }
}

fn work_access(x: f32, z: f32) -> crate::public_works::AccessWay {
    crate::public_works::AccessWay {
        entrance: Vec2::new(x, z), street: Vec2::new(x, 0.0), half_width: 0.85,
    }
}

impl Site {
    pub(super) fn standing(self, cfg: &WorldSampleConfig, offset: Vec2) -> Vec3 {
        let top = cfg.world_y(cfg.blocks_for_height(self.datum));
        Vec3::new(
            self.center.x + offset.x,
            top + 0.02,
            self.center.y + offset.y,
        )
    }

    pub(super) fn home(self, cfg: &WorldSampleConfig, lot: u32) -> Vec3 {
        self.standing(cfg, home_access(lot).entrance)
    }

    pub(super) fn daily_destination(
        self,
        cfg: &WorldSampleConfig,
        lot: u32,
        position: Vec3,
        at_home: bool,
    ) -> Vec3 {
        let local = position.xz() - self.center;
        let access = home_access(lot);
        let door = access.entrance;
        let stall = Vec2::new(2.0 * lot as f32 - 5.0, 0.0);
        let target = if at_home {
            if (local.x - door.x).abs() > 0.65 {
                access.street
            } else {
                door
            }
        } else if local.y.abs() > 0.7 {
            access.street
        } else {
            stall
        };
        self.standing(cfg, target)
    }

    fn vault(self) -> f32 {
        self.datum - self.history.burial.depth() - 4.0
    }

    fn masonry(self) -> B {
        match self.history.rock {
            B::Limestone | B::Sandstone | B::Basalt => self.history.rock,
            _ => B::StoneBricks,
        }
    }

    fn cache(self, cfg: &WorldSampleConfig) -> IVec3 {
        (Vec3::new(self.center.x + 3.5, self.vault() + 0.25, self.center.y) / cfg.voxel_meters())
            .floor()
            .as_ivec3()
    }

    fn underground(
        self,
        cfg: &WorldSampleConfig,
        p: IVec3,
        q: Vec2,
        y: f32,
        top: f32,
    ) -> Option<B> {
        let floor = self.vault();
        let v = y - floor;
        if !(-1.0..4.0).contains(&v) || !q.abs().cmplt(ROOM_HALF).all() || top < floor + 5.0 {
            return None;
        }

        // The cap is untouched natural terrain. The masonry, doorway, aisle
        // and remnants are the only generated changes inside this stratum.
        if p == self.cache(cfg) {
            return Some(B::SurveyCache);
        }
        let edge = q.x.abs() >= ROOM_HALF.x - 1.0 || q.y.abs() >= ROOM_HALF.y - 1.0;
        let partition = q.x.abs() < 1.0 && q.y.abs() > 1.0;
        Some(if v < 0.0 || v >= 3.0 || edge || partition {
            self.masonry()
        } else {
            B::Air
        })
    }

    fn surface(self, cfg: &WorldSampleConfig, q: Vec2, y: f32, natural_top: f32) -> Option<B> {
        let v = y - self.datum;
        if !(-5.0..ROOF + 1.0).contains(&v) {
            return None;
        }
        let occupied = self.history.households;
        for lot in 0..self.history.peak_households {
            let local = q - lot_center(lot);
            if local.x.abs() >= 5.0 || local.y.abs() >= 5.0 {
                continue;
            }
            let wall = local.x.abs() >= 4.0 || local.y.abs() >= 4.0;
            let door = local.x.abs() < 1.0 && local.y.signum() == -lot_center(lot).y.signum();
            if v < 0.0 {
                return Some(self.masonry());
            }
            if lot >= occupied {
                let surviving_wall =
                    wall && !door && v < if local.x + local.y > 0.0 { 2.0 } else { 1.0 };
                return surviving_wall.then(|| self.masonry());
            }
            let window = (1.0..2.0).contains(&v) && local.y.abs() < 1.0;
            return Some(if v >= ROOF {
                B::Planks
            } else if wall && !door && !window {
                self.masonry()
            } else {
                B::Air
            });
        }

        // A surviving survey pier marks the sealed storehouse under the lane.
        if (q.x - 5.0).abs() < 1.0 && (q.y - 2.0).abs() < 1.0 && v < 2.0 {
            return Some(B::StoneBricks);
        }

        // The same corridors guide residents and create the walkable fabric.
        // Roads and door paths survive abandonment just like the foundations.
        let street = q.y.abs() < 4.0;
        let home_path = (0..self.history.peak_households).any(|lot| home_access(lot).contains(q));
        let work_path = [-21.0, 21.0].into_iter().any(|x|
            [-5.0, 5.0].into_iter().any(|z| work_access(x, z).contains(q)));
        if (street || home_path || work_path) && self.history.peak_households > 0 {
            let blend = ((q.x.abs() - 16.0) / 6.0).clamp(0.0, 1.0);
            let grade = self.datum + (natural_top - self.datum) * blend;
            let surface = cfg.world_y(cfg.blocks_for_height(grade));
            return Some(if y < surface { self.masonry() } else { B::Air });
        }
        if occupied > 0 && q.x.abs() > 19.0 && (5.0..10.0).contains(&q.y.abs()) {
            let soil = matches!(
                self.history.livelihood,
                Livelihood::Floodplain | Livelihood::Pasture
            );
            let h = y - natural_top;
            let step = cfg.voxel_meters();
            if soil && (-step..0.0).contains(&h) {
                return Some(B::FungalSoil);
            }
            if soil && (0.0..step).contains(&h) {
                return Some(B::WheatCrop);
            }
            if !soil && (0.0..1.0).contains(&h) && q.y.abs() > 8.0 {
                return Some(self.masonry());
            }
        }
        None
    }
}

fn site_at(cfg: &WorldSampleConfig, point: Vec2) -> Option<Site> {
    let key = key_at(point);
    // Reject columns outside all candidate footprints before fetching a plan.
    let relative = (point - cell_center(key)).abs();
    if (relative.x - 132.0).abs() > 48.0 + FOOTPRINT.x || relative.y > 48.0 + FOOTPRINT.y {
        return None;
    }
    plan(cfg, key)
        .iter()
        .flatten()
        .find(|site| (point - site.center).abs().cmplt(FOOTPRINT).all())
        .copied()
}

pub(crate) fn clears_vegetation(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    enabled(cfg) && (site_at(cfg, Vec2::new(x, z)).is_some()
        || routes::contains(cfg, Vec2::new(x, z), 1.8))
}

pub fn visit_skylines(
    cfg: &WorldSampleConfig,
    point: Vec2,
    radius: f32,
    cancel: &std::sync::atomic::AtomicBool,
    mut emit: impl FnMut([Vec3; 9], f32, B, B),
) {
    if !enabled(cfg) {
        return;
    }
    let ring = [
        (-5., -5.),
        (0., -5.),
        (5., -5.),
        (5., 0.),
        (5., 5.),
        (0., 5.),
        (-5., 5.),
        (-5., 0.),
        (0., 0.),
    ];
    for key in keys_near(point, radius) {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        for site in plan(cfg, key).iter().flatten() {
            for lot in 0..site.history.households {
                let center = site.center + lot_center(lot);
                if point.distance_squared(center) <= radius * radius {
                    emit(
                        ring.map(|(x, z)| {
                            Vec3::new(center.x + x, site.datum + ROOF + 1.0, center.y + z)
                        }),
                        site.datum - 2.0,
                        site.masonry(),
                        B::Planks,
                    );
                }
            }
        }
    }
}

pub(crate) fn block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> Option<B> {
    if !enabled(cfg) {
        return None;
    }
    let depth = cfg.world_y(column.height - p.y);
    if !(-10.0..=17.0).contains(&depth) {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    if p.y == column.height && routes::contains(cfg, m.xz(), 1.15) {
        return Some(B::Gravel);
    }
    let site = site_at(cfg, m.xz())?;
    let offset = m.xz() - site.center;
    let natural_top = cfg.world_y(column.height + 1);
    site.underground(cfg, p, offset, m.y, natural_top)
        .or_else(|| site.surface(cfg, offset, m.y, natural_top))
}

pub fn archive(cfg: &WorldSampleConfig, p: IVec3, block: B) -> Option<History> {
    if block != B::SurveyCache || !enabled(cfg) {
        return None;
    }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let site = site_at(cfg, m.xz())?;
    (site.cache(cfg) == p).then_some(site.history)
}

pub fn archive_position(cfg: &WorldSampleConfig, id: u32) -> Option<IVec3> {
    settlement_by_id(cfg, id)?;
    let code = id - BASE;
    let key = IVec2::new((code >> 11) as i32 - 64, ((code >> 4) & 127) as i32 - 64);
    Some(plan(cfg, key)[((code >> 3) & 1) as usize]?.cache(cfg))
}

pub fn archive_text(cfg: &WorldSampleConfig, p: IVec3, block: B) -> Option<String> {
    let history = archive(cfg, p, block)?;
    if cfg.generator_version >= CHRONICLE_GENERATOR {
        let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
        let key = key_at(m.xz());
        let plan = plan(cfg, key);
        let slot = plan.iter().position(|s| s.is_some_and(|s| s.cache(cfg) == p))?;
        return plan.chronicle.as_ref()?.archive_text(encode(key, slot, 0));
    }
    let end = history.abandoned.map_or_else(
        || "The settlement survives above.".to_owned(),
        |year| format!("The last households left in year {year}."),
    );
    Some(format!("{} ledger. Founded in year {}, with {} households at its height. {end} {}",
        history.livelihood.name(), history.founded, history.peak_households, history.burial.description()))
}

pub fn location(cfg: &WorldSampleConfig, meters: Vec3) -> Option<&'static str> {
    if !enabled(cfg) {
        return None;
    }
    let site = site_at(cfg, meters.xz())?;
    if (meters.xz() - site.center).abs().cmplt(ROOM_HALF).all()
        && (site.vault() - 1.0..site.vault() + 4.0).contains(&meters.y)
    {
        return Some(match site.history.burial {
            Burial::Alluvium => "Silt-Sealed Storehouse",
            Burial::Scree => "Storehouse Beneath the Talus",
            Burial::Bedrock if site.history.cap_rock == B::Limestone => "Limestone Undercroft",
            Burial::Bedrock => "Rock-Cut Storehouse",
        });
    }
    (-5.0..=9.0).contains(&(meters.y - site.datum)).then(|| {
        if site.history.households == 0 {
            "Abandoned Hold"
        } else {
            site.history.livelihood.name()
        }
    })
}
