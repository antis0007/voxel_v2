//! Rune phrases on card wands. The grammar and its table live in `engine_content::runes`;
//! this side loads `assets/runes.ron` and inscribes a compiled phrase onto the held wand
//! as deck cards, so the existing evaluator casts it.
use super::*;
use engine_content::runes::{Program, Table};

/// The rune table, loaded once from `assets/runes.ron`.
pub static TABLE: std::sync::LazyLock<Table> = std::sync::LazyLock::new(|| {
    Table::parse(include_str!("../../../../../assets/runes.ron"))
        .unwrap_or_else(|errors| panic!("assets/runes.ron: {errors:?}"))
});

/// Marks an unused byte in a packed deck.
const NO_CARD: u64 = 0xff;
/// Cards one packed deck can carry: a wand's main and aux rows.
pub const MAX_CARDS: usize = 8;

/// Deck cards as one word, first card in the low byte, for a `Copy` action.
pub fn pack(cards: &[u32]) -> Option<u64> {
    if cards.is_empty() || cards.len() > MAX_CARDS || cards.iter().any(|&card| card as u64 >= NO_CARD) {
        return None;
    }
    let mut word = u64::MAX;
    for (index, &card) in cards.iter().enumerate() {
        let shift = index * 8;
        word = (word & !(NO_CARD << shift)) | ((card as u64) << shift);
    }
    Some(word)
}

pub fn unpack(word: u64) -> Vec<u32> {
    (0..MAX_CARDS)
        .map(|index| (word >> (index * 8)) & NO_CARD)
        .take_while(|&card| card != NO_CARD)
        .map(|card| card as u32)
        .collect()
}

/// Why a compiled phrase cannot go on this wand, or the deck it becomes.
pub fn deck_for(program: &Program, view: &View) -> Result<Vec<u32>, String> {
    let Some(cards) = TABLE.lower(program) else {
        return Err(if program.circle { "Needs Circle".into() } else { "No Wand Form".into() });
    };
    let capacity = wands::capacity(view.wand);
    if view.wand == 0 {
        return Err("No Wand".into());
    }
    if cards.len() > capacity {
        return Err(format!("Wand Slots: {}/{}", cards.len(), capacity));
    }
    if let Some(&card) = cards.iter().find(|&&card| !view.can_wield(card)) {
        return Err(format!("Not Found: {}", spell_name(card)));
    }
    Ok(cards)
}

impl Adventure {
    /// Replace the held wand's deck with a packed rune deck. Every card must be one this
    /// caster can already wield; nothing is written otherwise.
    pub fn inscribe_wand(&mut self, word: u64) -> Result<(), &'static str> {
        let cards = unpack(word);
        let capacity = wands::capacity(self.wand);
        if self.wand == 0 {
            return Err("No Wand");
        }
        if cards.is_empty() || cards.len() > capacity {
            return Err("Wand Slots Full");
        }
        if cards.iter().any(|&card| card > MAX_SPELL_ID || !self.can_wield(card)) {
            return Err("Spell Not Found");
        }
        for slot in 0..capacity {
            self.loadout.set_slot(slot, cards.get(slot).copied().unwrap_or(wands::EMPTY));
        }
        // The deck now speaks for the wand; drop any bound native phrase.
        self.rune_cast.unbind();
        self.loadout.cursor = 0;
        self.prepare_wand();
        Ok(())
    }
}

/// A circle phrase as one word, for a `Copy` action: form, effect, material, two form
/// qualifiers and three effect qualifiers, one rune id per byte. Phrases with events or
/// more qualifiers do not fit.
pub fn pack_phrase(program: &Program) -> Option<u64> {
    let root = &program.root;
    let effect = root.effect.as_ref();
    let effect_qualifiers = effect.map_or(&[][..], |effect| &effect.qualifiers);
    if !root.events.is_empty() || root.qualifiers.len() > 2 || effect_qualifiers.len() > 3 {
        return None;
    }
    let mut bytes = [NO_CARD as u8; 8];
    bytes[0] = root.form as u8;
    if let Some(effect) = effect {
        bytes[1] = effect.rune as u8;
        bytes[2] = effect.material.map_or(NO_CARD as u8, |rune| rune as u8);
    }
    for (slot, &rune) in bytes[3..5].iter_mut().zip(&root.qualifiers) {
        *slot = rune as u8;
    }
    for (slot, &rune) in bytes[5..8].iter_mut().zip(effect_qualifiers) {
        *slot = rune as u8;
    }
    Some(u64::from_le_bytes(bytes))
}

/// The phrase text a packed circle word stands for. The caller compiles it again, so a
/// forged word can only name phrases the grammar accepts.
pub fn unpack_phrase(word: u64) -> Option<String> {
    let bytes = word.to_le_bytes();
    let key = |byte: u8| TABLE.runes.get(byte as usize).map(|rune| rune.key.as_str());
    let list = |bytes: &[u8]| -> Option<String> {
        let keys = bytes
            .iter()
            .filter(|&&byte| byte != NO_CARD as u8)
            .map(|&byte| key(byte))
            .collect::<Option<Vec<_>>>()?;
        Some(if keys.is_empty() { String::new() } else { format!("[{}]", keys.join(",")) })
    };
    let mut text = format!("{}{}", key(bytes[0])?, list(&bytes[3..5])?);
    if bytes[1] != NO_CARD as u8 {
        text += &format!(" then {}", key(bytes[1])?);
        if bytes[2] != NO_CARD as u8 {
            text += &format!("({})", key(bytes[2])?);
        }
        text += &list(&bytes[5..8])?;
    }
    Some(text)
}

/// Closes the innermost event child in a packed program.
const CLOSE: u8 = 0xfe;

/// Rune slots a wand of this tier holds for native phrases. Tier 0 is no wand. Tiers 1 to
/// 3 read the table's first three capacities; the rest are reserved for later wands.
pub fn rune_capacity(wand: u32) -> u32 {
    if wand == 0 { 0 } else { TABLE.capacity[(wand as usize - 1).min(TABLE.capacity.len() - 1)] }
}

/// Why this wand cannot hold the program: no wand, too few slots, or a rune above its tier.
pub fn fits_wand(program: &Program, wand: u32) -> Result<(), String> {
    if wand == 0 {
        return Err("No Wand".into());
    }
    let capacity = rune_capacity(wand);
    if program.slots > capacity {
        return Err(format!("Rune Slots: {}/{capacity}", program.slots));
    }
    match program.tier {
        Some(tier) if tier as u32 <= wand => Ok(()),
        Some(tier) => Err(format!("Requires: Tier {tier} Wand")),
        None => Err(format!("Rune Slots: {}/{capacity}", program.slots)),
    }
}

/// Any phrase as one word for a `Copy` action: rune ids in written order, one per byte,
/// each event's child closed by `CLOSE`. Eight bytes cover every tier-1 to tier-3 phrase
/// whose runes plus closing marks fit. Roles come from the table, so no other marks are
/// needed.
pub fn pack_program(program: &Program) -> Option<u64> {
    fn walk(phrase: &engine_content::runes::Phrase, out: &mut Vec<u8>) {
        out.push(phrase.form as u8);
        out.extend(phrase.qualifiers.iter().map(|&id| id as u8));
        if let Some(effect) = &phrase.effect {
            out.push(effect.rune as u8);
            out.extend(effect.material.map(|id| id as u8));
            out.extend(effect.qualifiers.iter().map(|&id| id as u8));
        }
        for (event, child) in &phrase.events {
            out.push(*event as u8);
            walk(child, out);
            out.push(CLOSE);
        }
    }
    if TABLE.runes.len() >= CLOSE as usize {
        return None;
    }
    let mut bytes = Vec::new();
    walk(&program.root, &mut bytes);
    // Closing marks at the very end are implied.
    while bytes.last() == Some(&CLOSE) {
        bytes.pop();
    }
    if bytes.len() > 8 {
        return None;
    }
    bytes.resize(8, NO_CARD as u8);
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes);
    Some(u64::from_le_bytes(word))
}

/// The phrase text a word from `pack_program` stands for. The caller compiles it again,
/// so a forged word can only name phrases the grammar accepts.
pub fn unpack_program(word: u64) -> Option<String> {
    use engine_content::runes::Role;
    let bytes: Vec<u8> = word.to_le_bytes().into_iter().take_while(|&b| b != NO_CARD as u8).collect();
    fn rune(byte: u8) -> Option<&'static engine_content::runes::Rune> {
        TABLE.runes.get(byte as usize)
    }
    fn role_at(bytes: &[u8], at: usize) -> Option<Role> {
        bytes.get(at).filter(|&&b| b != CLOSE).and_then(|&b| rune(b)).map(|r| r.role)
    }
    fn qualifiers(bytes: &[u8], at: &mut usize, out: &mut String) {
        let mut first = true;
        while role_at(bytes, *at) == Some(Role::Qualifier) {
            out.push(if first { '[' } else { ',' });
            out.push_str(&TABLE.runes[bytes[*at] as usize].key);
            first = false;
            *at += 1;
        }
        if !first {
            out.push(']');
        }
    }
    fn phrase(bytes: &[u8], at: &mut usize, out: &mut String) -> Option<()> {
        if role_at(bytes, *at)? != Role::Form {
            return None;
        }
        out.push_str(&rune(bytes[*at])?.key);
        *at += 1;
        qualifiers(bytes, at, out);
        if role_at(bytes, *at) == Some(Role::Effect) {
            out.push_str(" then ");
            out.push_str(&rune(bytes[*at])?.key);
            *at += 1;
            if role_at(bytes, *at) == Some(Role::Material) {
                out.push('(');
                out.push_str(&rune(bytes[*at])?.key);
                out.push(')');
                *at += 1;
            }
            qualifiers(bytes, at, out);
        }
        while role_at(bytes, *at) == Some(Role::Event) {
            out.push_str(" then ");
            out.push_str(&rune(bytes[*at])?.key);
            out.push_str(" { ");
            *at += 1;
            phrase(bytes, at, out)?;
            out.push_str(" }");
            match bytes.get(*at) {
                Some(&CLOSE) => *at += 1,
                None => {}
                Some(_) => return None,
            }
        }
        Some(())
    }
    let mut at = 0;
    let mut text = String::new();
    phrase(&bytes, &mut at, &mut text)?;
    // A stray close mark or trailing rune means the word was not packed by us.
    (at == bytes.len()).then_some(text)
}
