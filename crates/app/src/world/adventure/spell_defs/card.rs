//! Compact, read-only spell cards built from the same rules as casting.
use super::*;
use super::super::{status::{Affixes, Element, Status}, wands::card_rules as rules};

pub fn description(spell: u32) -> Option<&'static engine_content::descriptions::Description> {
    engine_content::descriptions::get(&format!("spell_{spell}"))
}

pub struct Card {
    pub stats: String,
    pub cost: String,
    pub details: Vec<(&'static str, String)>,
}

pub fn amount(value: f32) -> String {
    if value.fract().abs() < 0.001 { format!("{value:.0}") } else { format!("{value:.1}") }
}

/// A neutral, fully unlocked evaluation. This describes the card in isolation;
/// the wand's next-cast readout remains the authority for the equipped deck.
fn preview(spell: u32) -> wands::CastBlockEval {
    let loadout = wands::Loadout {
        slots: [spell, 0, 2, 3],
        aux_slots: [5, wands::EMPTY, wands::EMPTY, wands::EMPTY],
        ..Default::default()
    };
    wands::evaluate_programmed(3, u32::MAX, 0, 0, &loadout, 0, 0.0, true, 0)
}

pub fn read(spell: u32, discoveries: u32, rank: u32) -> Card {
    let cost = inscriptions::cost(spell, discoveries);
    let cost_text = amount(cost);
    let mut card = Card {
        stats: String::new(),
        cost: format!("{cost_text} mana"),
        details: Vec::new(),
    };
    if spell == wands::EMPTY {
        card.cost.clear();
        return card;
    }
    if spell == 22 {
        card.cost = format!("{cost_text} mana · up to {:.0} mana discount", rules::RAPID_MANA_REFUND);
    }
    if spell_class(spell) != Class::Modifier {
        if spell_class(spell) == Class::Projectile {
            card.stats = format!("{:.0} base damage · {:.0} m/s", spell_hit_damage(spell), spell_base_speed(spell));
            card.details.push(("Base flight time", format!("{:.2} s", wands::base_life(spell))));
            let mut innate = wands::ShotMods::default();
            wands::innate_mods(spell, &mut innate);
            if innate.bounces > 0 {
                card.details.push(("Wall ricochets", innate.bounces.to_string()));
            }
        }
        match spell {
            1 => card.stats = format!("Heals {:.0}", rules::mend_health(rank)),
            3 => card.details.push(("Crater radius", format!("{:.1} m", rules::RESONANT_CRATER))),
            4 => card.stats = format!("{} reflections · {:.0} s", rules::ward_hits(discoveries), rules::ward_duration(discoveries)),
            9 => card.stats = format!("{:.1} m beam reach", rules::DRILL_REACH),
            11 => card.details.push(("Plasma blades", super::super::spells::PLASMA_DIRECTIONS.len().to_string())),
            30 => card.stats = format!("{:.1} s enemy slow", rules::TIME_SNAG_DURATION),
            31 => card.stats = format!("{:.0} m reach", rules::GRAPPLE_REACH),
            74 => card.details.push(("Poison radius", format!("{:.1} m", rules::VENOM_NOVA_RADIUS))),
            75 => card.stats = format!("{:.1} × {:.1} m barrier", rules::ICE_WALL_WIDTH, rules::ICE_WALL_HEIGHT),
            84 => card.details.push(("Maximum flight time", format!("{:.1} s", super::super::arcana::LODESTONE_MAX_LIFE))),
            86 => card.details.push(("Terrain budget", format!("{} blocks", rules::DIG_BOLT_BUDGET))),
            87 => card.details.push(("Pellets", super::super::arcana::SCATTER_PELLETS.to_string())),
            _ => {}
        }
        card.details.push(("Base cooldown", format!("{:.2} s", spell_cooldown(spell))));
        return card;
    }

    let eval = preview(spell);
    let mods = eval.mods;
    let mut stats = Vec::new();
    if (mods.damage_mul - 1.0).abs() > 0.005 {
        let value = format!("{:+.0}%", (mods.damage_mul - 1.0) * 100.0);
        stats.push(format!("{value} damage"));
        card.details.push(("Damage", value));
        card.details.push(("Damage multiplier cap", format!("×{:.1}", rules::MAX_DAMAGE_MULTIPLIER)));
    }
    if (mods.speed_mul - 1.0).abs() > 0.005 {
        let value = format!("{:+.0}%", (mods.speed_mul - 1.0) * 100.0);
        stats.push(format!("{value} speed"));
        card.details.push(("Speed", value));
    }
    if mods.radius != 1.0 {
        stats.push(format!("×{:.1} collision radius", mods.radius));
    }
    if mods.life_mul != 1.0 {
        stats.push(format!("×{:.1} lifetime", mods.life_mul));
    }
    if mods.bounces > 0 {
        let value = format!("+{}", mods.bounces);
        if stats.is_empty() { stats.push(format!("{value} wall ricochets")); }
        card.details.push(("Wall ricochets", value));
    }
    if mods.phase > 0 { stats.push(format!("{} wall passes", mods.phase)); }
    if mods.pierce > 0 { stats.push(format!("{} enemy passes", mods.pierce)); }
    if mods.gravity != 0.0 {
        stats.push(format!("{:.0} m/s² downward", mods.gravity * rules::GRAVITY_ACCELERATION));
    }
    match spell {
        16 | 17 | 91 => stats.push(format!("{} spells per draw", eval.actions.len())),
        40 | 41 => stats.push(format!("{} copies", eval.actions.len())),
        27 => card.details.push(("Health per direct hit", format!("{:.0}", rules::SIPHON_HEALTH))),
        45 => stats.push(format!("{:.0}% base hit healing", rules::BLOOD_SIPHON_FRACTION * 100.0)),
        46 => stats.push(format!("Fizzle roll 1 / {}", rules::UNSTABLE_PERIOD)),
        47 => stats.push(format!("Health below {:.0}%", rules::WOUNDED_THRESHOLD * 100.0)),
        62 => {
            stats.push(format!("{:.0} damage · {:.1} m radius", rules::SHRAPNEL_DAMAGE, rules::SHRAPNEL_RADIUS));
        }
        67 => card.cost.push_str(&format!(" · {:.0} health", rules::RECKLESS_HEALTH)),
        68 => {
            card.cost = format!("{:.0}% of cast mana paid as health", rules::BLOOD_PRICE_FRACTION * 100.0);
            card.details.push(("Card mana contribution", cost_text));
            card.details.push(("Health reserve", format!("> {:.0} after payment", rules::BLOOD_PRICE_RESERVE)));
        }
        69 => {
            stats.push(format!("{} sparks · {:.0}% damage", rules::FORK_ANGLES.len(), rules::FORK_DAMAGE * 100.0));
        }
        70 => stats.push(format!("{:.1} s timer", mods.timer)),
        77 => stats.push(format!("{:.0} mana per direct hit", rules::MANA_LEECH)),
        78 => stats.push(format!("1 / {} roll · ×{:.1} damage", rules::CRITICAL_PERIOD, rules::CRITICAL_MULTIPLIER)),
        88 => stats.push(format!("×{:.0} conditional damage", rules::SOAKED_MULTIPLIER)),
        89 => card.details.push(("Crater radius", format!("{:.1} m", rules::EXPLOSIVE_CRATER))),
        93 => stats.push(format!("{:.0} m proximity", rules::PROXIMITY_RANGE)),
        94 => stats.push(format!("{:.1} m pull radius", rules::UNDERTOW_RADIUS)),
        _ => {}
    }
    if let Some(element) = mods.element {
        let mut status = Status::default();
        let reaction = status.apply(element, Affixes::default());
        let mut wet = Status::default();
        wet.apply(Element::Wet, Affixes::default());
        let wet_reaction = wet.apply(element, Affixes::default());
        match element {
            Element::Fire => stats.push(format!("{:.0} s Burning", status.burn)),
            Element::Frost => {
                stats.push(format!("{:.1} s chill · {:.1} s if Wet", reaction.chill, wet_reaction.chill));
            }
            Element::Shock => {
                stats.push(format!("{:.2} s stagger", reaction.stagger));
                card.details.push(("Damage against Wet", format!("{:+.0}%", (wet_reaction.damage_mul - 1.0) * 100.0)));
                card.details.push(("Stagger against Wet", format!("{:.2} s", wet_reaction.stagger)));
            }
            Element::Toxic => {
                stats.push(format!("{:.0} s poison · {} stacks max", status.poison, super::super::status::MAX_POISON_STACKS));
            }
            Element::Wet => stats.push(format!("{:.0} s Wet", status.wet)),
        }
    }
    card.stats = stats.join(" · ");
    card
}
