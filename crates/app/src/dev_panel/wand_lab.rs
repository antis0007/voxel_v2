//! Wand Lab: runs a loadout through the game's own deck evaluator and reports
//! what one full deck cycle costs and delivers. Read-only; needs no world.
//! Damage is a direct-hit estimate: status, crits, chains and splash are not
//! modelled, so compare builds with it rather than quoting it as combat DPS.
use crate::world::adventure::{spell_defs, wand_items, wands, MAX_SPELL_ID};
use serde_json::{Value, json};

pub const MAX_BLOCKS: usize = 64;

#[derive(serde::Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// Wand tier 1..3; sets capacity and base timings.
    pub tier: u32,
    /// Chassis mode: 0 ordered, 1 twin, 2 trigger, 3 rapid, 4/5 grimoire bindings.
    #[serde(default)]
    pub mode: u32,
    #[serde(default)]
    pub rank: u32,
    /// Found-wand seed; 0 is the plain crafted wand.
    #[serde(default)]
    pub seed: u32,
    /// Spell ids per slot, up to 8; `wands::EMPTY` (8) is an empty slot.
    pub slots: Vec<u32>,
    /// Rare-magic bitmask; omitted means every rare spell is found.
    #[serde(default)]
    pub rare: Option<u32>,
    #[serde(default)]
    pub discoveries: u32,
    #[serde(default = "one")]
    pub health: f32,
    #[serde(default = "yes")]
    pub foe_visible: bool,
}
fn one() -> f32 { 1.0 }
fn yes() -> bool { true }

impl Build {
    fn loadout(&self) -> Result<wands::Loadout, String> {
        if !(1..=3).contains(&self.tier) { return Err("tier: expected 1..3".into()); }
        if self.mode > 5 { return Err("mode: expected 0..5".into()); }
        if self.rank > 20 { return Err("rank: expected 0..20".into()); }
        if !(0.01..=1.0).contains(&self.health) { return Err("health: expected 0.01..1".into()); }
        let cap = wands::capacity(self.tier);
        if self.slots.len() > cap { return Err(format!("A tier {} wand holds {cap} slots", self.tier)); }
        let mut loadout = wands::Loadout { slots: [wands::EMPTY; 4], aux_slots: [wands::EMPTY; 4], cursor: 0, mode: self.mode, seed: self.seed };
        for (i, &spell) in self.slots.iter().enumerate() {
            if spell != wands::EMPTY && spell_defs::def(spell).is_none() { return Err(format!("Slot {i}: unknown spell {spell}")); }
            loadout.set_slot(i, spell);
        }
        Ok(loadout)
    }
    fn to_json(&self) -> Value {
        json!({"tier": self.tier, "mode": self.mode, "rank": self.rank, "seed": self.seed, "slots": self.slots,
            "rare": self.rare, "discoveries": self.discoveries, "health": self.health, "foe_visible": self.foe_visible})
    }
    fn with_slots(&self, slots: Vec<u32>) -> Self { Self { slots, ..self.clone() } }
}

fn listed(d: &spell_defs::SpellDef) -> bool { !matches!(d.palette, spell_defs::Palette::Hidden) }

/// Spell palette with the fields the lab needs, from the loaded table.
pub fn catalog(tier: u32) -> Value {
    let rows: Vec<Value> = (0..=MAX_SPELL_ID).filter_map(spell_defs::def).map(|d| json!({
        "id": d.id, "name": d.name, "tag": d.tag, "class": format!("{:?}", d.class),
        "palette": format!("{:?}", d.palette), "color": d.color, "cost": d.cost, "cooldown": d.cooldown,
        "damage": spell_defs::spell_hit_damage(d.id), "base_damage": d.damage, "speed": d.speed,
        "cast_delay": d.cast_delay, "recharge": d.recharge, "spread": d.spread, "life": wands::base_life(d.id),
        "unlock": format!("{:?}", d.unlock), "hint": spell_defs::spell_hint(d.id),
        "available": spell_defs::spell_available(d.id, tier.max(1), u32::MAX),
    })).collect();
    let chassis: Vec<Value> = (0..=5).map(|mode| {
        let s = wand_items::stats_for(tier.clamp(1, 3), &wands::Loadout { mode, ..Default::default() }, 0);
        json!({"mode": mode, "name": s.chassis, "spells_per_cast": s.spells_per_cast, "shuffle": s.shuffle})
    }).collect();
    let capacity = [1, 2, 3].map(wands::capacity);
    json!({"empty": wands::EMPTY, "max_slots": wands::MAX_SLOTS, "draft": spell_defs::SPELLS_OVERRIDE.get().is_some(),
        "capacity": capacity, "chassis": chassis, "spells": rows,
        "source": "assets/spells.ron; crates/app/src/world/adventure/wands.rs"})
}

fn mods_summary(m: &wands::ShotMods) -> Value {
    let mut flags = Vec::new();
    for (on, name) in [(m.homing,"homing"),(m.orbit,"orbit"),(m.bore,"bore"),(m.trail,"trail"),(m.volatile,"volatile"),
        (m.siphon,"siphon"),(m.chain,"chain"),(m.shrapnel,"shrapnel"),(m.reverse,"reverse"),(m.reckless,"reckless"),
        (m.blood_price,"blood_price"),(m.fork,"fork"),(m.mana_leech,"mana_leech"),(m.crit,"crit"),(m.explosive,"explosive"),
        (m.pull,"pull"),(m.unstable,"unstable"),(m.blood,"blood"),(m.seek_ally,"seek_ally"),(m.crit_wet,"crit_wet")] {
        if on { flags.push(name); }
    }
    json!({"damage_mul": m.damage_mul, "speed_mul": m.speed_mul, "life_mul": m.life_mul, "bounces": m.bounces,
        "pierce": m.pierce, "phase": m.phase, "radius": m.radius, "element": m.element.map(|e| format!("{e:?}")), "flags": flags})
}

fn flight(spell: u32, m: &wands::ShotMods) -> Value {
    json!({"speed": spell_defs::spell_base_speed(spell) * m.speed_mul, "life": wands::base_life(spell) * m.life_mul,
        "path": m.path, "gravity": m.gravity, "bounces": m.bounces, "homing": m.homing, "orbit": m.orbit,
        "reverse": m.reverse, "timer": m.timer, "pierce": m.pierce, "radius": m.radius})
}

fn hit(spell: u32, mods: &wands::ShotMods) -> f32 {
    if wands::projectile(spell) { spell_defs::spell_hit_damage(spell) * mods.damage_mul } else { 0.0 }
}

/// Sustained numbers for one deck cycle; the optimizer and survey use this
/// without building JSON.
#[derive(Clone, Copy, Debug, Default)]
pub struct Measure {
    pub cycle_s: f32,
    /// Mana actually paid; Blood price blocks pay health instead.
    pub mana: f32,
    /// Health paid by Blood price blocks per cycle.
    pub health: f32,
    pub damage: f32,
    pub casts: usize,
    pub bolts: usize,
    pub wrapped: bool,
    pub mana_max: f32,
    pub mana_charge: f32,
}
impl Measure {
    pub fn dps(&self) -> f32 { self.damage / self.cycle_s.max(1e-3) }
    pub fn mana_rate(&self) -> f32 { self.mana / self.cycle_s.max(1e-3) }
    pub fn drain(&self) -> f32 { self.mana_rate() - self.mana_charge }
    pub fn dpm(&self) -> f32 { if self.mana > 0.0 { self.damage / self.mana } else { 0.0 } }
    pub fn health_rate(&self) -> f32 { self.health / self.cycle_s.max(1e-3) }
    /// Average damage per second over `window` seconds of held fire: full
    /// rate until the pool is dry, then the rate mana charge can pay for.
    pub fn window_dps(&self, window: f32) -> f32 {
        let drain = self.drain();
        if drain <= 0.0 { return self.dps(); }
        let full = (self.mana_max / drain).min(window);
        let starved = self.mana_charge / self.mana_rate().max(1e-3);
        (self.dps() * full + self.dps() * starved * (window - full)) / window
    }
    fn summary(&self) -> Value {
        json!({"dps": self.dps(), "dps_30s": self.window_dps(30.0), "mana_per_s": self.mana_rate(), "damage_per_mana": self.dpm(),
            "cycle_s": self.cycle_s, "sustainable": self.drain() <= 0.0, "damage_per_cycle": self.damage,
            "health_per_s": self.health_rate(), "wrapped": self.wrapped})
    }
}

fn run(build: &Build, loadout: &wands::Loadout, mut on_block: impl FnMut(&wands::CastBlockEval, f32, f32)) -> Measure {
    let rare = build.rare.unwrap_or(u32::MAX);
    let stats = wand_items::stats_for(build.tier, loadout, build.rank);
    let cap = wands::capacity(build.tier);
    let usable = |s: u32| wands::fits_instrument(s, loadout.mode) && spell_defs::spell_available(s, build.tier, rare);
    let mut m = Measure { mana_max: stats.mana_max, mana_charge: stats.mana_charge_speed, ..Default::default() };
    let mut cursor = 0usize;
    for index in 0..MAX_BLOCKS {
        let eval = wands::evaluate_programmed(build.tier, rare, build.discoveries, build.rank, loadout, cursor,
            build.health, build.foe_visible, index as u32);
        if eval.actions.is_empty() { break; }
        let damage: f32 = eval.actions.iter()
            .map(|a| hit(a.spell, &a.mods) + a.payload.map_or(0.0, |p| hit(p, &a.payload_mods))).sum();
        on_block(&eval, damage, m.cycle_s);
        m.bolts += eval.actions.len();
        m.casts += 1;
        m.cycle_s += eval.cast_delay + eval.recharge_time;
        // Mirrors Adventure's cast: Blood price pays a fraction in health, no mana.
        if eval.actions.iter().any(|a| a.mods.blood_price) {
            m.health += eval.mana_cost * wands::card_rules::BLOOD_PRICE_FRACTION;
        } else {
            m.mana += eval.mana_cost;
        }
        if eval.actions.iter().any(|a| a.mods.reckless) { m.health += wands::card_rules::RECKLESS_HEALTH; }
        m.damage += damage;
        if eval.recharge_time > 0.0 { m.wrapped = true; break; }
        // Mirror Adventure::advance_wand: the next wieldable slot after the block.
        cursor = (1..=cap).map(|n| (eval.last_slot + n) % cap).find(|&i| usable(loadout.slot(i))).unwrap_or(eval.last_slot);
    }
    m
}

pub fn measure(build: &Build) -> Result<Measure, String> {
    let loadout = build.loadout()?;
    Ok(run(build, &loadout, |_, _, _| {}))
}

/// Evaluates cast blocks from slot 0 until the deck wraps (recharge) or
/// `MAX_BLOCKS` is reached, then derives sustained cost and output.
pub fn simulate(build: &Build) -> Result<Value, String> {
    let loadout = build.loadout()?;
    let rare = build.rare.unwrap_or(u32::MAX);
    let stats = wand_items::stats_for(build.tier, &loadout, build.rank);
    let usable = |s: u32| wands::fits_instrument(s, loadout.mode) && spell_defs::spell_available(s, build.tier, rare);
    let slots: Vec<Value> = (0..wands::capacity(build.tier)).map(|i| {
        let s = loadout.slot(i);
        json!({"slot": i, "spell": s, "name": if s == wands::EMPTY { "Empty" } else { spell_defs::spell_name(s) }, "usable": usable(s)})
    }).collect();
    let mut blocks = Vec::new();
    let m = run(build, &loadout, |eval, damage, start| {
        let blood = eval.actions.iter().any(|a| a.mods.blood_price);
        let actions: Vec<Value> = eval.actions.iter().map(|a| {
            let d = hit(a.spell, &a.mods) + a.payload.map_or(0.0, |p| hit(p, &a.payload_mods));
            json!({"slot": a.slot, "spell": a.spell, "name": spell_defs::spell_name(a.spell),
                "payload": a.payload.map(|p| json!({"spell": p, "name": spell_defs::spell_name(p), "slot": a.payload_slot, "mods": mods_summary(&a.payload_mods)})),
                "follow": a.follow.map(|f| json!({"spell": f, "name": spell_defs::spell_name(f)})),
                "mods": mods_summary(&a.mods), "flight": flight(a.spell, &a.mods), "hit_damage": d})
        }).collect();
        blocks.push(json!({"start_s": start, "first_slot": eval.actions[0].slot, "last_slot": eval.last_slot,
            "modifier_slots": eval.modifier_slots,
            "mana": if blood { 0.0 } else { eval.mana_cost },
            "health": if blood { eval.mana_cost * wands::card_rules::BLOOD_PRICE_FRACTION } else { 0.0 }
                + if eval.actions.iter().any(|a| a.mods.reckless) { wands::card_rules::RECKLESS_HEALTH } else { 0.0 },
            "cast_delay": eval.cast_delay,
            "recharge": eval.recharge_time, "spread_deg": eval.spread_deg, "damage": damage, "actions": actions}));
    });
    let drain = m.drain();
    Ok(json!({
        "chassis": {"name": stats.name, "chassis": stats.chassis, "shuffle": stats.shuffle, "spells_per_cast": stats.spells_per_cast,
            "cast_delay": stats.cast_delay, "recharge": stats.recharge_time, "mana_max": stats.mana_max,
            "mana_charge": stats.mana_charge_speed, "capacity": stats.capacity, "spread_deg": stats.spread_deg, "always_cast": stats.always_cast},
        "slots": slots, "blocks": blocks, "draft": spell_defs::SPELLS_OVERRIDE.get().is_some(),
        "summary": {"cycle_s": m.cycle_s, "wrapped": m.wrapped, "casts": m.casts, "bolts": m.bolts, "mana_per_cycle": m.mana,
            "mana_per_s": m.mana_rate(), "damage_per_cycle": m.damage, "dps": m.dps(), "damage_per_mana": m.dpm(),
            "sustainable": drain <= 0.0, "seconds_to_empty": (drain > 0.0).then(|| m.mana_max / drain),
            "dps_30s": m.window_dps(30.0), "health_per_cycle": m.health, "health_per_s": m.health_rate(),
            "loops": !m.wrapped && m.casts >= MAX_BLOCKS},
        "scope": "Direct-hit damage estimate from the native evaluator. Blood price and Reckless are charged in health as in the game; status, crits, chains, splash and shuffle order are not modelled.",
    }))
}

// ---------- optimizer ----------

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Search {
    pub build: Build,
    /// "dps", "sustained" (30 s held-fire average), "efficiency" (damage per mana) or "burst".
    #[serde(default = "sustained")]
    pub objective: String,
    /// Reject decks that drain mana faster than it charges.
    #[serde(default)]
    pub must_sustain: bool,
    /// Allow decks that pay with health (Blood price, Reckless). Off by default so the
    /// search does not mistake health-paid casts for free ones.
    #[serde(default)]
    pub allow_health_costs: bool,
    /// Allow decks whose cast blocks wrap mid-draw and so never pay recharge
    /// in the shipping evaluator. Off by default: it reads as an exploit.
    #[serde(default)]
    pub allow_no_recharge: bool,
    /// Slot indices the search keeps as they are.
    #[serde(default)]
    pub locked: Vec<usize>,
    /// Allowed spell ids; omitted means every castable listed spell.
    #[serde(default)]
    pub pool: Option<Vec<u32>>,
    #[serde(default = "iterations")]
    pub iterations: u32,
    #[serde(default)]
    pub seed: u64,
}
fn sustained() -> String { "sustained".into() }
fn iterations() -> u32 { 4000 }

fn mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
fn pick<T: Copy>(rng: &mut u64, items: &[T]) -> T { items[(mix(rng) % items.len() as u64) as usize] }

fn score(m: &Measure, objective: &str, must_sustain: bool, allow_health: bool, allow_no_recharge: bool) -> f32 {
    let base = match objective {
        "dps" => m.dps(),
        "efficiency" => m.dpm() * 10.0 + m.dps() * 0.01,
        // Damage the full pool can deliver, per second of the first cycle.
        "burst" => m.damage.min(m.mana_max * m.dpm()) / m.cycle_s.clamp(0.05, 3.0),
        _ => m.window_dps(30.0),
    };
    let base = if !allow_health && m.health > 0.0 { base * 0.05 - m.health_rate() } else { base };
    let base = if !allow_no_recharge && !m.wrapped && m.casts > 0 { base * 0.05 } else { base };
    if must_sustain && m.drain() > 0.0 { base * 0.1 - m.drain() } else { base }
}

/// Deterministic hill climbing with restarts over slot contents and order.
/// The same request and seed give the same answer.
pub fn optimize(search: &Search) -> Result<Value, String> {
    if !["dps", "sustained", "efficiency", "burst"].contains(&search.objective.as_str()) {
        return Err("objective: dps, sustained, efficiency or burst".into());
    }
    if !(100..=50_000).contains(&search.iterations) { return Err("iterations: expected 100..50000".into()); }
    let cap = wands::capacity(search.build.tier);
    if search.locked.iter().any(|&i| i >= cap) { return Err("locked: slot outside this wand".into()); }
    let rare = search.build.rare.unwrap_or(u32::MAX);
    let mut pool: Vec<u32> = match &search.pool {
        Some(ids) => ids.clone(),
        None => (0..=MAX_SPELL_ID).filter(|&id| spell_defs::def(id).is_some_and(listed)).collect(),
    };
    pool.retain(|&s| s != wands::EMPTY && wands::fits_instrument(s, search.build.mode) && spell_defs::spell_available(s, search.build.tier, rare));
    if pool.is_empty() { return Err("No castable spells in the pool".into()); }
    pool.push(wands::EMPTY);
    let mut start = search.build.slots.clone();
    start.resize(cap, wands::EMPTY);
    let free: Vec<usize> = (0..cap).filter(|i| !search.locked.contains(i)).collect();
    if free.is_empty() { return Err("Every slot is locked".into()); }
    let try_deck = |slots: &[u32]| -> Result<(f32, Measure), String> {
        let m = measure(&search.build.with_slots(slots.to_vec()))?;
        Ok((score(&m, &search.objective, search.must_sustain, search.allow_health_costs, search.allow_no_recharge), m))
    };
    let mut rng = search.seed ^ 0xA5A5_5A5A;
    let (mut best_slots, (mut best, mut best_m)) = (start.clone(), try_deck(&start)?);
    let initial = best_m;
    let restarts = 6u32;
    let per = search.iterations / restarts;
    let mut evaluated = 1u32;
    for restart in 0..restarts {
        let mut cur = start.clone();
        if restart > 0 { for &i in &free { cur[i] = pick(&mut rng, &pool); } }
        let (mut cur_score, _) = try_deck(&cur)?;
        evaluated += 1;
        for _ in 0..per {
            let mut next = cur.clone();
            let i = pick(&mut rng, &free);
            if mix(&mut rng) % 3 == 0 && free.len() > 1 {
                next.swap(i, pick(&mut rng, &free));
            } else {
                next[i] = pick(&mut rng, &pool);
            }
            let (sc, m) = try_deck(&next)?;
            evaluated += 1;
            if sc >= cur_score {
                cur = next;
                cur_score = sc;
                if sc > best { best = sc; best_slots = cur.clone(); best_m = m; }
            }
        }
    }
    Ok(json!({"build": search.build.with_slots(best_slots).to_json(), "score": best, "objective": search.objective,
        "evaluated": evaluated, "before": initial.summary(), "after": best_m.summary(),
        "scope": "Hill climbing with restarts on the native evaluator; a strong local optimum, not a proof of the best deck."}))
}

// ---------- balance survey ----------

/// Scores every castable projectile alone and every modifier in front of a
/// reference bolt, so outliers stand out in one table.
pub fn survey(build: &Build, reference: u32) -> Result<Value, String> {
    let rare = build.rare.unwrap_or(u32::MAX);
    let base = measure(&build.with_slots(vec![reference]))?;
    let mut rows = Vec::new();
    for id in 0..=MAX_SPELL_ID {
        let Some(d) = spell_defs::def(id) else { continue };
        if !listed(d) || !spell_defs::spell_available(id, build.tier, rare) { continue; }
        let m = match d.class {
            spell_defs::Class::Projectile => measure(&build.with_slots(vec![id]))?,
            spell_defs::Class::Modifier => measure(&build.with_slots(vec![id, reference]))?,
            _ => continue,
        };
        let mut row = m.summary();
        row["id"] = json!(id);
        row["name"] = json!(d.name);
        row["class"] = json!(format!("{:?}", d.class));
        row["cost"] = json!(d.cost);
        row["color"] = json!(d.color);
        row["vs_reference"] = json!(if base.dps() > 0.0 { m.dps() / base.dps() } else { 0.0 });
        rows.push(row);
    }
    // Blocks that wrap mid-draw skip recharge in the shipping evaluator; flag
    // every modifier that produces such a deck in front of the reference.
    for row in rows.iter_mut() {
        if row["wrapped"] == json!(false) { row["never_recharges"] = json!(true); }
    }
    Ok(json!({"reference": {"id": reference, "name": spell_defs::spell_name(reference), "dps": base.dps(), "damage_per_mana": base.dpm()},
        "rows": rows, "scope": "Projectiles alone; modifiers placed before the reference bolt. Direct-hit estimates."}))
}

/// Mints the build as a wand into an empty hotbar slot for a live test.
pub fn equip(world: &mut crate::world::World, build: &Build) -> Result<Value, String> {
    use crate::world::{adventure::wand_items::WandRecord, inventory::ItemStack};
    let loadout = build.loadout()?;
    let slot = world.inventory.hotbar.iter().position(Option::is_none).ok_or("Clear one hotbar slot first")?;
    let item = world.adventure.mint_wand(WandRecord { tier: build.tier, loadout }).ok_or("Wand bank is full")?;
    world.inventory.hotbar[slot] = Some(ItemStack::new(item, 1));
    Ok(json!({"applied": "live", "scope": "session world", "hotbar_slot": slot + 1,
        "note": "Select that hotbar slot to cast the build"}))
}

#[derive(serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Line {
    Spells { #[serde(default)] tier: Option<u32> },
    Wand { build: Build },
    Optimize(Search),
    Survey { build: Build, #[serde(default)] reference: u32 },
}

/// `--wand-lab`: one JSON request per stdin line, one JSON reply per stdout
/// line. No window, world or save is touched, so the dev panel can keep one
/// process open and re-evaluate on every edit. `--spells=<path>` evaluates a
/// draft spell table instead of the compiled one, without a rebuild.
pub fn run_headless() -> anyhow::Result<()> {
    use std::io::{BufRead, Write};
    if let Some(path) = std::env::args().find_map(|a| a.strip_prefix("--spells=").map(String::from)) {
        let text = std::fs::read_to_string(&path)?;
        if let Err(errors) = engine_content::spells::parse::<crate::audio::Cue>(&text) {
            println!("{}", json!({"ok": false, "error": format!("Draft spell table: {errors:?}")}));
            return Ok(());
        }
        // Must happen before anything reads the table.
        if spell_defs::SPELLS_OVERRIDE.set(Box::leak(text.into_boxed_str())).is_err() {
            anyhow::bail!("spell table already loaded");
        }
    }
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let reply = match serde_json::from_str::<Line>(&line).map_err(|e| e.to_string()).and_then(|req| match req {
            Line::Spells { tier } => Ok(catalog(tier.unwrap_or(3))),
            Line::Wand { build } => simulate(&build),
            Line::Optimize(search) => optimize(&search),
            Line::Survey { build, reference } => survey(&build, reference),
        }) {
            Ok(data) => json!({"ok": true, "data": data}),
            Err(error) => json!({"ok": false, "error": error}),
        };
        writeln!(out, "{reply}")?;
        out.flush()?;
    }
    Ok(())
}
