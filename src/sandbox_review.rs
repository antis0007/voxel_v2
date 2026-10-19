//! Opt-in in-game acceptance scenario. Uses the real movement, simulation,
//! checkpoint worker, loader and renderer; never runs during ordinary play.
use crate::{
    player::{FlyMode, Input, Player},
    simulation::{WorldSimulation, sandbox::ThermalCell},
    world::{
        BlockId as B, DayNightCycle, World,
        storage::{SaveService, SavedSession},
    },
};
use glam::{DVec3, IVec3, Vec3};
use std::{collections::HashSet, sync::mpsc};
#[derive(Default)]
pub struct SandboxReview {
    elapsed: f32,
    phase: u8,
    shots: u8,
    precision_samples: usize,
    moved_samples: usize,
    before: DVec3,
    rounded: Vec3,
    expected: Vec<u8>,
    expected_pos: DVec3,
    save: Option<SaveService>,
    loading: Option<mpsc::Receiver<anyhow::Result<SavedSession>>>,
    saved: bool,
    mass: u32,
    smoke: u32,
    fire_seen: bool,
    spread_seen: bool,
    body_seen: bool,
}
fn flag(v: bool) -> &'static str {
    if v { "PASS" } else { "FAIL" }
}
impl SandboxReview {
    pub fn install(&mut self, w: &mut World, p: &mut Player) {
        let mut edits = Vec::new();
        for y in 96..110 {
            for z in -6..14 {
                for x in -14..20 {
                    let solid = y == 96
                        || ((x == 6 || x == 9 || x == 12 || z == -3 || z == 3 || y == 107)
                            && (6..=12).contains(&x)
                            && (-3..=3).contains(&z));
                    edits.push((
                        IVec3::new(x, y, z),
                        if solid { B::StoneBricks } else { B::Air },
                    ));
                }
            }
        }
        edits.extend([
            (IVec3::new(-8, 97, 0), B::Log),
            (IVec3::new(-7, 97, 0), B::Fire),
            (IVec3::new(-9, 97, 0), B::Fire),
            (IVec3::new(7, 97, 0), B::Smoke),
            (IVec3::new(10, 97, 0), B::Steam),
            (IVec3::new(0, 104, 8), B::RigidCore),
            (IVec3::new(1, 104, 8), B::Planks),
            (IVec3::new(2, 104, 8), B::Planks),
        ]);
        // A visible distant arch beyond the resident view radius.
        let h = crate::world::distant_surface(&w.sample_config(), 320, 0)
            .0
            .ceil() as i32;
        for x in 304..336 {
            for y in h..h + 32 {
                for z in -4..4 {
                    let arch = x < 310 || x >= 330 || y >= h + 25;
                    edits.push((
                        IVec3::new(x, y, z),
                        if arch { B::StoneBricks } else { B::Air },
                    ));
                }
            }
        }
        w.set_many(&edits);
        w.sandbox.set(
            IVec3::new(7, 97, 0),
            ThermalCell {
                temperature: 80.0,
                fuel: 0.0,
                gas: 2,
            },
        );
        w.sandbox.set(
            IVec3::new(10, 97, 0),
            ThermalCell {
                temperature: 120.0,
                fuel: 0.0,
                gas: 4,
            },
        );
        *p = Player::new();
        p.flying = true;
        p.fly_mode = FlyMode::Noclip;
        Self::camera(p, Vec3::new(16.0, 108.0, 22.0), Vec3::new(0.0, 99.0, 1.0));
        self.mass = 4;
        self.smoke = 2;
        println!(
            "[sandbox-review] installed fire/logs, sealed gases, three-voxel rigid body, distant arch at320m"
        );
    }
    fn camera(p: &mut Player, pos: Vec3, target: Vec3) {
        p.pos = pos;
        p.precise_pos = pos.as_dvec3();
        p.vel = Vec3::ZERO;
        let d = (target - pos).normalize();
        p.yaw = d.x.atan2(-d.z);
        p.pitch = d.y.asin();
        p.flying = true;
        p.fly_mode = FlyMode::Noclip;
    }
    pub fn input(&mut self, p: &mut Player, input: &mut Input, dt: f32) {
        if self.save.is_none() {
            self.elapsed += dt;
        }
        input.clear();
        if (25.0..26.0).contains(&self.elapsed) {
            p.vel = Vec3::X * 0.25;
        } else if (26.0..30.0).contains(&self.elapsed) {
            input.forward = true;
        }
        self.before = p.precise_pos;
        self.rounded = p.pos;
    }
    pub fn observe(&mut self, p: &Player) {
        if (25.0..30.0).contains(&self.elapsed) {
            if p.precise_pos != self.before {
                self.moved_samples += 1;
                if p.pos == self.rounded {
                    self.precision_samples += 1;
                }
            }
        }
    }
    pub fn tick(
        &mut self,
        w: &mut World,
        p: &mut Player,
        day: &DayNightCycle,
        sim: &mut WorldSimulation,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) -> (bool, Option<String>) {
        self.fire_seen |= w.get(IVec3::new(-7, 97, 0)) == B::Fire;
        self.spread_seen |= w.get(IVec3::new(-8, 97, 0)) != B::Log;
        self.body_seen |= !w.sandbox.bodies.is_empty();
        let mut shot = None;
        let mut reloaded = false;
        if self.elapsed >= 12.0 && self.phase == 0 {
            let mut steam_mass = 0;
            let mut smoke_mass = 0;
            let mut smoke_y = 0;
            for y in 97..107 {
                for z in -2..3 {
                    for x in 7..12 {
                        let q = IVec3::new(x, y, z);
                        match w.get(q) {
                            B::Water => steam_mass += w.water_volume(q) as u32,
                            B::Steam => steam_mass += w.sandbox.cell(q).gas as u32,
                            B::Smoke => {
                                smoke_mass += w.sandbox.cell(q).gas as u32;
                                smoke_y = smoke_y.max(y);
                            }
                            _ => {}
                        }
                    }
                }
            }
            let moved = w
                .sandbox
                .bodies
                .iter()
                .any(|b| b.occupied.len() == 3 && b.occupied.iter().all(|(q, _)| q.y == 97));
            println!(
                "[sandbox-review] fire={} spread={} rigid={} body_seen={} gas_mass={} steam_water_units={} smoke_units={} gas_rise={}",
                flag(
                    self.fire_seen
                        && (w.get(IVec3::new(-7, 97, 0)) != B::Fire
                            || w.sandbox.cell(IVec3::new(-7, 97, 0)).fuel < 80.0)
                ),
                flag(self.spread_seen),
                flag(moved),
                self.body_seen,
                flag(steam_mass == self.mass && smoke_mass == self.smoke),
                steam_mass,
                smoke_mass,
                flag(smoke_y > 97)
            );
            println!(
                "[sandbox-review] original_fire_fuel_at12={}",
                w.sandbox.cell(IVec3::new(-7, 97, 0)).fuel
            );
            self.phase = 1;
            shot = Some("12-physics".into());
        }
        if self.elapsed >= 16.0 && self.phase == 1 {
            let h = crate::world::distant_surface(&w.sample_config(), 320, 0).0;
            Self::camera(
                p,
                Vec3::new(320.0, h + 22.0, -200.0),
                Vec3::new(320.0, h + 15.0, 0.0),
            );
            self.phase = 2;
        }
        if self.elapsed >= 22.0 && self.phase == 2 {
            shot = Some("22-distant-edit".into());
            self.phase = 3;
        }
        if self.elapsed >= 25.0 && self.phase == 3 {
            Self::camera(
                p,
                Vec3::new(900032.0, 100.0, 0.5),
                Vec3::new(900042.0, 100.0, 0.5),
            );
            p.precise_pos.x = 900031.999;
            p.pos = p.precise_pos.as_vec3();
            self.phase = 4;
        }
        if self.elapsed >= 30.0 && self.phase == 4 {
            println!(
                "[sandbox-review] floating_origin={} precise_steps={} sub_ulp_steps={} x={:.9}",
                flag(
                    self.moved_samples > 100
                        && self.precision_samples > 0
                        && p.precise_pos.x > 900032.0
                ),
                self.moved_samples,
                self.precision_samples,
                p.precise_pos.x
            );
            p.vel = Vec3::ZERO;
            self.phase = 5;
            shot = Some("30-floating-origin".into());
        }
        if self.elapsed >= 32.0 && self.phase == 5 {
            let q = IVec3::new(-10, 97, 0);
            w.set(q, B::Fire);
            blocks.insert(q);
            crate::world::mark_chunk_and_neighbors_dirty(chunks, q);
            w.sandbox.set(
                q,
                ThermalCell {
                    temperature: 500.125,
                    fuel: 25.5,
                    gas: 0,
                },
            );
            if let Some(body) = w.sandbox.bodies.first_mut() {
                body.velocity.x = 0.625;
                body.angular.y = 0.125;
            }
            w.sandbox.encode(&mut self.expected);
            self.expected_pos = p.precise_pos;
            let mut save = SaveService::new();
            save.request(true);
            self.save = Some(save);
            sim.set_enabled(false);
            self.phase = 6;
        }
        if let Some(save) = &mut self.save {
            sim.set_enabled(false);
            save.tick(Some(w), p, day);
            if !save.busy() && !self.saved {
                self.saved = true;
                if save.failed {
                    println!("[sandbox-review] save=FAIL {:?}", save.status);
                } else {
                    println!("[sandbox-review] save=PASS {:?}", save.status);
                    let (tx, rx) = mpsc::sync_channel(1);
                    self.loading = Some(rx);
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::world::storage::load_review_checkpoint());
                    });
                }
            }
        }
        if let Some(rx) = &self.loading {
            if let Ok(result) = rx.try_recv() {
                self.loading = None;
                match result {
                    Ok(loaded) => {
                        let mut actual = Vec::new();
                        loaded.world.sandbox.encode(&mut actual);
                        let equal = actual == self.expected
                            && loaded.player.precise_pos == self.expected_pos;
                        let arch_y =
                            crate::world::distant_surface(&loaded.world.sample_config(), 305, 0)
                                .0
                                .ceil() as i32;
                        println!(
                            "[sandbox-review] load={} sandbox_bytes={} precise_delta={} distant_edit={}",
                            flag(equal),
                            actual.len(),
                            loaded.player.precise_pos.distance(self.expected_pos),
                            flag(
                                loaded.world.get(IVec3::new(305, arch_y + 8, 0)) == B::StoneBricks
                            )
                        );
                        *w = loaded.world;
                        *p = loaded.player;
                        sim.reset_for_world(w.voxel_scale);
                        sim.set_enabled(true);
                        Self::camera(p, Vec3::new(16.0, 108.0, 22.0), Vec3::new(0.0, 99.0, 1.0));
                        self.save = None;
                        self.phase = 7;
                        reloaded = true;
                        shot = Some("33-loaded".into());
                    }
                    Err(e) => println!("[sandbox-review] load=FAIL {e:#}"),
                }
            }
        }
        if self.elapsed >= 38.0 && self.phase == 7 {
            println!(
                "[sandbox-review] resumed={} complete=YES",
                flag(w.get(IVec3::new(-10, 97, 0)) != B::Fire)
            );
            println!(
                "[sandbox-review] original_burnout={} original_fuel={}",
                flag(w.get(IVec3::new(-7, 97, 0)) != B::Fire),
                w.sandbox.cell(IVec3::new(-7, 97, 0)).fuel
            );
            self.phase = 8;
            shot = Some("38-resumed".into());
        }
        if self.elapsed >= 3.0 && self.shots == 0 {
            self.shots = 1;
            shot = Some("03-initial-physics".into());
        }
        if self.elapsed >= 5.0 && self.shots == 1 {
            Self::camera(p, Vec3::new(7.5, 100.0, 1.8), Vec3::new(7.5, 106.0, 0.0));
            self.shots = 2;
        }
        if self.elapsed >= 7.0 && self.shots == 2 {
            shot = Some("07-contained-gas".into());
            self.shots = 3;
        }
        if self.elapsed >= 8.0 && self.shots == 3 {
            Self::camera(p, Vec3::new(16.0, 108.0, 22.0), Vec3::new(0.0, 99.0, 1.0));
            self.shots = 4;
        }
        (reloaded, shot)
    }
    pub fn running(&self) -> bool {
        self.save.is_none()
    }
}
