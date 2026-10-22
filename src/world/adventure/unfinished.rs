//! A finite powered workshop with an interceptable, genuinely usable module.
//! Staged until the family/recipe/save/interaction/arena integration is complete.
use super::*;
use crate::world::{
    factory::portable::PowerKit,
    inventory::{Item, ItemStack},
};
use action::{Phase, Run};
pub(super) const KIND: u32 = 19;
pub(super) const KIT_CHARGE: u32 = 16_000;
pub(super) const FABRICATION_ENERGY: u32 = KIT_CHARGE + 400;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Sweep,
    Slam,
    Coil,
    Transfer,
}
#[derive(Default)]
pub(super) struct State {
    pub run: Option<Run<Action>>,
    pub reserved: u32,
    pub parts: bool,
    pub charge_work: f64,
    pub work: f64,
    pub module: Option<PowerKit>,
    pub module_pos: Vec3,
    pub installed: bool,
    pub charge: u32,
    pub pause: f32,
    pub drop_pending: bool,
    pub built: u32,
    pub stolen: u32,
}
pub(super) fn hurt(e: &mut Enemy, amount: f32) {
    if amount < 18.0 {
        return;
    }
    e.unfinished.pause = 0.7;
    if let Some(run) = &mut e.unfinished.run {
        if run.kind == Action::Transfer && matches!(run.phase(), Phase::Prepare | Phase::Active) {
            run.interrupt();
            e.unfinished.drop_pending = true;
        }
    }
}
pub(super) fn action_name(e: &Enemy) -> Option<&'static str> {
    Some(
        match e.unfinished.run.as_ref().map(|r| (r.kind, r.phase())) {
            Some((_, Phase::Recover)) => "Braced recovery",
            Some((Action::Sweep, _)) => "Cutting sweep — flank",
            Some((Action::Slam, _)) => "Stamp — jump or retreat",
            Some((Action::Coil, _)) => "Powered discharge — use cover",
            Some((Action::Transfer, _)) => "Installing charged module — intercept it",
            None if e.unfinished.parts && e.unfinished.reserved < FABRICATION_ENERGY => {
                "Charging module input — electrical supply required"
            }
            None if e.unfinished.parts => "Fabricating charged module",
            None if e.unfinished.module.is_some() => "Charged module exposed",
            None if e.unfinished.charge == 0 => "Depleted — needs stock and electrical power",
            None => "Holding a powered tool",
        },
    )
}
pub(super) fn claim(world: &mut World, player: Vec3, id: u32) -> bool {
    let Some(index) = world
        .adventure
        .enemies
        .iter()
        .position(|e| e.habitat == Some(id) && e.kind == KIND)
    else {
        return false;
    };
    let e = &world.adventure.enemies[index];
    let Some(kit) = e.unfinished.module else {
        return false;
    };
    if player.distance(e.unfinished.module_pos) > 3.0
        || !line(world, player + Vec3::Y * 1.6, e.unfinished.module_pos)
    {
        return false;
    }
    let (inserted, _) = world.inventory.try_insert(ItemStack {
        item: Item::PowerKit(kit),
        count: 1,
    });
    if inserted == 0 {
        return false;
    }
    let e = &mut world.adventure.enemies[index];
    e.unfinished.module = None;
    e.unfinished.stolen = e.unfinished.stolen.saturating_add(1);
    if let Some(run) = &mut e.unfinished.run {
        if run.kind == Action::Transfer {
            run.interrupt();
        }
    }
    world.expedition.say("Charged foundry module recovered: a tier-2 portable battery with its real stored energy. Place it as a power assembly.");
    println!(
        "[unfinished] module claimed owner={id} energy={} tier={}",
        kit.energy, kit.upgrade
    );
    true
}
pub(super) fn tick(
    e: &mut Enemy,
    world: &mut World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    shots: &mut Vec<Projectile>,
    sounds: &mut Vec<Sound>,
    navigation_budget: &mut usize,
) -> (f32, Vec3) {
    navigation::gravity(e, world, dt);
    e.unfinished.pause = (e.unfinished.pause - dt).max(0.0);
    let near = e.visible && feet.distance(e.pos) < 24.0;
    let service = family_service(world, e);
    // Reserve actual parts once. Charge arrives through a real battery's I/O
    // allowance; fabrication is not permission to bypass electrical throughput.
    if (near || service)
        && e.unfinished.module.is_none()
        && !e.unfinished.installed
        && !e.unfinished.parts
    {
        if family_foundry_reserve(world, e) {
            e.unfinished.parts = true;
            e.unfinished.reserved = family_foundry_stored_energy(world, e, FABRICATION_ENERGY);
            e.unfinished.work = 0.0;
        }
    }
    if e.unfinished.parts && e.unfinished.pause == 0.0 {
        if e.unfinished.reserved < FABRICATION_ENERGY {
            e.unfinished.charge_work += dt as f64;
            if e.unfinished.charge_work >= 0.25 {
                e.unfinished.charge_work -= 0.25;
                e.unfinished.reserved +=
                    family_foundry_recharge(world, e, FABRICATION_ENERGY - e.unfinished.reserved);
            }
        } else {
            e.unfinished.work += dt as f64;
        }
        if e.unfinished.work >= 3.0 {
            e.unfinished.module = Some(PowerKit {
                kind: factory::Kind::Battery,
                energy: KIT_CHARGE,
                upgrade: 2,
                mode: 0,
                limit: 32,
            });
            e.unfinished.reserved = 0;
            e.unfinished.parts = false;
            family_electrical_work(world, e, 400);
            e.unfinished.work = 0.0;
            e.unfinished.module_pos = e.pos + Vec3::new(1.8, 0.8, 0.0);
            e.unfinished.built = e.unfinished.built.saturating_add(1);
            println!(
                "[unfinished] module completed energy={KIT_CHARGE} built={}",
                e.unfinished.built
            );
        }
    } else if e.unfinished.installed && e.unfinished.charge < KIT_CHARGE {
        e.unfinished.charge_work += dt as f64;
        if e.unfinished.charge_work >= 0.25 {
            e.unfinished.charge_work -= 0.25;
            let requested = (KIT_CHARGE - e.unfinished.charge).min(400);
            e.unfinished.charge += family_foundry_recharge(world, e, requested);
        }
    }
    if e.unfinished.drop_pending {
        let q = e.pos + Vec3::new(1.8, 0.5, 0.0);
        if line(world, e.unfinished.module_pos, q) {
            e.unfinished.module_pos = q;
        }
        e.unfinished.drop_pending = false;
    }
    if let Some(mut run) = e.unfinished.run.take() {
        if e.stagger > 0.35 && run.phase() == Phase::Prepare {
            run.interrupt();
        }
        let previous = run.phase();
        let release = run.advance(dt);
        let mut damage = 0.0;
        e.tell = if run.phase() == Phase::Prepare {
            run.prepare - run.elapsed
        } else {
            0.0
        };
        if release && run.kind == Action::Coil && e.unfinished.charge >= 400 && shots.len() <= 93 {
            e.unfinished.charge -= 400;
            for i in -1..=1 {
                family_bolt(shots, e.center(), e.aim, i as f32 * 0.12, 11.0, 1.2);
            }
        }
        if run.phase() == Phase::Active {
            if run.kind == Action::Transfer {
                if e.unfinished.module.is_none() {
                    run.interrupt();
                } else {
                    let step = (run.elapsed - run.prepare) / run.active;
                    let q = (e.pos + Vec3::new(1.8, 0.8, 0.0))
                        .lerp(e.pos + Vec3::Y * 1.35, step.clamp(0.0, 1.0));
                    if line(world, e.unfinished.module_pos, q) {
                        e.unfinished.module_pos = q;
                    } else {
                        run.interrupt();
                    }
                }
            } else if !run.hit && matches!(run.kind, Action::Sweep | Action::Slam) {
                let d = (feet - e.pos) * Vec3::new(1.0, 0.0, 1.0);
                let contact = if run.kind == Action::Slam {
                    d.length() < 3.4 && feet.y - e.pos.y < 0.65
                } else {
                    d.length() < 3.6 && d.normalize_or_zero().dot(e.lunge) > -0.1
                };
                if contact && (feet.y - e.pos.y).abs() < 2.0 && line(world, e.center(), eye) {
                    damage = 23.0;
                    run.hit = true;
                }
            }
        }
        if run.kind == Action::Transfer
            && previous == Phase::Active
            && run.phase() == Phase::Recover
            && !run.hit
            && line(world, e.unfinished.module_pos, e.pos + Vec3::Y * 1.35)
        {
            if let Some(kit) = e.unfinished.module.take() {
                e.unfinished.installed = true;
                e.unfinished.charge = kit.energy;
                println!("[unfinished] installed energy={}", kit.energy);
            }
        }
        if run.phase() != Phase::Done {
            e.unfinished.run = Some(run);
        } else {
            e.cooldown = 0.5;
        }
        return (damage, (e.center() - eye).normalize_or_zero() * damage);
    }
    if e.cooldown > 0.0 || (!near && !service) {
        return (0.0, Vec3::ZERO);
    }
    let distance = feet.distance(e.pos);
    let action = if near && distance < 3.5 {
        if e.attacks % 2 == 0 {
            Action::Slam
        } else {
            Action::Sweep
        }
    } else if e.unfinished.module.is_some() {
        Action::Transfer
    } else if near && e.unfinished.charge >= 400 {
        Action::Coil
    } else {
        let goal = if near && e.pos.distance(e.home) < 20.0 {
            feet
        } else {
            e.home
        };
        let direction =
            e.navigation
                .heading(world, e.pos, goal, KIND, dt, e.slot, navigation_budget);
        let q = e.pos + direction * dt * 0.9;
        if free(world, q, KIND) && factory::solid_point(world, q - Vec3::Y * 0.15) {
            e.phase += e.pos.distance(q);
            e.pos = q;
        }
        return (0.0, Vec3::ZERO);
    };
    e.aim = eye;
    e.lunge = ((feet - e.pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    e.yaw = (-e.lunge.x).atan2(-e.lunge.z);
    let (prepare, active, recover) = match action {
        Action::Sweep => (0.9, 0.45, 1.0),
        Action::Slam => (1.2, 0.2, 1.1),
        Action::Coil => (1.1, 0.1, 1.0),
        Action::Transfer => (0.8, 3.0, 0.6),
    };
    e.attacks = e.attacks.wrapping_add(1);
    e.attack = 8;
    e.tell = prepare;
    e.unfinished.run = Some(Run::new(action, e.attacks, prepare, active, recover));
    sound(sounds, Cue::ForgeWindup, Some(e.center()), 0.75);
    println!(
        "[unfinished] action={action:?} electrical_charge={} work={:.2}",
        e.unfinished.charge, e.unfinished.work
    );
    (0.0, Vec3::ZERO)
}
pub(super) fn model(e: &Enemy, m: &mut Model, _time: f32) {
    let mark = m.mark();
    m.set_surface_kind(58.0);
    let local = |x, y, z| e.pos + Vec3::new(x, y, z);
    let wind = e.unfinished.run.as_ref().map_or(0.0, |r| r.wind());
    let iron = if e.flash > 0.0 {
        PALETTE_PAPER
    } else {
        [0.28, 0.3, 0.31]
    };
    for side in [-1.0, 1.0] {
        let step = (e.phase * 4.0 + side).sin() * 0.15;
        m.cuboid(
            local(side * 0.65, 0.3, step),
            Vec3::new(0.65, 0.55, 0.95),
            Quat::IDENTITY,
            iron,
            0.0,
        );
        m.beam(
            local(side * 0.55, 0.5, 0.0),
            local(side * 0.5, 1.1, 0.0),
            0.2,
            iron,
            0.0,
        );
    }
    m.cuboid(
        local(0.0, 1.35, 0.0),
        Vec3::new(1.55, 1.2, 1.2),
        Quat::IDENTITY,
        [0.45, 0.23, 0.14],
        0.0,
    );
    m.cuboid(
        local(0.0, 2.15, 0.0),
        Vec3::new(0.7, 0.5, 0.7),
        Quat::IDENTITY,
        iron,
        0.0,
    );
    m.cuboid(
        local(0.0, 1.1, -0.66),
        Vec3::new(0.8, 0.55, 0.1),
        Quat::IDENTITY,
        [0.9, 0.46, 0.2],
        0.25,
    );
    m.beam(
        local(-0.65, 1.7, 0.0),
        local(-1.35, 1.2, -0.75 - wind * 0.5),
        0.2,
        iron,
        0.0,
    );
    m.cuboid(
        local(-1.35, 1.2, -1.0 - wind * 0.5),
        Vec3::new(0.15, 0.75, 0.65),
        Quat::IDENTITY,
        [0.62, 0.65, 0.62],
        0.0,
    );
    let fabrication = (e.unfinished.work as f32 / 3.0).clamp(0.0, 1.0);
    if e.unfinished.parts {
        m.cuboid(
            local(0.5, 1.8, 0.0),
            Vec3::new(0.55, 0.15 + fabrication * 0.4, 0.55),
            Quat::IDENTITY,
            [0.55, 0.74, 0.82],
            0.0,
        );
    }
    if e.unfinished.installed {
        m.cuboid(
            local(0.65, 1.75, -0.2),
            Vec3::new(0.65, 0.65, 0.85),
            Quat::IDENTITY,
            [0.55, 0.74, 0.82],
            0.0,
        );
    }
    m.bake_model_uv_since(mark, e.pos, 58.0);
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
    if e.unfinished.module.is_some() {
        let kit_mark = m.mark();
        m.cuboid(
            e.unfinished.module_pos,
            Vec3::splat(0.55),
            Quat::IDENTITY,
            [0.55, 0.74, 0.82],
            0.0,
        );
        m.bake_model_uv_since(kit_mark, e.unfinished.module_pos, 58.0);
        m.beam(
            e.pos + Vec3::Y * 1.7,
            e.unfinished.module_pos,
            0.1,
            iron,
            0.0,
        );
    }
    m.set_surface_kind(41.0);
}
impl State {
    pub(super) fn save(&self, out: &mut Vec<u32>) {
        let (tag, payload) = self
            .module
            .map_or((0, 0), |k| Item::PowerKit(k).encode_u32());
        out.extend([
            self.parts as u32,
            self.reserved,
            self.work.to_bits() as u32,
            (self.work.to_bits() >> 32) as u32,
            self.charge_work.to_bits() as u32,
            (self.charge_work.to_bits() >> 32) as u32,
            self.module.is_some() as u32,
            tag,
            payload,
            self.installed as u32,
            self.charge,
            self.built,
            self.stolen,
        ]);
        out.extend(self.module_pos.to_array().map(f32::to_bits));
    }
    pub(super) fn restore(&mut self, words: &[u32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            words.len() == 16
                && words[0] <= 1
                && words[1] <= FABRICATION_ENERGY
                && words[6] <= 1
                && words[9] <= 1
                && words[10] <= KIT_CHARGE,
            "Invalid foundry state"
        );
        let work = f64::from_bits(words[2] as u64 | ((words[3] as u64) << 32));
        let charge_work = f64::from_bits(words[4] as u64 | ((words[5] as u64) << 32));
        let module = if words[6] == 1 {
            match Item::decode_u32(words[7], words[8])? {
                Item::PowerKit(k)
                    if k.kind == factory::Kind::Battery
                        && k.upgrade == 2
                        && k.energy <= KIT_CHARGE =>
                {
                    Some(k)
                }
                _ => anyhow::bail!("Invalid foundry module"),
            }
        } else {
            None
        };
        let pos = Vec3::new(
            f32::from_bits(words[13]),
            f32::from_bits(words[14]),
            f32::from_bits(words[15]),
        );
        anyhow::ensure!(
            work.is_finite()
                && (0.0..3.0).contains(&work)
                && pos.is_finite()
                && pos.abs().max_element() <= 1_000_000.0
                && charge_work.is_finite()
                && (0.0..0.25).contains(&charge_work)
                && !(words[0] == 1 && (module.is_some() || words[9] == 1))
                && (words[0] == 1 || words[1] == 0)
                && !(words[9] == 1 && module.is_some()),
            "Invalid foundry ownership"
        );
        self.parts = words[0] == 1;
        self.reserved = words[1];
        self.work = work;
        self.charge_work = charge_work;
        self.module = module;
        self.module_pos = pos;
        self.installed = words[9] == 1;
        self.charge = words[10];
        self.built = words[11];
        self.stolen = words[12];
        Ok(())
    }
}
