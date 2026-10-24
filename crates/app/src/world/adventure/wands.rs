//! Noita-style ordered spell deck evaluator: combinable modifiers, multicasts,
//! impact triggers, and chassis stats visible on hover.
use super::*;

pub const EMPTY: u32 = 8;
pub const MAX_SLOTS: usize = 8;

pub use super::spell_defs::spell_audio;

/// Fires a bolt that flies and impacts.
pub fn projectile(spell: u32) -> bool {
    spell_class(spell) == Class::Projectile
}

/// A card that changes the next bolt, or the deck's control flow.
pub fn is_modifier(spell: u32) -> bool {
    spell_class(spell) == Class::Modifier
}

/// A growth or summon effect delivered by a Touch or a trigger.
pub fn is_effect(spell: u32) -> bool {
    spell_class(spell) == Class::Effect
}

/// Accumulated projectile modifiers evaluated left-to-right across a cast block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotMods {
    pub damage_mul: f32,
    pub speed_mul: f32,
    pub bounces: u8,
    pub homing: bool,
    pub orbit: bool,
    pub bore: bool,
    pub trail: bool,
    /// Releases a small crater and radial impulse on impact.
    pub volatile: bool,
    /// Restores player health on a direct hit.
    pub siphon: bool,
    /// Projectile collision scale. 1.0 is the authored bolt.
    pub radius: f32,
    /// Steer toward the owned lanterncap before enemies.
    pub seek_ally: bool,
    /// 0 straight, 1 curve, 2 wiggle, 3 orbit.
    pub path: u8,
    /// Heal for a fraction of magic damage dealt.
    pub blood: bool,
    /// One cast in five fizzles.
    pub unstable: bool,
    pub life_mul: f32,
    /// Elemental status applied to enemies on a direct hit.
    pub element: Option<super::status::Element>,
    /// Arcs to nearby enemies on impact.
    pub chain: bool,
    /// Bursts into shrapnel that hurts nearby enemies on impact.
    pub shrapnel: bool,
    /// Fires the bolt behind the caster.
    pub reverse: bool,
    /// Downward pull on the bolt in units of 14 m/s^2; negative floats it.
    pub gravity: f32,
    /// Walls this bolt may pass straight through before it behaves normally.
    pub phase: u8,
    /// +damage, but each cast costs a little health.
    pub reckless: bool,
    /// The cast is paid for in health instead of mana.
    pub blood_price: bool,
    /// Breaks into three weak sparks on impact.
    pub fork: bool,
    /// Seconds of flight before an attached payload fires mid-air (0 = off).
    pub timer: f32,
    /// Each hit restores a little mana.
    pub mana_leech: bool,
    /// One hit in four lands for 2.5x damage.
    pub crit: bool,
    /// The attached payload also fires if the bolt times out without a hit.
    pub expire: bool,
    /// Hits on wet or chilled foes land for double damage (Soaked strike).
    pub crit_wet: bool,
    /// A larger blast that hurts foes, and the careless caster, on impact.
    pub explosive: bool,
    /// Foes this bolt may pass through before it stops.
    pub pierce: u8,
    /// Slot + 1 of the foe last pierced, so one foe is not hit every substep.
    pub pierced: u32,
    /// The attached payload fires at the bolt's first bounce.
    pub bounce_release: bool,
    /// The attached payload fires when a foe comes within reach.
    pub proximity: bool,
    /// Drags nearby foes toward the impact.
    pub pull: bool,
}

impl Default for ShotMods {
    fn default() -> Self {
        Self {
            damage_mul: 1.0,
            speed_mul: 1.0,
            bounces: 0,
            homing: false,
            orbit: false,
            bore: false,
            trail: false,
            volatile: false,
            siphon: false,
            radius: 1.0,
            seek_ally: false,
            path: 0,
            blood: false,
            unstable: false,
            life_mul: 1.0,
            element: None,
            chain: false,
            shrapnel: false,
            reverse: false,
            gravity: 0.0,
            phase: 0,
            reckless: false,
            blood_price: false,
            fork: false,
            timer: 0.0,
            mana_leech: false,
            crit: false,
            expire: false,
            crit_wet: false,
            explosive: false,
            pierce: 0,
            pierced: 0,
            bounce_release: false,
            proximity: false,
            pull: false,
        }
    }
}

/// Noita-style wand chassis statistics displayed on hover and used during evaluation.
#[derive(Clone, Copy, Debug)]
pub struct WandStats {
    pub name: &'static str,
    pub chassis: &'static str,
    pub shuffle: bool,
    pub spells_per_cast: u32,
    pub cast_delay: f32,
    pub recharge_time: f32,
    pub mana_max: f32,
    pub mana_charge_speed: f32,
    pub capacity: usize,
    pub spread_deg: f32,
    pub always_cast: Option<u32>,
}

/// One resolved spell invocation inside a cast block.
#[derive(Clone, Copy, Debug)]
pub struct CastAction {
    pub slot: usize,
    pub spell: u32,
    pub payload: Option<u32>,
    pub payload_slot: Option<usize>,
    /// Modifiers drawn after the carrier and before the payload. They do not
    /// touch the carrier. An empty payload leaves this at the default shot.
    pub payload_mods: ShotMods,
    pub follow: Option<u32>,
    pub mods: ShotMods,
}

/// Complete evaluation of the cast block starting at a given wand cursor.
#[derive(Clone, Debug)]
pub struct CastBlockEval {
    pub actions: Vec<CastAction>,
    pub modifier_slots: Vec<usize>,
    pub last_slot: usize,
    pub mana_cost: f32,
    pub cast_delay: f32,
    pub recharge_time: f32,
    pub spread_deg: f32,
    pub mods: ShotMods,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub slots: [u32; 4],
    pub aux_slots: [u32; 4],
    pub cursor: usize,
    pub mode: u32,
    /// Found wands roll their chassis from this; 0 is the plain crafted wand.
    pub seed: u32,
}

impl Default for Loadout {
    fn default() -> Self {
        Self {
            slots: [0, 2, EMPTY, EMPTY],
            aux_slots: [EMPTY; 4],
            cursor: 0,
            mode: 0,
            seed: 0,
        }
    }
}

impl Loadout {
    pub fn slot(&self, idx: usize) -> u32 {
        if idx < 4 {
            self.slots[idx]
        } else if idx < MAX_SLOTS {
            self.aux_slots[idx - 4]
        } else {
            EMPTY
        }
    }

    pub fn set_slot(&mut self, idx: usize, spell: u32) {
        if idx < 4 {
            self.slots[idx] = spell;
        } else if idx < MAX_SLOTS {
            self.aux_slots[idx - 4] = spell;
        }
    }
}

pub fn capacity(tier: u32) -> usize {
    match tier {
        0 => 1,
        1 => 4,
        2 => 6,
        _ => 8,
    }
}

/// Fills newly unlocked empty slots when a wand is assembled or upgraded, placing
/// only spells the player has found. Existing spells stay put so a player's deck
/// is never overwritten.
pub fn seed_unlocked_slots_known(loadout: &mut Loadout, tier: u32, known: &dyn Fn(u32) -> bool) {
    let offers: &[u32] = match tier {
        1 => &[0, 2, 1, 4],
        2 => &[9, 16, 3, 5],
        _ => &[12, 19, 13, 20],
    };
    let mut offer = 0usize;
    let cap = capacity(tier);
    for idx in 0..cap {
        if loadout.slot(idx) != EMPTY {
            continue;
        }
        while offer < offers.len()
            && ((0..idx).any(|i| loadout.slot(i) == offers[offer]) || !known(offers[offer]))
        {
            offer += 1;
        }
        if offer >= offers.len() {
            break;
        }
        loadout.set_slot(idx, offers[offer]);
        offer += 1;
    }
}





pub fn stats(tier: u32, mode: u32, rank: u32) -> WandStats {
    let cap = capacity(tier);
    let base_recharge: f32 = match tier {
        1 => 0.65,
        2 => 0.45,
        _ => 0.30,
    };
    let base_delay: f32 = match tier {
        1 => 0.16,
        2 => 0.11,
        _ => 0.07,
    };
    let (chassis, spells_per_cast, delay_mod, recharge_mod, spread_mod) = match mode {
        1 => ("Twin Scatter", 2, 0.04_f32, 0.05_f32, 6.0_f32),
        2 => ("Trigger Conduit", 1, 0.02_f32, 0.0_f32, 0.0_f32),
        3 => ("Rapid Flux", 1, -0.06_f32, -0.08_f32, 1.5_f32),
        _ => ("Ordered Deck", 1, 0.0_f32, 0.0_f32, 0.0_f32),
    };
    WandStats {
        name: match tier {
            0 => "Unarmed",
            1 => "Lumen Twig",
            2 => "Quarry Conduit",
            _ => "Sanctum Sovereign",
        },
        chassis,
        shuffle: mode == 3,
        spells_per_cast,
        cast_delay: (base_delay + delay_mod).max(0.03),
        recharge_time: (base_recharge + recharge_mod).max(0.10),
        mana_max: 100.0 + rank as f32 * 15.0,
        mana_charge_speed: 8.0 + rank as f32 * 1.5 + tier as f32 * 4.5,
        capacity: cap,
        spread_deg: (match tier {
            1 => 2.0,
            2 => 0.8,
            _ => 0.0,
        } + spread_mod),
        always_cast: if tier >= 3 && mode == 3 {
            Some(23)
        } else {
            None
        },
    }
}

pub fn evaluate_cast_block(
    wand: u32,
    rare_spells: u32,
    discoveries: u32,
    rank: u32,
    loadout: &Loadout,
    start_cursor: usize,
) -> CastBlockEval {
    evaluate_programmed(
        wand,
        rare_spells,
        discoveries,
        rank,
        loadout,
        start_cursor,
        1.0,
        false,
        0,
    )
}

/// Stat modifiers. Control-flow cards (multicast, trigger, divide, conditions,
/// alpha) return false so the caller can nest the next draw.
fn paint_shot(spell: u32, mods: &mut ShotMods, salt: u32) -> bool {
    match spell {
        19 => mods.homing = true,
        20 => {
            mods.orbit = true;
            mods.bounces = (mods.bounces + 2).min(4);
            mods.damage_mul = (mods.damage_mul + 0.22).min(2.6);
            mods.speed_mul = (mods.speed_mul * 1.08).min(1.6);
        }
        21 => {
            mods.bore = true;
            mods.damage_mul = (mods.damage_mul + 0.35).min(2.6);
            mods.speed_mul = (mods.speed_mul * 0.84).max(0.65);
        }
        23 => {
            mods.trail = true;
            mods.damage_mul = (mods.damage_mul + 0.12).min(2.6);
        }
        26 => {
            mods.volatile = true;
            mods.damage_mul = (mods.damage_mul + 0.08).min(2.6);
        }
        27 => {
            mods.siphon = true;
            mods.damage_mul = (mods.damage_mul + 0.05).min(2.6);
        }
        37 => mods.radius = 1.8,
        38 => {
            mods.seek_ally = true;
            mods.homing = true;
        }
        42 => mods.path = 1,
        43 => mods.path = 2,
        44 => mods.path = 3,
        45 => mods.blood = true,
        46 => mods.unstable = true,
        50 => mods.path = 1 + (salt % 3) as u8,
        51 => {
            mods.speed_mul = (mods.speed_mul * 1.35).min(1.6);
            mods.damage_mul = (mods.damage_mul - 0.15).max(0.5);
        }
        52 => mods.life_mul = (mods.life_mul * 1.8).min(3.0),
        53 => mods.bounces = (mods.bounces + 1).min(4),
        54 => mods.speed_mul = (mods.speed_mul * 1.25).min(1.6),
        55 => mods.damage_mul = (mods.damage_mul + 0.15).min(2.6),
        56 => mods.element = Some(super::status::Element::Fire),
        57 => mods.element = Some(super::status::Element::Shock),
        58 => mods.element = Some(super::status::Element::Toxic),
        59 => mods.element = Some(super::status::Element::Wet),
        60 => mods.element = Some(super::status::Element::Frost),
        61 => mods.chain = true,
        62 => mods.shrapnel = true,
        63 => {
            mods.damage_mul = (mods.damage_mul + 0.45).min(2.6);
            mods.speed_mul = (mods.speed_mul * 0.9).max(0.65);
        }
        64 => mods.reverse = true,
        65 => mods.gravity = (mods.gravity + 1.0).min(2.0),
        66 => mods.phase = (mods.phase + 1).min(2),
        67 => {
            mods.reckless = true;
            mods.damage_mul = (mods.damage_mul + 0.6).min(2.6);
        }
        68 => mods.blood_price = true,
        69 => mods.fork = true,
        77 => mods.mana_leech = true,
        78 => mods.crit = true,
        88 => mods.crit_wet = true,
        89 => {
            mods.explosive = true;
            mods.speed_mul = (mods.speed_mul * 0.85).max(0.65);
        }
        90 => mods.pierce = (mods.pierce + 2).min(4),
        94 => mods.pull = true,
        _ => return false,
    }
    true
}

pub fn evaluate_programmed(
    wand: u32,
    rare_spells: u32,
    discoveries: u32,
    rank: u32,
    loadout: &Loadout,
    start_cursor: usize,
    health_frac: f32,
    foe_visible: bool,
    salt: u32,
) -> CastBlockEval {
    let cap = capacity(wand);
    if !(0..cap).any(|i| spell_available(loadout.slot(i), wand, rare_spells)) {
        return CastBlockEval {
            actions: Vec::new(),
            modifier_slots: Vec::new(),
            last_slot: start_cursor % cap.max(1),
            mana_cost: 0.0,
            cast_delay: 0.12,
            recharge_time: 0.0,
            spread_deg: 0.0,
            mods: ShotMods::default(),
        };
    }
    let wand_stats = super::wand_items::stats_for(wand, loadout, rank);
    let find_next = |after: usize| -> usize {
        (1..=cap)
            .map(|n| (after + n) % cap)
            .find(|&i| spell_available(loadout.slot(i), wand, rare_spells))
            .unwrap_or(after % cap)
    };

    let mut cur = start_cursor % cap;
    if !spell_available(loadout.slot(cur), wand, rare_spells) {
        cur = find_next(cur);
    }
    let first_slot = cur;
    let first_spell = loadout.slot(first_slot);

    // Preserve exact legacy behavior for non-modifier single slots in modes 0, 1, 2
    // so all existing acceptance & unit tests match bit-for-bit.
    if !is_modifier(first_spell) && loadout.mode <= 2 {
        let next = find_next(first_slot);
        let payload = if loadout.mode == 2
            && projectile(first_spell)
            && next > first_slot
            && projectile(loadout.slot(next))
        {
            Some(loadout.slot(next))
        } else {
            None
        };
        let twin = loadout.mode == 1 && projectile(first_spell);
        let count = if twin { 2 } else { 1 };
        let mana_cost = inscriptions::cost(first_spell, discoveries) * if twin { 1.6 } else { 1.0 }
            + payload.map_or(0.0, |p| inscriptions::cost(p, discoveries));
        let last_slot = if payload.is_some() { next } else { first_slot };
        let after_cast = find_next(last_slot);
        let wrapped = after_cast <= last_slot;
        let recharge = if wrapped {
            (wand_stats.recharge_time + spell_recharge_delta(first_spell)).max(0.08)
        } else {
            0.0
        };
        let cast_delay = (spell_cooldown(first_spell) * if twin { 1.2 } else { 1.0 }
            + spell_cast_delay_delta(first_spell))
        .max(0.04);
        let payload_slot = payload.map(|_| next);
        let mut actions = Vec::with_capacity(count);
        for _ in 0..count {
            actions.push(CastAction {
                slot: first_slot,
                spell: first_spell,
                payload,
                payload_slot,
                payload_mods: ShotMods::default(),
                follow: None,
                mods: ShotMods::default(),
            });
        }
        return CastBlockEval {
            actions,
            modifier_slots: Vec::new(),
            last_slot,
            mana_cost,
            cast_delay,
            recharge_time: recharge,
            spread_deg: if twin { 6.3 } else { wand_stats.spread_deg },
            mods: ShotMods::default(),
        };
    }

    // Full Noita left-to-right modifier, multicast, and trigger deck evaluation.
    let mut mods = ShotMods::default();
    if wand_stats.always_cast == Some(23) {
        mods.trail = true;
        mods.damage_mul = (mods.damage_mul + 0.10).min(2.6);
    }
    let mut draws: usize = if loadout.mode == 1 { 2 } else { 1 };
    let mut mana_refund = 0.0f32;
    let mut attach_trigger = loadout.mode == 2;
    let mut delay_delta = if loadout.mode == 3 { -0.06 } else { 0.0 };
    let mut recharge_delta = if loadout.mode == 3 { -0.08 } else { 0.0 };
    let mut spread_deg = wand_stats.spread_deg;
    let mut mana_cost = 0.0f32;
    let mut copies: u8 = 1;
    let mut skip_draw = false;
    let mut rejected_draw = false;
    let mut copy_alpha = false;
    let alpha = (0..cap).map(|i| loadout.slot(i)).find(|s| projectile(*s));
    let mut modifier_slots = Vec::new();
    let mut actions = Vec::new();
    let mut last_slot = first_slot;
    let mut steps = 0usize;

    while draws > 0 && steps < cap {
        steps += 1;
        let s = loadout.slot(cur);
        if !spell_available(s, wand, rare_spells) {
            break;
        }
        last_slot = cur;
        if is_modifier(s) {
            modifier_slots.push(cur);
            mana_cost += inscriptions::cost(s, discoveries);
            delay_delta += spell_cast_delay_delta(s);
            recharge_delta += spell_recharge_delta(s);
            spread_deg = (spread_deg + spell_spread_delta(s)).max(0.0);
            if !paint_shot(s, &mut mods, salt) {
                match s {
                    16 => draws = (draws + 1).min(4),
                    17 => draws = (draws + 2).min(4),
                    18 => attach_trigger = true,
                    22 => mana_refund = mana_refund.max(20.0),
                    40 => copies = 2,
                    41 => copies = 4,
                    47 => skip_draw = health_frac >= 0.5,
                    48 => skip_draw = !foe_visible,
                    49 => copy_alpha = true,
                    70 => {
                        // Like Trigger bolt, but the payload fires mid-flight.
                        attach_trigger = true;
                        mods.timer = 0.6;
                    }
                    79 => {
                        // Trigger bolt plus: also fires if the bolt times out.
                        attach_trigger = true;
                        mods.expire = true;
                    }
                    91 => draws = (draws + 3).min(4),
                    92 => {
                        // Trigger bolt that fires its payload at the first bounce.
                        attach_trigger = true;
                        mods.bounce_release = true;
                        mods.bounces = (mods.bounces + 1).min(4);
                    }
                    93 => {
                        // Trigger bolt that fires its payload beside a foe.
                        attach_trigger = true;
                        mods.proximity = true;
                    }
                    _ => {}
                }
            }
            let next = find_next(cur);
            if next == first_slot && actions.is_empty() && steps >= cap {
                break;
            }
            cur = next;
        } else {
            if skip_draw {
                skip_draw = false;
                rejected_draw = true;
                cur = find_next(cur);
                continue;
            }
            draws -= 1;
            let fired = if copy_alpha { alpha.unwrap_or(s) } else { s };
            copy_alpha = false;
            mana_cost += inscriptions::cost(fired, discoveries);
            delay_delta += spell_cast_delay_delta(fired);
            recharge_delta += spell_recharge_delta(fired);
            spread_deg = (spread_deg + spell_spread_delta(fired)).max(0.0);

            let carrier_slot = cur;
            let mut payload = None;
            let mut payload_slot = None;
            let mut payload_mods = ShotMods::default();
            let mut follow = None;
            if attach_trigger && (projectile(fired) || fired == 34) {
                // Payload is a separate cast. Stat cards between the carrier
                // and the payload spell wrap the payload, not the carrier.
                let mut scan = find_next(cur);
                let mut guard = 0usize;
                while guard < cap
                    && scan != cur
                    && spell_available(loadout.slot(scan), wand, rare_spells)
                {
                    let wrapped = loadout.slot(scan);
                    if !paint_shot(wrapped, &mut payload_mods, salt) {
                        break;
                    }
                    modifier_slots.push(scan);
                    mana_cost += inscriptions::cost(wrapped, discoveries) * 0.85;
                    last_slot = scan;
                    scan = find_next(scan);
                    guard += 1;
                }
                let next_s = loadout.slot(scan);
                if scan != cur && (projectile(next_s) || next_s == 9 || is_effect(next_s)) {
                    payload = Some(next_s);
                    payload_slot = Some(scan);
                    mana_cost += inscriptions::cost(next_s, discoveries) * 0.85;
                    last_slot = scan;
                    cur = scan;
                    if is_effect(next_s) {
                        let after = find_next(scan);
                        let after_s = loadout.slot(after);
                        if after != scan && is_effect(after_s) {
                            follow = Some(after_s);
                            mana_cost += inscriptions::cost(after_s, discoveries) * 0.85;
                            last_slot = after;
                            cur = after;
                        }
                    }
                }
                attach_trigger = false;
            }

            let n = copies.clamp(1, 4);
            copies = 1;
            for _ in 0..n {
                if actions.len() >= 12 {
                    break;
                }
                actions.push(CastAction {
                    slot: carrier_slot,
                    spell: fired,
                    payload,
                    payload_slot,
                    payload_mods,
                    follow,
                    mods,
                });
            }

            if draws > 0 {
                let next = find_next(cur);
                if next == first_slot {
                    break;
                }
                cur = next;
            }
        }
    }

    // If the deck only contained modifiers, fire a modified Spark Bolt so the cast isn't dead.
    if actions.is_empty() && !modifier_slots.is_empty() && !rejected_draw {
        actions.push(CastAction {
            slot: first_slot,
            spell: 0,
            payload: None,
            payload_slot: None,
            payload_mods: ShotMods::default(),
            follow: None,
            mods,
        });
        mana_cost += inscriptions::cost(0, discoveries);
    }

    let base_cd = actions
        .iter()
        .map(|a| spell_cooldown(a.spell))
        .fold(0.18f32, f32::max);
    // Repeated Rapid Flux copies may still reduce delay, but never below the
    // single-copy allowance. This keeps a modifier deck from becoming a
    // free-casting machine.
    let delay_delta = delay_delta.max(-0.14);
    let recharge_delta = recharge_delta.max(-0.12);
    let cast_delay = (base_cd + delay_delta).max(0.04);
    let after_cast = find_next(last_slot);
    let wrapped = after_cast <= last_slot;
    let recharge = if wrapped {
        (wand_stats.recharge_time + recharge_delta).max(0.06)
    } else {
        0.0
    };

    CastBlockEval {
        actions,
        modifier_slots,
        last_slot,
        mana_cost: (mana_cost - mana_refund).max(0.0),
        cast_delay,
        recharge_time: recharge,
        spread_deg,
        mods,
    }
}

/// One hit in four is a critical hit. Deterministic in the clock and the
/// target so a replay of the same fight rolls the same dice.
pub(super) fn crit_roll(time: f32, slot: u32) -> bool {
    super::discovery::mix32(time.to_bits() ^ slot.wrapping_mul(0x9e37_79b1)).is_multiple_of(4)
}

/// Cast a ray from a position in metres. `raycast_block` works in voxel
/// coordinates, so at any scale other than 1 m a raw metre origin would aim from
/// the wrong place. Returns the struck cell and its face normal.
pub(super) fn aim_cell(
    world: &World,
    eye_m: Vec3,
    dir: Vec3,
    max_m: f32,
) -> Option<(IVec3, IVec3)> {
    let scale = world.voxel_scale.meters();
    crate::world::raycast_block(world, eye_m / scale, dir, max_m / scale)
}

impl Adventure {
    pub fn set_wand_slot(&mut self, slot: u32, spell: u32) {
        let cap = capacity(self.wand);
        if self.wand == 0
            || slot as usize >= cap
            || spell > MAX_SPELL_ID
            || (spell != EMPTY && !self.can_wield(spell))
        {
            return;
        }
        self.loadout.set_slot(slot as usize, spell);
        self.loadout.cursor = slot as usize;
        self.prepare_wand();
    }

    pub fn swap_wand_slots(&mut self, a: u32, b: u32) {
        let cap = capacity(self.wand);
        let (ia, ib) = (a as usize, b as usize);
        if self.wand == 0 || ia >= cap || ib >= cap || ia == ib {
            return;
        }
        let sa = self.loadout.slot(ia);
        let sb = self.loadout.slot(ib);
        self.loadout.set_slot(ia, sb);
        self.loadout.set_slot(ib, sa);
        self.loadout.cursor = ib;
        self.prepare_wand();
    }

    pub(super) fn prepare_wand(&mut self) {
        let cap = capacity(self.wand);
        self.loadout.cursor %= cap;
        for _ in 0..cap {
            let s = self.loadout.slot(self.loadout.cursor);
            if self.can_wield(s) {
                self.spell = s;
                return;
            }
            self.loadout.cursor = (self.loadout.cursor + 1) % cap;
        }
        self.spell = EMPTY;
    }

    fn next_slot(&self, after: usize) -> usize {
        let cap = capacity(self.wand);
        (1..=cap)
            .map(|n| (after + n) % cap)
            .find(|&i| self.can_wield(self.loadout.slot(i)))
            .unwrap_or(after % cap)
    }

    pub(super) fn advance_wand(&mut self) {
        self.loadout.cursor = self.next_slot(self.loadout.cursor);
        self.prepare_wand();
    }

    #[cfg(test)]
    pub(super) fn emit_spell(&mut self, spell: u32, pos: Vec3, dir: Vec3, payload: Option<u32>) {
        self.emit_spell_modified(spell, pos, dir, payload, ShotMods::default());
    }

    pub(super) fn emit_spell_modified(
        &mut self,
        spell: u32,
        pos: Vec3,
        dir: Vec3,
        payload: Option<u32>,
        mods: ShotMods,
    ) {
        if self.projectiles.len() >= 96 {
            return;
        }
        let mut mods = mods;
        if spell == 76 {
            // Bounce orb always ricochets, whatever else is on the deck.
            mods.bounces = mods.bounces.max(4);
        }
        arcana::innate_mods(spell, &mut mods);
        let speed = spell_base_speed(spell) * mods.speed_mul;
        let arc_lift = match spell {
            5 => Vec3::Y * 2.0,
            12 => Vec3::Y * 1.6,
            28 => Vec3::Y * 1.2,
            _ => Vec3::ZERO,
        };
        for dir in arcana::scatter_dirs(spell, dir) {
        if self.projectiles.len() >= 96 {
            return;
        }
        self.projectiles.push(Projectile {
            farm: None,
            pos,
            velocity: dir * speed + arc_lift,
            hostile: false,
            spell,
            life: ((match spell {
                10 => 3.6,
                71 => 0.16, // Chainsaw: a few metres of reach
                76 => 4.0,
                _ => arcana::base_life(spell).unwrap_or(3.0),
            }) * mods.life_mul.max(0.25))
            .min(arcana::life_cap(spell)),
            payload,
            payload_mods: ShotMods::default(),
            mods,
            bore_budget: if spell == 10 {
                24
            } else if spell == arcana::DIG_BOLT {
                12
            } else if mods.bore {
                6
            } else {
                0
            },
            trail_timer: 0.0,
            splashed: false,
            follow: None,
        });
        }
    }

    fn room_for_spell(&self, spell: u32, count: usize) -> bool {
        let incoming = self
            .projectiles
            .iter()
            .filter(|p| !p.hostile)
            .map(|p| usize::from(p.spell == spell) + usize::from(p.payload == Some(spell)))
            .sum::<usize>();
        match spell {
            3 | 29 => self.terrain_blasts.len() + incoming + count <= 4,
            5 => self.blooms.len() + incoming + count <= 6,
            6 | 10 => self.wells.len() + incoming + count <= 4,
            11 => self.crosses.len() + incoming + count <= 4,
            _ => true,
        }
    }

    pub(super) fn cast_wand(
        &mut self,
        world: &mut World,
        player: &mut Player,
        eye: Vec3,
        forward: Vec3,
    ) {
        if self.wand == 0 {
            self.cast_cd = 0.7;
            world.expedition.say(
                "Craft a lumen wand at a workbench. Arrange its spells in Gear; hold R to cast.",
            );
            return;
        }
        self.prepare_wand();
        let rank = self.rank();
        let foe = self
            .enemies
            .iter()
            .any(|e| e.health > 0.0 && (e.center() - eye).length() < 27.0);
        let eval = evaluate_programmed(
            self.wand,
            self.rare_spells,
            self.discoveries,
            rank,
            &self.usable_loadout(),
            self.loadout.cursor,
            self.health / self.max_health().max(1.0),
            foe,
            self.time.to_bits(),
        );
        if eval.actions.is_empty() {
            self.cast_cd = 0.15;
            return;
        }

        if eval.actions.len() == 1 && eval.actions[0].spell == 1 && self.health >= self.max_health()
        {
            self.loadout.cursor = eval.last_slot;
            self.advance_wand();
            self.cast_cd = 0.12;
            return;
        }
        // Blood price pays half the mana cost in health, and refuses to be the
        // reason the caster dies.
        let blood_price = eval.actions.iter().any(|a| a.mods.blood_price);
        let health_cost = eval.mana_cost * 0.5;
        if blood_price {
            if self.health <= health_cost + 4.0 {
                self.cast_cd = 0.12;
                self.mana_flash = 0.45;
                return;
            }
        } else if self.mana < eval.mana_cost * super::modifiers::mana_mul(self.site_modifier) {
            self.cast_cd = 0.12;
            self.mana_flash = 0.45;
            return;
        }

        let proj_count = eval.actions.iter().filter(|a| projectile(a.spell)).count();
        if proj_count > 0 && self.projectiles.len() + proj_count > 96 {
            self.cast_cd = 0.12;
            return;
        }
        for act in &eval.actions {
            if projectile(act.spell) {
                let same_count = eval.actions.iter().filter(|a| a.spell == act.spell).count();
                if !self.room_for_spell(act.spell, same_count)
                    || act.payload.is_some_and(|p| {
                        !self.room_for_spell(p, if p == act.spell { 2 } else { 1 })
                    })
                {
                    self.cast_cd = 0.12;
                    return;
                }
            }
        }
        if eval.actions.len() == 1 && eval.actions[0].spell == 7 && !self.echo_step(world, player) {
            self.cast_cd = 0.12;
            return;
        }
        // A grapple with nothing in reach is a free no-op, not a wasted cast.
        if eval.actions.len() == 1
            && eval.actions[0].spell == 31
            && aim_cell(world, eye, forward, 32.0).is_none()
        {
            self.cast_cd = 0.12;
            return;
        }

        if blood_price {
            self.health -= health_cost;
            self.spawn_voxel_burst(eye + forward * 0.4, [0.72, 0.16, 0.22], 8, 1.0, 0.03);
        } else {
            self.mana = (self.mana - eval.mana_cost * super::modifiers::mana_mul(self.site_modifier)).clamp(0.0, self.max_mana());
        }
        let forward = super::modifiers::wobble(self.site_modifier, forward, self.time);
        if eval.actions.iter().any(|a| a.mods.reckless) {
            // Never lethal: Reckless trades health for damage, not for a death.
            self.health = (self.health - 4.0).max(1.0);
        }
        self.last_cast = eval.actions[0].spell;
        self.cast_pose = 0.4;

        let total_actions = eval.actions.len();
        let up_axis = if forward.y.abs() > 0.98 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let mut played_cue = None;
        let mut played_pitch = 1.0;

        for (idx, act) in eval.actions.iter().enumerate() {
            let spell = act.spell;
            match spell {
                1 => {
                    self.health = (self.health + 30.0 + rank as f32 * 5.0).min(self.max_health());
                    if self.knows(6) {
                        self.hunger = (self.hunger + 12.0).min(100.0);
                    }
                    self.effect(eye, spell_color(1), 1.0);
                    self.spawn_voxel_burst(eye + forward * 0.6, spell_color(1), 18, 1.4, 0.045);
                    played_cue.get_or_insert(Cue::Mend);
                }
                4 => {
                    self.ward = if self.knows(9) { 9.0 } else { 6.0 };
                    self.ward_hits = if self.knows(10) {4} else {3};
                    self.effect(eye + forward * 0.7, spell_color(4), 0.7);
                    self.spawn_voxel_burst(eye + forward * 0.7, spell_color(4), 16, 1.1, 0.04);
                    played_cue.get_or_insert(Cue::Ward);
                }
                7 => {
                    if total_actions > 1 {
                        let _ = self.echo_step(world, player);
                    }
                }
                9 => {
                    // Luminous Drill: instant 3D laser beam that carves rock/ore and slices enemies
                    let angle = if total_actions > 1 {
                        let centered = idx as f32 - (total_actions - 1) as f32 * 0.5;
                        centered * eval.spread_deg.to_radians().max(0.045)
                    } else {
                        0.0
                    };
                    let dir = Quat::from_axis_angle(up_axis, angle) * forward;
                    self.cast_luminous_drill(world, eye, dir, act.mods, act.payload);
                    played_cue.get_or_insert(Cue::Spark);
                }
                75 => {
                    self.cast_ice_wall(world, eye, forward);
                    played_cue.get_or_insert(Cue::Frost);
                    played_pitch = 0.8;
                }
                30 => {
                    self.cast_time_snag(eye);
                    played_cue.get_or_insert(Cue::Echo);
                    played_pitch = 0.9;
                }
                31 => {
                    self.cast_grapple(world, player, eye, forward);
                    played_cue.get_or_insert(Cue::Spark);
                    played_pitch = 1.4;
                }
                32 => {
                    self.cast_leap(world, player, eye, forward);
                    played_cue.get_or_insert(Cue::Echo);
                    played_pitch = 1.25;
                }
                24 => {
                    self.cast_timberfall(world, eye, forward);
                    played_cue.get_or_insert(Cue::Impact);
                }
                25 => {
                    self.cast_gale(world, eye, forward);
                    played_cue.get_or_insert(Cue::Ward);
                }
                34 => {
                    let at = aim_cell(world, eye, forward, 8.0)
                        .map(|(cell, _)| {
                            (cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * world.voxel_scale.meters()
                        })
                        .unwrap_or(eye + forward * 2.0);
                    let created = act
                        .payload
                        .filter(|p| is_effect(*p))
                        .and_then(|p| growth::resolve_effect(world, p, at, None));
                    if let Some(follow) = act.follow.filter(|f| is_effect(*f)) {
                        growth::resolve_effect(world, follow, at, created);
                    }
                    self.spawn_voxel_burst(at, spell_color(34), 10, 1.1, 0.04);
                    played_cue.get_or_insert(Cue::Mend);
                }
                35 | 36 | 39 => {
                    let at = eye + forward * 2.0;
                    growth::resolve_effect(world, spell, at, None);
                    played_cue.get_or_insert(Cue::Bloom);
                }
                _ => {
                    if act.mods.unstable && (idx as u32 + self.time.to_bits()).is_multiple_of(5) {
                        self.spawn_voxel_burst(
                            eye + forward * 0.4,
                            [0.95, 0.55, 0.15],
                            6,
                            0.8,
                            0.03,
                        );
                        continue;
                    }
                    let dir = if total_actions == 2
                        && self.loadout.mode == 1
                        && eval.modifier_slots.is_empty()
                    {
                        Quat::from_axis_angle(up_axis, if idx == 0 { -0.055 } else { 0.055 })
                            * forward
                    } else if total_actions > 1 {
                        let step_rad = (eval.spread_deg.to_radians()
                            / (total_actions as f32 - 1.0).max(1.0))
                        .clamp(0.045, 0.14);
                        let centered = idx as f32 - (total_actions - 1) as f32 * 0.5;
                        Quat::from_axis_angle(up_axis, centered * step_rad) * forward
                    } else {
                        forward
                    };
                    let dir = if act.mods.reverse { -dir } else { dir };
                    let (origin, dir) = if spell == 29 {
                        self.meteor_launch(world, eye, dir)
                    } else {
                        (eye, dir)
                    };
                    self.emit_spell_modified(spell, origin, dir, act.payload, act.mods);
                    if let Some(shot) = self.projectiles.last_mut() {
                        shot.payload_mods = act.payload_mods;
                        if let Some(follow) = act.follow {
                            shot.follow = Some(follow);
                        }
                    }
                    if spell == 14 && !player.flying {
                        player.vel -= dir * 1.4;
                    }
                    // The viewmodel already supplies the muzzle flash. The old
                    // ten-orb burst at the aiming point covered distant tells.
                    // Keep a few small departing sparks beside the wand instead.
                    let muzzle = eye + forward * 0.65
                        + forward.cross(up_axis).normalize_or_zero() * 0.22 - up_axis * 0.14;
                    self.spawn_voxel_burst(muzzle, spell_color(spell), 4, 0.6, 0.02);
                    let (cue, pitch) = spell_audio(spell);
                    played_cue.get_or_insert(cue);
                    played_pitch = pitch;
                }
            }
        }

        if let Some(cue) = played_cue {
            sound_pitched(&mut self.sounds, cue, None, 0.7, played_pitch);
        }

        self.loadout.cursor = eval.last_slot;
        self.advance_wand();
        self.cast_delay_total = eval.cast_delay;
        self.recharge_total = eval.recharge_time;
        self.cast_cd = eval.cast_delay + eval.recharge_time;
        self.cast_total = self.cast_cd;
        if self.loadout.mode == 3 && eval.recharge_time > 0.0 {
            // The deck wrapped, so the Chaotic chassis deals a fresh order.
            self.shuffle_seed =
                super::discovery::mix32(self.shuffle_seed ^ self.time.to_bits() ^ 0x5eed_0001);
        }
    }

    /// A short, bounded physics shockwave. Rigid assemblies receive a real
    /// impulse while enemies only receive stagger and knockback, so a single
    /// melee finisher cannot fling every actor across the world.
    pub(super) fn kinetic_pulse(
        &mut self,
        world: &mut World,
        centre: Vec3,
        radius: f32,
        strength: f32,
    ) {
        if !centre.is_finite() || !radius.is_finite() || radius <= 0.0 {
            return;
        }
        let scale = world.voxel_scale.meters();
        world
            .sandbox
            .apply_radial_impulse(centre, radius, strength, scale);
        let radius_sq = radius * radius;
        for enemy in &mut self.enemies {
            let delta = enemy.center() - centre;
            let distance_sq = delta.length_squared();
            if distance_sq >= radius_sq {
                continue;
            }
            let falloff = 1.0 - (distance_sq / radius_sq).sqrt();
            enemy.knock += delta.normalize_or_zero() * (strength * falloff * 0.45);
            enemy.stagger = enemy.stagger.max(0.22 + falloff * 0.2);
        }
        if self.effects.len() < 12 {
            self.effect(centre, [0.56, 0.85, 0.78], radius * 0.16);
        }
    }

    /// Where a Meteor starts and which way it falls so it lands on the aimed
    /// block. It begins high above the target and toward the caster, but is
    /// lowered under any ceiling so it cannot detonate on a cave roof.
    pub(super) fn meteor_launch(&self, world: &World, eye: Vec3, aim: Vec3) -> (Vec3, Vec3) {
        let scale = world.voxel_scale.meters();
        let target = aim_cell(world, eye, aim, 60.0)
            .map(|(cell, _)| (cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * scale)
            .unwrap_or(eye + aim * 30.0);
        let mut height = 22.0_f32;
        if let Some((cell, _)) = aim_cell(world, target + Vec3::Y * 0.3, Vec3::Y, height) {
            let roof = cell.y as f32 * scale;
            height = (roof - target.y - 0.6).clamp(3.0, height);
        }
        let side = Vec3::new(-aim.x, 0.0, -aim.z).normalize_or_zero() * (height * 0.35);
        let origin = target + Vec3::Y * height + side;
        let dir = (target - origin).normalize_or_zero();
        (origin, if dir == Vec3::ZERO { Vec3::NEG_Y } else { dir })
    }

    /// Slows every foe nearby and crushes hostile bolts to a crawl.
    pub(super) fn cast_time_snag(&mut self, eye: Vec3) {
        for e in &mut self.enemies {
            if e.health > 0.0 && (e.center() - eye).length() < 9.0 {
                e.slow = e.slow.max(3.5);
                e.stagger = e.stagger.max(0.25);
            }
        }
        for shot in self.projectiles.iter_mut().filter(|p| p.hostile) {
            if (shot.pos - eye).length() < 12.0 {
                shot.velocity *= 0.35;
                shot.life += 1.0;
            }
        }
        self.effect(eye, spell_color(30), 2.4);
        self.spawn_voxel_burst(eye, spell_color(30), 22, 3.0, 0.04);
    }

    /// Hooks the aimed block and hauls the caster toward it.
    pub(super) fn cast_grapple(
        &mut self,
        world: &World,
        player: &mut Player,
        eye: Vec3,
        forward: Vec3,
    ) {
        let scale = world.voxel_scale.meters();
        let Some((cell, _)) = aim_cell(world, eye, forward, 32.0) else {
            return;
        };
        let target = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
        let to = (target - eye).normalize_or_zero();
        if to == Vec3::ZERO {
            return;
        }
        let pull = crate::world::speed_blocks(world.voxel_scale, 15.0);
        let lift = crate::world::speed_blocks(world.voxel_scale, 2.5);
        player.vel = to * pull + Vec3::Y * lift;
        player.on_ground = false;
        // A readable tether of motes from the hand to the hook.
        for i in 1..=8 {
            let at = eye.lerp(target, i as f32 / 8.0);
            self.spawn_voxel_burst(at, spell_color(31), 2, 0.4, 0.025);
        }
        self.effect(target, spell_color(31), 0.7);
    }

    /// Launches the caster up and forward. Usable in mid-air.
    pub(super) fn cast_leap(
        &mut self,
        world: &World,
        player: &mut Player,
        eye: Vec3,
        forward: Vec3,
    ) {
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        player.vel.y = crate::world::speed_blocks(world.voxel_scale, 9.0);
        player.vel += flat * crate::world::speed_blocks(world.voxel_scale, 5.0);
        player.on_ground = false;
        self.spawn_voxel_burst(eye - Vec3::Y * 1.2, spell_color(32), 14, 1.6, 0.035);
    }

    /// Fires the payload a carrier holds, from the carrier's position. Shared by
    /// On timer and On expire; afterwards the carrier holds no payload.
    pub(super) fn release_payload(&mut self, world: &mut World, shot: &mut Projectile) {
        let Some(payload) = shot.payload.take() else {
            return;
        };
        let p_dir = shot.velocity.normalize_or_zero();
        if is_effect(payload) {
            let created = growth::resolve_effect(world, payload, shot.pos, None);
            if let Some(follow) = shot.follow.filter(|f| is_effect(*f)) {
                growth::resolve_effect(world, follow, shot.pos, created);
            }
        } else if payload == 9 {
            self.cast_luminous_drill(world, shot.pos, p_dir, shot.payload_mods, None);
        } else {
            self.emit_spell_modified(payload, shot.pos, p_dir, None, shot.payload_mods);
        }
        self.spawn_voxel_burst(shot.pos, shot.color(), 8, 1.4, 0.03);
    }

    /// Swaps the caster to where a Teleport bolt landed, backing off the wall
    /// until the body fits. `Player::pos` is the feet, so the body is placed
    /// one eye-height below the bolt. Returns false and does nothing if no
    /// nearby spot is clear.
    pub(super) fn teleport_to_impact(
        &mut self,
        world: &World,
        player: &mut Player,
        at: Vec3,
        travel: Vec3,
    ) -> bool {
        let scale = world.voxel_scale.meters();
        let back = -travel.normalize_or_zero();
        let eye_height = (player.eye(world.voxel_scale) - player.pos).y * scale;
        for lift in [0.0_f32, 0.5, 1.0, 1.5, 2.0, 2.5] {
            for step in [0.35_f32, 0.7, 1.1, 1.6, 2.2] {
                let feet = at + back * step + Vec3::Y * lift - Vec3::Y * eye_height;
                let (min, max) = super::super::player_aabb(
                    feet / scale,
                    world.voxel_scale,
                    player.scale,
                    player.crouch_t,
                );
                if collides_world(world, min, max) {
                    continue;
                }
                let origin = player.pos * scale;
                player.pos = feet / scale;
                player.vel = Vec3::ZERO;
                self.invulnerable = self.invulnerable.max(0.4);
                self.effect(origin + Vec3::Y, spell_color(72), 1.0);
                self.effect(feet + Vec3::Y, spell_color(72), 1.0);
                sound(&mut self.sounds, Cue::Echo, None, 0.8);
                return true;
            }
        }
        false
    }

    /// Pours a bounded puddle of water at an impact and soaks nearby foes.
    pub(super) fn pour_water(&mut self, world: &mut World, at: Vec3) {
        use crate::world::BlockId as B;
        let scale = world.voxel_scale.meters();
        let centre = (at / scale).floor().as_ivec3();
        let mut cells: Vec<(i32, IVec3)> = Vec::new();
        for dx in -2..=2 {
            for dy in -1..=2 {
                for dz in -2..=2 {
                    let d2 = dx * dx + dy * dy + dz * dz;
                    let cell = centre + IVec3::new(dx, dy, dz);
                    if d2 <= 5 && world.get(cell) == B::Air {
                        // Nearest first, lowest first, so it pools instead of floating.
                        cells.push((d2 * 4 + dy, cell));
                    }
                }
            }
        }
        cells.sort_by_key(|(k, c)| (*k, c.x, c.y, c.z));
        for (_, cell) in cells.into_iter().take(18) {
            world.set(cell, B::Water);
            world.activity.wake(cell);
            self.mark_voxel_dirty(world, cell);
        }
        for e in &mut self.enemies {
            if e.health > 0.0 && (e.center() - at).length() < 3.5 {
                e.status.apply(super::status::Element::Wet, e.affixes);
            }
        }
        sound(&mut self.sounds, Cue::Splash, Some(at), 0.8);
    }

    /// Poisons every foe within 3.4 m of the impact that it can see.
    pub(super) fn venom_nova(&mut self, world: &World, at: Vec3, damage_mul: f32) {
        use super::status::Element;
        for e in &mut self.enemies {
            if e.health <= 0.0 || (e.center() - at).length() >= 3.4 || !line(world, at, e.center())
            {
                continue;
            }
            e.status.apply(Element::Toxic, e.affixes);
            e.status.apply(Element::Toxic, e.affixes);
            e.hurt(8.0 * damage_mul * e.affixes.damage_taken());
            e.flash = 0.16;
        }
        self.spawn_voxel_burst(at, Element::Toxic.color(), 26, 3.0, 0.045);
        self.effect(at, Element::Toxic.color(), 2.2);
        sound(&mut self.sounds, Cue::Bloom, Some(at), 0.8);
    }

    /// Raises a 3 m wide, 2.5 m tall ice barrier three metres ahead of the
    /// caster. Only empty cells are filled, and the total is bounded so a very
    /// fine voxel scale cannot flood the sim.
    pub(super) fn cast_ice_wall(&mut self, world: &mut World, eye: Vec3, forward: Vec3) {
        use crate::world::BlockId as B;
        let scale = world.voxel_scale.meters();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let flat = if flat == Vec3::ZERO { Vec3::NEG_Z } else { flat };
        let side = Vec3::new(-flat.z, 0.0, flat.x);
        let base = eye + flat * 3.0 - Vec3::Y * 1.5;
        let step = scale * 0.9;
        // Sample both endpoints of each span, or the wall stops short of its
        // stated 3 m x 2.5 m at coarse voxel sizes.
        let cols = (3.0 / step).ceil() as i32;
        let rows = (2.5 / step).ceil() as i32;
        let mut made = 0;
        for ci in 0..=cols {
            let u = (-1.5 + ci as f32 * step).min(1.5);
            for ri in 0..=rows {
                let h = (ri as f32 * step).min(2.5);
                let cell = ((base + side * u + Vec3::Y * h) / scale).floor().as_ivec3();
                if made < 400 && world.get(cell) == B::Air {
                    world.set(cell, B::Ice);
                    world.activity.wake(cell);
                    self.mark_voxel_dirty(world, cell);
                    made += 1;
                }
            }
        }
        self.effect(base + Vec3::Y, spell_color(75), 1.6);
        self.spawn_voxel_burst(base + Vec3::Y, spell_color(75), 20, 2.0, 0.04);
    }

    pub fn cancel_cast_input(&mut self) {
        self.queued_cast = false;
        self.queued_cycle = false;
    }

    pub fn blast_visual(&mut self, pos: Vec3, radius: f32, normal: Vec3, color: [f32; 3]) {
        if self.blasts.len() >= 8 {
            self.blasts.remove(0);
        }
        self.blasts.push(crate::tool_effects::Blast {
            pos,
            normal,
            radius,
            color,
            age: 0.0,
        });
    }

    pub fn blast_feedback(&mut self, pos: Vec3, radius: f32, normal: Vec3, color: [f32; 3]) {
        sound(&mut self.sounds, Cue::Burst, Some(pos), 1.0);
        self.blast_visual(pos, radius, normal, color);
        for e in &mut self.enemies {
            let d = e.center() - pos;
            if d.length() < radius {
                e.health -= 48.0 * (1.0 - d.length() / radius);
                e.knock = d.normalize_or_zero() * 6.0;
                e.flash = 0.2;
            }
        }
    }
}

#[cfg(test)]
mod acceptance {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};
    #[test]
    fn wand_burst_detonates_at_point_blank_walls_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(-0.05, 150.0, scale.meters() * 0.5) / scale.meters();
            p.flying = true;
            let eye = p.eye(scale) * scale.meters();
            let cell = IVec3::new(0, (eye.y / scale.meters()).floor() as i32, 0);
            w.set(cell, crate::world::BlockId::Stone);
            let mut a = Adventure::default();
            a.wand = 3;
            a.invulnerable = 0.0;
            a.emit_spell(3, eye, Vec3::X, None);
            w.adventure = a;
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert!(w.adventure.projectiles.is_empty(), "{scale:?}");
            let (point, radius) = w
                .adventure
                .take_terrain_blast()
                .expect("burst crater queued");
            assert!(point.x <= 0.0 && point.x >= -0.05);
            assert_eq!(radius, 2.0);
            assert!(!w.adventure.blasts.is_empty());
            assert!(w.adventure.health < 100.0);
        }
    }
    #[test]
    fn wand_taps_survive_zero_step_frames_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
            p.flying = true;
            w.adventure.wand = 3;
            w.adventure.loadout.slots = [0, 2, EMPTY, EMPTY];
            super::super::tick(&mut w, &mut p, 0.0, true, false, false);
            assert!(w.adventure.projectiles.is_empty());
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert_eq!(w.adventure.projectiles.len(), 1, "{scale:?}");
            assert!(w.adventure.mana < 90.0);
            assert_eq!(w.adventure.loadout.cursor, 1);
            super::super::tick(&mut w, &mut p, 0.0, false, true, false);
            super::super::tick(&mut w, &mut p, 1.0 / 60.0, false, false, false);
            assert_eq!(w.adventure.loadout.cursor, 0);
        }
    }
    #[test]
    fn wand_patterns_prepaid_payload_and_twin_at_every_scale() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let mut w = World::with_seed(WorldType::Superflat, scale, 17);
            let mut p = Player::new();
            p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
            let eye = p.eye(scale) * scale.meters();
            let mut a = Adventure::default();
            a.wand = 3;
            a.loadout.slots = [0, 3, EMPTY, EMPTY];
            a.loadout.mode = 2;
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.projectiles.len(), 1);
            assert_eq!(a.projectiles[0].payload, Some(3));
            assert_eq!(a.mana, 58.0);
            assert_eq!(a.loadout.cursor, 0);
            a.projectiles.clear();
            a.mana = 100.0;
            a.loadout.mode = 1;
            a.cast_wand(&mut w, &mut p, eye, Vec3::Y);
            assert_eq!(a.projectiles.len(), 2);
            assert!(
                a.projectiles[0]
                    .velocity
                    .distance(a.projectiles[1].velocity)
                    > 1.0
            );
            assert!((a.mana - 80.8).abs() < 0.01);
        }
    }
    #[test]
    fn stacked_modifiers_are_capped_instead_of_multiplying_without_bound() {
        let mut b = Adventure::default();
        b.wand = 3;
        b.rare_spells = !0;
        // Four damage modifiers in a row. The old evaluator multiplied each
        // copy, reaching 1.65^4 * 1.30 = 5.8x before rank bonuses.
        b.loadout.slots = [21, 21, 21, 20];
        b.loadout.aux_slots = [0, EMPTY, EMPTY, EMPTY];
        let eval = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        assert!(eval.actions[0].mods.damage_mul <= 2.6);
        assert!(eval.actions[0].mods.bounces <= 4);
    }

    #[test]
    fn multicast_draw_count_is_bounded() {
        let mut b = Adventure::default();
        b.wand = 3;
        b.rare_spells = !0;
        // Trifurcate copies used to push draws to six. Five projectiles are
        // available, so an uncapped evaluator would fire all five.
        b.loadout.slots = [17, 17, 17, 0];
        b.loadout.aux_slots = [2, 3, 0, 2];
        let eval = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        assert!(eval.actions.len() <= 4, "{} actions", eval.actions.len());
    }

    #[test]
    fn new_utility_spells_cast_and_new_modifiers_attach_to_the_next_spell() {
        let mut b = Adventure::default();
        b.wand = 3;
        b.rare_spells = !0;
        b.loadout.slots = [24, EMPTY, EMPTY, EMPTY];
        let utility = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        assert_eq!(utility.actions[0].spell, 24);
        assert!(utility.modifier_slots.is_empty());

        b.loadout.slots = [26, 0, EMPTY, EMPTY];
        let modified = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        assert_eq!(modified.actions[0].spell, 0);
        assert!(modified.actions[0].mods.volatile);
    }

    #[test]
    fn spell_blast_erases_without_running_the_mining_inventory() {
        use crate::world::{BlockId as B, WorldType};
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
        let cell = IVec3::new(0, 100, 0);
        w.set(cell, B::Stone);
        let before = w
            .inventory
            .count_item(crate::world::inventory::Item::Block(B::Stone));
        let mut blocks = std::collections::HashSet::new();
        let mut chunks = std::collections::HashSet::new();
        assert!(crate::world::expedition::spell_blast(
            &mut w,
            cell,
            &mut blocks,
            &mut chunks
        ));
        assert_eq!(w.get(cell), B::Air);
        assert_eq!(
            w.inventory
                .count_item(crate::world::inventory::Item::Block(B::Stone)),
            before,
            "spell craters must not auto-harvest rubble"
        );
    }

    #[test]
    fn noita_modifier_chains_combine_multicast_trigger_and_rapid_flux() {
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0) / w.voxel_scale.meters();
        let eye = p.eye(w.voxel_scale) * w.voxel_scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        // Deck: [Rapid Flux (22)] -> [Bifurcate (16)] -> [Homing (19)] -> [Trigger (18)] -> [Spark (0)] -> [Burst (3)] -> [Frost (2)]
        a.loadout.slots = [22, 16, 19, 18];
        a.loadout.aux_slots = [0, 3, 2, EMPTY];
        a.loadout.cursor = 0;
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        // Bifurcate draws 2 projectiles: Spark (with Burst payload) and Frost Lance, both with Homing!
        assert_eq!(a.projectiles.len(), 2);
        assert_eq!(a.projectiles[0].spell, 0);
        assert_eq!(a.projectiles[0].payload, Some(3));
        assert!(a.projectiles[0].mods.homing);
        assert_eq!(a.projectiles[1].spell, 2);
        assert!(a.projectiles[1].mods.homing);
        // Cursor wrapped past slot 6 back to 0!
        assert_eq!(a.loadout.cursor, 0);
    }

    #[test]
    fn splash_emits_a_bounded_ring_and_droplets() {
        let mut a = Adventure::default();
        a.splash(Vec3::new(2.0, 64.5, -3.0), 0.85);
        assert_eq!(a.splashes.len(), 1);
        assert!(a.splashes[0].radius > 0.5);
        assert!(!a.particles.is_empty());
        for _ in 0..40 {
            a.splash(Vec3::new(2.0, 64.5, -3.0), 0.85);
        }
        assert_eq!(a.splashes.len(), 24);
    }

    #[test]
    fn noita_world_interactive_spells_and_8_slot_save_roundtrip() {
        use crate::world::BlockId as B;
        let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
        let center = IVec3::new(10, 80, 10);
        w.set(center, B::Water);
        w.set(center + IVec3::X, B::Stone);
        w.set(center + IVec3::Z, B::Stone);

        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        let pos = (center.as_vec3() + Vec3::splat(0.5)) * w.voxel_scale.meters();

        // 1. Cryo Nova freezes Water into walkable Ice and emits 3D translucent voxel particles
        a.cryo_bloom(&mut w, pos, 1.5);
        assert_eq!(w.get(center), B::Ice);
        assert!(!a.particles.is_empty());

        // 2. Magma Glob thermal bloom melts Ice back to Water / Steam and heats thermal cells
        a.thermal_bloom(&mut w, pos, 1.8, true);
        assert!(matches!(w.get(center), B::Water | B::Steam | B::Fire));

        // 3. Touch of Gold transmutes Stone into Crystal / Ore / GlowCap
        a.alchemical_transmute(&mut w, pos, 2.0);
        assert!(matches!(
            w.get(center + IVec3::X),
            B::Crystal | B::CopperOre | B::IronOre | B::GlowCap
        ));

        // 4. Lumen Drill carves solid blocks along its beam and spawns laser + voxel sparks
        w.set(center + IVec3::Z, B::Stone);
        let drill_origin = (center.as_vec3() + Vec3::new(0.5, 0.5, -1.0)) * w.voxel_scale.meters();
        a.cast_luminous_drill(&mut w, drill_origin, Vec3::Z, ShotMods::default(), None);
        assert_eq!(w.get(center + IVec3::Z), B::Air);
        assert!(!a.lasers.is_empty());

        // 5. 8-slot wand deck save/decode roundtrip preserves all 8 slots and mode 3
        a.spell = 10;
        a.loadout.slots = [16, 19, 10, 11];
        a.loadout.aux_slots = [22, 18, 12, 15];
        a.loadout.cursor = 5;
        a.loadout.mode = 3;
        let mut bytes = Vec::new();
        a.encode(&mut bytes);
        let mut idx = 0usize;
        let decoded = Adventure::decode(
            &mut || {
                let word = u32::from_le_bytes(bytes[idx..idx + 4].try_into().unwrap());
                idx += 4;
                Ok(word)
            },
            true,
            true,
        )
        .expect("decode 8-slot wand");
        assert_eq!(idx, bytes.len());
        assert_eq!(decoded.spell, 10);
        assert_eq!(decoded.loadout.slots, [16, 19, 10, 11]);
        assert_eq!(decoded.loadout.aux_slots, [22, 18, 12, 15]);
        assert_eq!(decoded.loadout.cursor, 5);
        assert_eq!(decoded.loadout.mode, 3);

        // 6. Direct drag-and-drop wand slot swapping & trigger-payload slot tracking
        let mut b = Adventure::default();
        b.wand = 3;
        b.rare_spells = !0;
        b.loadout.slots = [18, 10, 11, 17];
        b.loadout.aux_slots = [12, 0, 0, 0];
        b.swap_wand_slots(0, 4);
        assert_eq!(b.loadout.slot(0), 12);
        assert_eq!(b.loadout.slot(4), 18);
        b.swap_wand_slots(0, 4);
        let eval = evaluate_cast_block(
            b.wand,
            b.rank(),
            b.discoveries,
            b.rare_spells,
            &b.loadout,
            0,
        );
        assert_eq!(eval.actions.len(), 1);
        assert_eq!(eval.actions[0].slot, 1);
        assert_eq!(eval.actions[0].spell, 10);
        assert_eq!(eval.actions[0].payload, Some(11));
        assert_eq!(eval.actions[0].payload_slot, Some(2));
    }

    fn blank(wand: u32) -> Adventure {
        let mut a = Adventure::default();
        a.wand = wand;
        a.loadout.slots = [EMPTY; 4];
        a.loadout.aux_slots = [EMPTY; 4];
        a.mana = 100.0;
        a
    }

    #[test]
    fn programmed_deck_can_be_empty_and_still_runs_divides_and_branches() {
        let empty = blank(3);
        let eval = evaluate_cast_block(3, 0, 0, 0, &empty.loadout, 0);
        assert!(eval.actions.is_empty());
        assert_eq!(eval.mana_cost, 0.0);

        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
        let mut player = Player::new();
        player.flying = true;
        player.pos = Vec3::new(0.0, 150.0, 0.0);
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let mut quiet = blank(3);
        quiet.cast_wand(&mut world, &mut player, eye, Vec3::NEG_Z);
        assert!(quiet.projectiles.is_empty());
        assert_eq!(quiet.mana, 100.0);

        let mut divided = blank(3);
        divided.loadout.slots = [40, 0, EMPTY, EMPTY];
        let eval = evaluate_programmed(3, 0, 0, 0, &divided.loadout, 0, 1.0, false, 0);
        assert_eq!(eval.actions.len(), 2);
        assert!(eval.actions.iter().all(|a| a.spell == 0));
        assert_eq!(eval.mana_cost, 20.0);
        divided.cast_wand(&mut world, &mut player, eye, Vec3::NEG_Z);
        assert_eq!(divided.projectiles.len(), 2);
        assert_eq!(divided.mana, 80.0);

        let mut quad = blank(3);
        quad.loadout.slots = [41, 0, EMPTY, EMPTY];
        let eval = evaluate_programmed(3, 0, 0, 0, &quad.loadout, 0, 1.0, true, 0);
        assert_eq!(eval.actions.len(), 4);
        assert_eq!(eval.mana_cost, 28.0);

        let mut healthy = blank(3);
        healthy.loadout.slots = [47, 0, 2, EMPTY];
        let skipped = evaluate_programmed(3, 0, 0, 0, &healthy.loadout, 0, 1.0, false, 0);
        assert_eq!(skipped.actions.len(), 1);
        assert_eq!(skipped.actions[0].spell, 2);
        let wounded = evaluate_programmed(3, 0, 0, 0, &healthy.loadout, 0, 0.2, false, 0);
        assert_eq!(wounded.actions[0].spell, 0);

        let mut only = blank(3);
        only.loadout.slots = [47, 0, EMPTY, EMPTY];
        let held = evaluate_programmed(3, 0, 0, 0, &only.loadout, 0, 1.0, false, 0);
        assert!(
            held.actions.is_empty(),
            "a failed condition does not invent a spark"
        );

        let mut gated = blank(3);
        gated.loadout.slots = [48, 3, 0, EMPTY];
        let hidden = evaluate_programmed(3, 0, 0, 0, &gated.loadout, 0, 1.0, false, 0);
        assert_eq!(hidden.actions[0].spell, 0);
        let seen = evaluate_programmed(3, 0, 0, 0, &gated.loadout, 0, 1.0, true, 0);
        assert_eq!(seen.actions[0].spell, 3);

        let mut alpha = blank(3);
        alpha.loadout.slots = [0, 49, 3, EMPTY];
        let copied = evaluate_programmed(3, 0, 0, 0, &alpha.loadout, 1, 1.0, false, 0);
        assert_eq!(copied.actions[0].spell, 0);

        let mut path = blank(3);
        path.loadout.slots = [50, 0, EMPTY, EMPTY];
        assert_eq!(
            evaluate_programmed(3, 0, 0, 0, &path.loadout, 0, 1.0, false, 0).actions[0]
                .mods
                .path,
            1
        );
        assert_eq!(
            evaluate_programmed(3, 0, 0, 0, &path.loadout, 0, 1.0, false, 2).actions[0]
                .mods
                .path,
            3
        );

        let mut triggered = blank(3);
        triggered.loadout.slots = [19, 18, 0, 56];
        triggered.loadout.aux_slots = [3, EMPTY, EMPTY, EMPTY];
        let cast = evaluate_programmed(3, 0, 0, 0, &triggered.loadout, 0, 1.0, true, 0);
        assert_eq!(cast.actions[0].spell, 0);
        assert!(cast.actions[0].mods.homing);
        assert!(cast.actions[0].mods.element.is_none());
        assert_eq!(cast.actions[0].payload, Some(3));
        assert_eq!(
            cast.actions[0].payload_mods.element,
            Some(super::status::Element::Fire)
        );
        assert!(!cast.actions[0].payload_mods.homing);

        let mut curve = blank(3);
        curve.loadout.slots = [42, EMPTY, EMPTY, EMPTY];
        let spark = evaluate_programmed(3, 0, 0, 0, &curve.loadout, 0, 1.0, false, 0);
        assert_eq!(spark.actions[0].spell, 0);
        assert_eq!(spark.actions[0].mods.path, 1);
    }

    #[test]
    fn curve_turns_a_fired_bolt() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17);
        let mut player = Player::new();
        player.flying = true;
        player.pos = Vec3::new(0.0, 150.0, 0.0);
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let mut a = blank(3);
        a.loadout.slots = [42, 0, EMPTY, EMPTY];
        a.cast_wand(&mut world, &mut player, eye, Vec3::NEG_Z);
        let before = a.projectiles[0].velocity;
        world.adventure = a;
        super::super::tick(&mut world, &mut player, 1.0 / 60.0, false, false, false);
        let after = world.adventure.projectiles[0].velocity;
        let turned = before.cross(after).length();
        assert!(
            turned > 1.0,
            "curve should yaw the bolt, cross was {turned}"
        );
    }
}

#[cfg(test)]
mod audio_mapping {
    use super::*;

    #[test]
    fn spell_families_do_not_share_one_projectile_cue() {
        let sparks = spell_audio(0).0 as usize;
        for spell in [2, 3, 5, 6, 7] {
            assert_ne!(
                spell_audio(spell).0 as usize,
                sparks,
                "spell {spell} should have its own audio family"
            );
        }
        assert!((spell_audio(2).1 - spell_audio(7).1).abs() > 0.2);
        assert!((0.60..=1.70).contains(&spell_audio(6).1));
    }
}


#[cfg(test)]
mod new_spell_tests {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{BlockId as B, WorldType},
    };

    const SCALES: [VoxelScalePreset; 3] = [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ];

    /// A flying caster in open air with a full top-tier wand and a full tank.
    fn rig(scale: VoxelScalePreset) -> (World, Player, Vec3, Adventure) {
        let w = World::with_seed(WorldType::Superflat, scale, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
        p.flying = true;
        let eye = p.eye(scale) * scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        a.mana = 100.0;
        a.health = 100.0;
        a.loadout.aux_slots = [EMPTY; 4];
        (w, p, eye, a)
    }

    fn run(w: &mut World, p: &mut Player, frames: usize) {
        for _ in 0..frames {
            super::super::tick(w, p, 1.0 / 60.0, false, false, false);
        }
    }

    #[test]
    fn new_modifier_cards_flag_the_next_bolt() {
        let mut b = Adventure::default();
        b.wand = 3;
        b.loadout.slots = [64, 65, 66, 67];
        b.loadout.aux_slots = [0, EMPTY, EMPTY, EMPTY];
        let eval = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        let m = eval.actions[0].mods;
        assert_eq!(eval.actions[0].spell, 0);
        assert!(m.reverse && m.reckless);
        assert_eq!(m.phase, 1);
        assert!((m.gravity - 1.0).abs() < 1e-6);
        assert!(m.damage_mul > 1.5 && m.damage_mul <= 2.6);

        b.loadout.slots = [68, 69, 0, EMPTY];
        let eval = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0);
        assert!(eval.actions[0].mods.blood_price && eval.actions[0].mods.fork);
    }

    #[test]
    fn stacking_new_modifiers_stays_bounded() {
        let mut b = Adventure::default();
        b.wand = 3;
        b.loadout.slots = [65, 65, 65, 66];
        b.loadout.aux_slots = [66, 66, 67, 0];
        let m = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0).actions[0].mods;
        assert!(m.gravity <= 2.0, "gravity {}", m.gravity);
        assert!(m.phase <= 2, "phase {}", m.phase);
        assert!(m.damage_mul <= 2.6);
    }

    #[test]
    fn on_timer_fires_its_payload_mid_flight() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.slots = [70, 0, 3, EMPTY];
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert_eq!(a.projectiles[0].payload, Some(3));
        assert!(a.projectiles[0].mods.timer > 0.0);
        w.adventure = a;
        run(&mut w, &mut p, 50);
        let shots = &w.adventure.projectiles;
        assert!(shots.iter().any(|s| s.spell == 3), "payload never launched");
        assert!(
            shots.iter().any(|s| s.spell == 0 && s.payload.is_none()),
            "the carrier must keep flying without a payload"
        );
    }

    #[test]
    fn blood_price_pays_in_health_and_refuses_when_low() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.slots = [68, 0, EMPTY, EMPTY];
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert_eq!(a.mana, 100.0, "mana must be untouched");
        assert!((a.health - 94.0).abs() < 0.01, "health {}", a.health);

        a.projectiles.clear();
        a.loadout.cursor = 0;
        a.health = 8.0;
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert!(a.projectiles.is_empty(), "must not cast at low health");
        assert_eq!(a.health, 8.0);
    }

    #[test]
    fn reckless_never_kills_its_caster() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.slots = [67, 0, EMPTY, EMPTY];
        a.health = 3.0;
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert!(a.health >= 1.0 && a.health < 3.0, "health {}", a.health);
        assert!(a.projectiles[0].mods.damage_mul > 1.5);
    }

    #[test]
    fn meteor_falls_from_above_onto_the_aimed_block_at_every_scale() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            let s = scale.meters();
            let cell = (eye / s).floor().as_ivec3() + IVec3::new(0, 0, -8);
            w.set(cell, B::Stone);
            let target = (cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5)) * s;
            a.loadout.slots = [29, EMPTY, EMPTY, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.projectiles.len(), 1, "{scale:?}");
            let shot = &a.projectiles[0];
            assert_eq!(shot.spell, 29);
            assert!(
                shot.pos.y > target.y + 15.0,
                "{scale:?}: meteor starts at {} for a target at {}",
                shot.pos.y,
                target.y
            );
            let aimed = (target - shot.pos).normalize();
            assert!(
                aimed.dot(shot.velocity.normalize()) > 0.99,
                "{scale:?}: meteor is not heading for the target"
            );
        }
    }

    #[test]
    fn grapple_hauls_the_caster_toward_the_hook_and_is_free_without_one() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            let s = scale.meters();
            a.loadout.slots = [31, EMPTY, EMPTY, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.mana, 100.0, "{scale:?}: a miss must be free");
            assert_eq!(p.vel, Vec3::ZERO, "{scale:?}");

            let cell = (eye / s).floor().as_ivec3() + IVec3::new(0, 0, -10);
            w.set(cell, B::Stone);
            a.loadout.cursor = 0;
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert!(a.mana < 100.0, "{scale:?}: a hit must cost mana");
            assert!(p.vel.z < 0.0, "{scale:?}: pulled the wrong way {:?}", p.vel);
        }
    }

    #[test]
    fn leap_launches_the_caster_up_and_forward() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            a.loadout.slots = [32, EMPTY, EMPTY, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert!(p.vel.y > 0.0 && p.vel.z < 0.0, "{scale:?}: {:?}", p.vel);
            assert!(a.mana < 100.0);
        }
    }

    #[test]
    fn phase_passes_through_one_wall_and_then_behaves_normally() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            let s = scale.meters();
            let base = (eye / s).floor().as_ivec3();
            for dx in 3..5 {
                w.set(base + IVec3::new(dx, 0, 0), B::Stone);
            }
            let mods = ShotMods {
                phase: 1,
                ..Default::default()
            };
            a.emit_spell_modified(0, eye, Vec3::X, None, mods);
            w.adventure = a;
            run(&mut w, &mut p, 30);
            let shots = &w.adventure.projectiles;
            assert_eq!(shots.len(), 1, "{scale:?}: the bolt should have survived the wall");
            assert_eq!(shots[0].mods.phase, 0, "{scale:?}");
            assert!(shots[0].pos.x > eye.x + 6.0 * s, "{scale:?}: x = {}", shots[0].pos.x);
        }
    }

    #[test]
    fn fork_splits_into_three_weak_sparks_that_cannot_fork_again() {
        let scale = VoxelScalePreset::OneMeter;
        let mut w = World::with_seed(WorldType::Superflat, scale, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(-0.05, 150.0, scale.meters() * 0.5) / scale.meters();
        p.flying = true;
        let eye = p.eye(scale) * scale.meters();
        w.set(IVec3::new(0, (eye.y / scale.meters()).floor() as i32, 0), B::Stone);
        let mut a = Adventure::default();
        a.wand = 3;
        a.invulnerable = 0.0;
        let mods = ShotMods {
            fork: true,
            ..Default::default()
        };
        a.emit_spell_modified(0, eye, Vec3::X, None, mods);
        w.adventure = a;
        run(&mut w, &mut p, 1);
        let shots = &w.adventure.projectiles;
        assert_eq!(shots.len(), 3, "a forked impact must leave three children");
        assert!(
            shots
                .iter()
                .all(|s| s.spell == 0 && !s.mods.fork && s.mods.damage_mul < 0.5)
        );
    }

    #[test]
    fn gravity_bends_a_bolt_downward() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        let mods = ShotMods {
            gravity: 1.0,
            ..Default::default()
        };
        a.emit_spell_modified(0, eye, Vec3::X, None, mods);
        w.adventure = a;
        run(&mut w, &mut p, 20);
        assert!(w.adventure.projectiles[0].velocity.y < -1.0);
    }

    #[test]
    fn reverse_fires_behind_the_caster() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.slots = [64, 0, EMPTY, EMPTY];
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert!(a.projectiles[0].velocity.z > 1.0, "{:?}", a.projectiles[0].velocity);
    }
}

#[cfg(test)]
mod time_snag_tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};

    #[test]
    fn time_snag_slows_only_nearby_foes_and_crushes_hostile_bolts() {
        let scale = VoxelScalePreset::OneMeter;
        let mut w = World::with_seed(WorldType::Superflat, scale, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0);
        p.flying = true;
        let eye = p.eye(scale) * scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        a.mana = 100.0;
        a.loadout.slots = [30, EMPTY, EMPTY, EMPTY];
        a.loadout.aux_slots = [EMPTY; 4];

        a.enemies
            .push(Enemy::new(IVec3::ZERO, 0, 0, eye + Vec3::new(3.0, -1.5, 0.0), 1));
        a.enemies
            .push(Enemy::new(IVec3::ZERO, 1, 0, eye + Vec3::new(40.0, -1.5, 0.0), 1));
        a.emit_spell(0, eye + Vec3::X * 4.0, Vec3::X, None);
        a.projectiles[0].hostile = true;
        let before = a.projectiles[0].velocity.length();

        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);

        assert!(a.enemies[0].slow >= 3.5, "near foe slow {}", a.enemies[0].slow);
        assert_eq!(a.enemies[1].slow, 0.0, "a distant foe must be untouched");
        let after = a.projectiles[0].velocity.length();
        assert!(
            (after / before - 0.35).abs() < 0.01,
            "hostile bolt went from {before} to {after}"
        );
        assert!(a.mana < 100.0, "the cast must cost mana");
    }
}

#[cfg(test)]
mod noita_mechanics_tests {
    use super::*;
    use crate::{
        fluid::VoxelScalePreset,
        world::{BlockId as B, WorldType},
    };

    const SCALES: [VoxelScalePreset; 3] = [
        VoxelScalePreset::OneMeter,
        VoxelScalePreset::HalfMeter,
        VoxelScalePreset::QuarterMeter,
    ];

    fn rig(scale: VoxelScalePreset) -> (World, Player, Vec3, Adventure) {
        let w = World::with_seed(WorldType::Superflat, scale, 17);
        let mut p = Player::new();
        p.pos = Vec3::new(0.0, 150.0, 0.0) / scale.meters();
        p.flying = true;
        let eye = p.eye(scale) * scale.meters();
        let mut a = Adventure::default();
        a.wand = 3;
        a.rare_spells = !0;
        a.mana = 100.0;
        a.health = 100.0;
        a.loadout.aux_slots = [EMPTY; 4];
        (w, p, eye, a)
    }

    fn run(w: &mut World, p: &mut Player, frames: usize) {
        for _ in 0..frames {
            super::super::tick(w, p, 1.0 / 60.0, false, false, false);
        }
    }

    /// An enemy whose body centre sits exactly at `centre`.
    fn enemy_at(centre: Vec3, slot: u32) -> Enemy {
        let mut e = Enemy::new(IVec3::ZERO, slot, 0, centre, 1);
        let delta = centre - e.center();
        e.pos += delta;
        e
    }

    #[test]
    fn chainsaw_is_a_near_free_bolt_that_only_reaches_a_few_metres() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.slots = [71, EMPTY, EMPTY, EMPTY];
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.projectiles.len(), 1);
        assert_eq!(a.projectiles[0].spell, 71);
        assert!(a.projectiles[0].life <= 0.2, "life {}", a.projectiles[0].life);
        assert!((a.mana - 99.0).abs() < 0.01, "mana {}", a.mana);
        assert_eq!(spell_hit_damage(71), 7.0, "chainsaw must not inherit the 22 floor");
        assert_eq!(spell_hit_damage(0), 22.0, "older spells keep the floor");
        w.adventure = a;
        run(&mut w, &mut p, 15);
        assert!(w.adventure.projectiles.is_empty(), "the chainsaw must fizzle out");
    }

    #[test]
    fn bounce_orb_ricochets_off_a_wall_instead_of_dying() {
        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        let base = (eye / 1.0).floor().as_ivec3();
        w.set(base + IVec3::new(4, 0, 0), B::Stone);
        a.emit_spell_modified(76, eye, Vec3::X, None, ShotMods::default());
        assert_eq!(a.projectiles[0].mods.bounces, 4);
        w.adventure = a;
        run(&mut w, &mut p, 18);
        let shots = &w.adventure.projectiles;
        assert_eq!(shots.len(), 1, "the orb should have bounced, not been destroyed");
        assert!(shots[0].velocity.x < 0.0, "velocity {:?}", shots[0].velocity);
        assert!(shots[0].mods.bounces < 4);
    }

    #[test]
    fn teleport_bolt_swaps_the_caster_to_the_impact_at_every_scale() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            let s = scale.meters();
            let start = p.pos * s;
            let wall = (eye / s).floor().as_ivec3() + IVec3::new(0, 0, -(8.0 / s) as i32);
            w.set(wall, B::Stone);
            a.loadout.slots = [72, EMPTY, EMPTY, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.projectiles.len(), 1, "{scale:?}");
            w.adventure = a;
            run(&mut w, &mut p, 40);
            assert!(w.adventure.projectiles.is_empty(), "{scale:?}: bolt still flying");
            let now = p.pos * s;
            assert!(
                start.z - now.z >= 4.0,
                "{scale:?}: only moved {} m",
                start.z - now.z
            );
            assert!(
                now.z > (wall.z + 1) as f32 * s - 0.001,
                "{scale:?}: landed inside the wall, z = {}",
                now.z
            );
            let (min, max) = crate::world::player_aabb(p.pos, scale, p.scale, p.crouch_t);
            assert!(!collides_world(&w, min, max), "{scale:?}: landed inside solid blocks");
        }
    }

    #[test]
    fn flood_bolt_pours_a_bounded_puddle_and_soaks_foes() {
        let scale = VoxelScalePreset::OneMeter;
        let (mut w, mut p, eye, mut a) = rig(scale);
        let wall = (eye).floor().as_ivec3() + IVec3::new(6, 0, 0);
        w.set(wall, B::Stone);
        let foe = enemy_at(eye + Vec3::new(4.2, 0.0, 1.0), 0);
        a.enemies.push(foe);
        a.loadout.slots = [73, EMPTY, EMPTY, EMPTY];
        a.cast_wand(&mut w, &mut p, eye, Vec3::X);
        w.adventure = a;
        run(&mut w, &mut p, 30);
        let mut water = 0;
        for dx in -4..=2 {
            for dy in -3..=4 {
                for dz in -4..=4 {
                    if w.get(wall + IVec3::new(dx, dy, dz)) == B::Water {
                        water += 1;
                    }
                }
            }
        }
        assert!((6..=18).contains(&water), "{water} water cells poured");
        assert!(
            w.adventure.enemies.iter().any(|e| e.status.wet > 0.0),
            "the foe should be soaked"
        );
    }

    #[test]
    fn venom_nova_poisons_foes_in_range_but_not_distant_ones() {
        let (w, _p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.enemies.push(enemy_at(eye + Vec3::new(2.0, 0.0, 0.0), 0));
        a.enemies.push(enemy_at(eye + Vec3::new(9.0, 0.0, 0.0), 1));
        let health = a.enemies[0].health;
        a.venom_nova(&w, eye, 1.0);
        assert!(a.enemies[0].status.stacks >= 1, "stacks {}", a.enemies[0].status.stacks);
        assert!(a.enemies[0].health < health);
        assert_eq!(a.enemies[1].status.stacks, 0, "a distant foe must be untouched");
    }

    #[test]
    fn ice_wall_raises_a_bounded_barrier_ahead_at_every_scale() {
        for scale in SCALES {
            let (mut w, mut p, eye, mut a) = rig(scale);
            let s = scale.meters();
            a.loadout.slots = [75, EMPTY, EMPTY, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert!(a.mana < 100.0, "{scale:?}: the cast must cost mana");
            let (mut ice, mut worst_z) = (0, 0.0_f32);
            let lo = ((eye - Vec3::new(5.0, 4.0, 6.0)) / s).floor().as_ivec3();
            let hi = ((eye + Vec3::new(5.0, 3.0, 1.0)) / s).ceil().as_ivec3();
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        if w.get(IVec3::new(x, y, z)) == B::Ice {
                            ice += 1;
                            worst_z = worst_z.max(((z as f32 + 0.5) * s - (eye.z - 3.0)).abs());
                        }
                    }
                }
            }
            assert!((9..=400).contains(&ice), "{scale:?}: {ice} ice cells");
            assert!(worst_z <= s * 2.0, "{scale:?}: wall is not a flat plane ({worst_z})");
        }
    }

    #[test]
    fn mana_leech_returns_mana_only_when_it_hits() {
        let measure = |leech: bool| {
            let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
            a.mana = 50.0;
            a.enemies.push(enemy_at(eye + Vec3::new(5.0, 0.0, 0.0), 0));
            let mods = ShotMods {
                mana_leech: leech,
                ..Default::default()
            };
            a.emit_spell_modified(0, eye, Vec3::X, None, mods);
            w.adventure = a;
            run(&mut w, &mut p, 30);
            w.adventure.mana
        };
        let with = measure(true);
        let without = measure(false);
        assert!(
            (with - without - 4.0).abs() < 0.01,
            "leech gave {with}, control {without}"
        );
    }

    #[test]
    fn critical_hits_are_one_in_four_and_the_modifier_is_flagged() {
        let hits = (0..4000u32)
            .filter(|&i| crit_roll(i as f32 * 0.137, i % 7))
            .count();
        let rate = hits as f32 / 4000.0;
        assert!((0.20..=0.30).contains(&rate), "crit rate {rate}");

        let mut b = Adventure::default();
        b.wand = 3;
        b.loadout.slots = [78, 77, 0, EMPTY];
        let m = evaluate_cast_block(3, !0, 0, 0, &b.loadout, 0).actions[0].mods;
        assert!(m.crit && m.mana_leech);
    }

    #[test]
    fn on_expire_delivers_its_payload_on_a_miss_but_a_plain_trigger_does_not() {
        for (card, expect) in [(79u32, true), (18, false)] {
            let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
            a.loadout.slots = [card, 0, 3, EMPTY];
            a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
            assert_eq!(a.projectiles.len(), 1, "card {card}");
            assert_eq!(a.projectiles[0].payload, Some(3), "card {card}");
            a.projectiles[0].life = 0.2;
            w.adventure = a;
            run(&mut w, &mut p, 30);
            let released = w.adventure.projectiles.iter().any(|s| s.spell == 3);
            assert_eq!(released, expect, "card {card}: payload released = {released}");
        }
    }

    #[test]
    fn chaotic_chassis_reorders_the_deck_but_never_changes_its_contents() {
        let mut base = Loadout::default();
        base.slots = [0, 2, EMPTY, 3];
        base.aux_slots = [5, EMPTY, EMPTY, EMPTY];
        let sorted = |d: &Loadout| {
            let mut v: Vec<u32> = (0..MAX_SLOTS).map(|i| d.slot(i)).collect();
            v.sort_unstable();
            v
        };
        let mut seen = std::collections::HashSet::new();
        for seed in 0..60u32 {
            let mut d = base;
            super::super::discovery::shuffle_deck(&mut d, seed, 8);
            assert_eq!(sorted(&d), sorted(&base), "seed {seed} changed which spells exist");
            for i in 0..MAX_SLOTS {
                assert_eq!(
                    d.slot(i) == EMPTY,
                    base.slot(i) == EMPTY,
                    "seed {seed}: empty slot {i} moved"
                );
            }
            seen.insert((0..MAX_SLOTS).map(|i| d.slot(i)).collect::<Vec<_>>());
        }
        assert!(seen.len() >= 6, "only {} distinct orders in 60 seeds", seen.len());

        // Capacity limits the shuffle: slots beyond the wand are left alone.
        let mut d = base;
        super::super::discovery::shuffle_deck(&mut d, 7, 4);
        assert_eq!(d.slot(4), 5, "a slot beyond capacity must not move");
    }

    #[test]
    fn only_the_chaotic_chassis_shuffles_and_reloading_reseeds_it() {
        assert!(stats(3, 3, 0).shuffle);
        for mode in [0, 1, 2] {
            assert!(!stats(3, mode, 0).shuffle, "mode {mode}");
        }

        let (mut w, mut p, eye, mut a) = rig(VoxelScalePreset::OneMeter);
        a.loadout.mode = 3;
        a.loadout.slots = [0, 2, EMPTY, EMPTY];
        let before = a.shuffle_seed;
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_eq!(a.shuffle_seed, before, "no reload yet");
        a.cast_wand(&mut w, &mut p, eye, Vec3::NEG_Z);
        assert_ne!(a.shuffle_seed, before, "the deck wrapped, so it must reshuffle");

        // A fixed-order chassis never touches the order.
        let mut direct = Adventure::default();
        direct.wand = 3;
        direct.loadout.slots = [0, 2, 3, 5];
        direct.shuffle_seed = 12345;
        let deck = direct.usable_loadout();
        assert_eq!([deck.slot(0), deck.slot(1), deck.slot(2), deck.slot(3)], [0, 2, 3, 5]);
    }
}
