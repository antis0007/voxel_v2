//! Rune phrases: the one spell language behind presets, wands and circles.
//!
//! A phrase is `FORM[QUALIFIER] then EFFECT(MATERIAL)[QUALIFIER] then EVENT { child }`.
//! `Table::compile` turns the text into a typed tree with its slot count, mana, upkeep and
//! wand tier; editors, tooltips, cost previews and execution all read that compiled form.
//! The rune rows live in `assets/runes.ron`. Nothing here touches the world.

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    /// Where the magic goes: Touch, Bolt, Ring.
    Form,
    /// What it does there: Fire, Push, Place.
    Effect,
    /// What a building effect uses: Stone, Glass.
    Material,
    /// A change to one form or effect: Pierce, Sustain.
    Qualifier,
    /// Starts a child phrase: On first hit.
    Event,
}

/// Which part of a phrase a qualifier may sit on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Attach {
    Form,
    Effect,
    #[default]
    Either,
}

/// What an effect does to everything inside a ritual circle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Field {
    Burn,
    Chill,
    Shock,
    Venom,
    Drench,
    /// Throws foes outward.
    Push,
    /// Holds mana density at normal inside the circle.
    Purge,
    /// Builds the circle's footprint from the material.
    Place,
}

/// Colour group a rune shows in through rune goggles. The colours are the table's
/// `palette`; without goggles every rune reads in one plain ink.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Hue {
    /// Shapes and targets.
    #[default]
    Form,
    Fire,
    Frost,
    Shock,
    Venom,
    Water,
    /// Motion and impact.
    Force,
    Life,
    /// Stone, glass, metal and building.
    Earth,
    /// Mana itself: reach, duration, binding.
    Arcane,
    /// Triggers that start a child phrase.
    Event,
}

/// How a rune is drawn, on a grid 4 wide and 6 tall with y pointing down.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Glyph {
    #[default]
    None,
    /// Straight strokes `(x0, y0, x1, y1)`, like a carved letter.
    Strokes(Vec<(f32, f32, f32, f32)>),
    /// A cipher rune: one stave with this many branches to the left and to the right.
    /// Branches point up on a form qualifier, down on an effect qualifier and straight
    /// out on one that fits either.
    Cipher(u8, u8),
}

/// Grid size of a glyph: width, height.
pub const GLYPH_GRID: (f32, f32) = (4.0, 6.0);

fn one() -> f32 {
    1.0
}

fn first_tier() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rune {
    /// Upper-case word used in phrase text.
    pub key: String,
    /// Player-facing name.
    pub name: String,
    pub role: Role,
    /// Mana per cast.
    #[serde(default)]
    pub mana: f32,
    /// Mana per second while sustained.
    #[serde(default)]
    pub upkeep: f32,
    /// Qualifiers multiply the mana and upkeep of the part they sit on.
    #[serde(default = "one")]
    pub scale: f32,
    /// Lowest wand tier (1 to 5) that can hold this rune.
    #[serde(default = "first_tier")]
    pub tier: u8,
    /// Forms: the target is a drawn circle.
    #[serde(default)]
    pub circle: bool,
    /// Effects: a material must follow in parentheses.
    #[serde(default)]
    pub operand: bool,
    /// Qualifiers: the phrase is held and pays upkeep.
    #[serde(default)]
    pub sustain: bool,
    /// Qualifiers: the part this may sit on.
    #[serde(default)]
    pub on: Attach,
    /// Effects: what a ritual circle does with it.
    #[serde(default)]
    pub field: Option<Field>,
    /// Materials: the block PLACE builds with.
    #[serde(default)]
    pub block: Option<crate::BlockId>,
    /// Qualifiers: multiplies a circle's radius.
    #[serde(default = "one")]
    pub reach: f32,
    /// Deck spell id this rune lowers to on a card wand.
    #[serde(default)]
    pub card: Option<u32>,
    /// The card goes behind the form's card instead of in front.
    #[serde(default)]
    pub card_after: bool,
    #[serde(default)]
    pub glyph: Glyph,
    #[serde(default)]
    pub hue: Hue,
}

/// Index of a rune in its table.
pub type RuneId = u16;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Table {
    pub version: u32,
    /// Rune slots held by wand tiers I to V.
    pub capacity: [u32; 5],
    /// Deepest nesting of child phrases, the root counting as 1.
    pub max_depth: u32,
    /// Most events in one program, children included.
    pub max_events: u32,
    pub runes: Vec<Rune>,
    #[serde(default)]
    pub presets: Vec<String>,
    /// How existing deck spells read as rune phrases, for study and editing.
    #[serde(default)]
    pub recipes: Vec<Recipe>,
    /// Colour of each hue through rune goggles, as sRGB bytes.
    #[serde(default)]
    pub palette: Vec<(Hue, [u8; 3])>,
}

/// One deck spell written as a rune phrase.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Recipe {
    /// Spell id in `assets/spells.ron`.
    pub spell: u32,
    pub phrase: String,
}

/// A problem in a table or a phrase. `at` is a byte offset into the phrase text.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub at: usize,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Effect {
    pub rune: RuneId,
    pub material: Option<RuneId>,
    pub qualifiers: Vec<RuneId>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Phrase {
    pub form: RuneId,
    pub qualifiers: Vec<RuneId>,
    pub effect: Option<Effect>,
    /// Event rune and the child phrase it starts, in written order.
    pub events: Vec<(RuneId, Phrase)>,
}

/// A compiled phrase. Every reader uses these numbers; none re-derives them.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Program {
    pub root: Phrase,
    /// The phrase in its one canonical spelling.
    pub text: String,
    /// One per named rune, children included.
    pub slots: u32,
    /// Mana reserved per cast, with every child paid in advance.
    pub mana: f32,
    /// Mana per second while held; zero for a single cast.
    pub upkeep: f32,
    /// Lowest wand tier that can hold it, or `None` when only a ritual core can.
    pub tier: Option<u8>,
    pub events: u32,
    /// The root form targets a drawn circle.
    pub circle: bool,
    /// Stable identity of this text under this table version.
    pub id: u64,
}

impl Program {
    pub fn sustained(&self) -> bool {
        self.upkeep > 0.0
    }
}

impl Table {
    /// Parse and check a rune table. Presets must compile.
    pub fn parse(source: &str) -> Result<Self, Vec<Diagnostic>> {
        let table: Self = ron::from_str(source)
            .map_err(|error| vec![Diagnostic { at: 0, message: error.to_string() }])?;
        let mut errors = Vec::new();
        let mut fail = |message: String| errors.push(Diagnostic { at: 0, message });
        if table.runes.is_empty() {
            fail("Rune table is empty".into());
        }
        if table.runes.len() > RuneId::MAX as usize {
            fail("Too many runes".into());
        }
        if table.capacity.windows(2).any(|pair| pair[0] >= pair[1]) || table.capacity[0] == 0 {
            fail("Wand capacity must rise with each tier".into());
        }
        if table.max_depth == 0 {
            fail("max_depth must be at least 1".into());
        }
        for (index, rune) in table.runes.iter().enumerate() {
            let key_ok = !rune.key.is_empty()
                && rune.key.bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
                && rune.key != "THEN";
            if !key_ok {
                fail(format!("Rune key {:?} must be upper-case letters and underscores", rune.key));
            }
            if table.runes[..index].iter().any(|other| other.key == rune.key) {
                fail(format!("Rune key {} appears twice", rune.key));
            }
            if rune.name.trim().is_empty() {
                fail(format!("{}: name must not be blank", rune.key));
            }
            let numbers_ok = [rune.mana, rune.upkeep].iter().all(|v| v.is_finite() && *v >= 0.0)
                && rune.scale.is_finite()
                && rune.scale > 0.0;
            if !numbers_ok {
                fail(format!("{}: mana, upkeep and scale must be finite and not negative", rune.key));
            }
            if !(1..=5).contains(&rune.tier) {
                fail(format!("{}: tier must be 1 to 5", rune.key));
            }
            if rune.circle && rune.role != Role::Form {
                fail(format!("{}: only a form can target a circle", rune.key));
            }
            if rune.operand && rune.role != Role::Effect {
                fail(format!("{}: only an effect can take a material", rune.key));
            }
            if rune.field.is_some() && rune.role != Role::Effect {
                fail(format!("{}: only an effect can act in a circle", rune.key));
            }
            if rune.block.is_some() != (rune.role == Role::Material) {
                fail(format!("{}: a material, and only a material, names a block", rune.key));
            }
            if !(rune.reach.is_finite() && rune.reach > 0.0) {
                fail(format!("{}: reach must be positive", rune.key));
            }
            if rune.sustain && rune.role != Role::Qualifier {
                fail(format!("{}: only a qualifier can sustain", rune.key));
            }
            match &rune.glyph {
                Glyph::None => fail(format!("{}: needs a glyph", rune.key)),
                Glyph::Strokes(strokes) => {
                    let (w, h) = GLYPH_GRID;
                    let inside = |x: f32, y: f32| (0.0..=w).contains(&x) && (0.0..=h).contains(&y);
                    if strokes.is_empty() || !strokes.iter().all(|&(a, b, c, d)| inside(a, b) && inside(c, d)) {
                        fail(format!("{}: glyph strokes must lie on the 4 x 6 grid", rune.key));
                    }
                }
                Glyph::Cipher(left, right) => {
                    if rune.role != Role::Qualifier || *left > 3 || *right > 3 || left + right == 0 {
                        fail(format!("{}: cipher glyphs are for qualifiers, 1 to 3 branches a side", rune.key));
                    }
                }
            }
            if !table.palette.iter().any(|(hue, _)| *hue == rune.hue) {
                fail(format!("{}: hue {:?} has no palette colour", rune.key, rune.hue));
            }
            let strokes = table.strokes(index as RuneId);
            let same_look = (0..index).any(|other| table.strokes(other as RuneId) == strokes);
            if same_look {
                fail(format!("{}: glyph matches another rune", rune.key));
            }
        }
        if errors.is_empty() {
            let recipes = table.recipes.iter().map(|recipe| &recipe.phrase);
            for phrase in table.presets.iter().chain(recipes) {
                if let Err(error) = table.compile(phrase) {
                    errors.push(Diagnostic {
                        at: error.at,
                        message: format!("Phrase {phrase:?}: {}", error.message),
                    });
                }
            }
            for (index, recipe) in table.recipes.iter().enumerate() {
                if table.recipes[..index].iter().any(|other| other.spell == recipe.spell) {
                    errors.push(Diagnostic {
                        at: 0,
                        message: format!("Spell {} has two recipes", recipe.spell),
                    });
                }
            }
        }
        if errors.is_empty() { Ok(table) } else { Err(errors) }
    }

    pub fn rune(&self, id: RuneId) -> &Rune {
        &self.runes[id as usize]
    }

    /// Goggle colour of a rune as sRGB bytes.
    pub fn color(&self, id: RuneId) -> [u8; 3] {
        let hue = self.rune(id).hue;
        self.palette.iter().find(|(h, _)| *h == hue).map_or([220, 214, 196], |(_, rgb)| *rgb)
    }

    /// Strokes of a rune's glyph on the 4 x 6 grid, cipher runes expanded.
    pub fn strokes(&self, id: RuneId) -> Vec<(f32, f32, f32, f32)> {
        let rune = self.rune(id);
        match &rune.glyph {
            Glyph::None => Vec::new(),
            Glyph::Strokes(strokes) => strokes.clone(),
            Glyph::Cipher(left, right) => {
                let mut out = vec![(2.0, 0.0, 2.0, 6.0)];
                // Branch rise: up on a form qualifier, down on an effect one, level otherwise.
                let rise = match rune.on {
                    Attach::Form => -0.9,
                    Attach::Effect => 0.9,
                    Attach::Either => 0.0,
                };
                let mut side = |count: u8, dx: f32| {
                    for k in 0..count {
                        let y = 1.2 + k as f32 * 1.1;
                        out.push((2.0, y, 2.0 + dx, (y + rise).clamp(0.0, 6.0)));
                    }
                };
                side(*left, -1.6);
                side(*right, 1.6);
                out
            }
        }
    }

    /// Runes of a phrase in written order, events and children included, for glyph strips.
    pub fn written(&self, phrase: &Phrase) -> Vec<RuneId> {
        let mut out = vec![phrase.form];
        out.extend(&phrase.qualifiers);
        if let Some(effect) = &phrase.effect {
            out.push(effect.rune);
            out.extend(effect.material);
            out.extend(&effect.qualifiers);
        }
        for (event, child) in &phrase.events {
            out.push(*event);
            out.extend(self.written(child));
        }
        out
    }

    /// The rune phrase a deck spell reads as, if one is written down.
    pub fn recipe(&self, spell: u32) -> Option<&str> {
        self.recipes.iter().find(|recipe| recipe.spell == spell).map(|recipe| recipe.phrase.as_str())
    }

    pub fn find(&self, key: &str) -> Option<RuneId> {
        self.runes.iter().position(|rune| rune.key == key).map(|index| index as RuneId)
    }

    /// Lowest wand tier (1 to 5) with room for `slots` runes.
    pub fn tier_for(&self, slots: u32) -> Option<u8> {
        self.capacity.iter().position(|&cap| slots <= cap).map(|index| index as u8 + 1)
    }

    /// Compile phrase text. The first problem is returned with its position.
    pub fn compile(&self, text: &str) -> Result<Program, Diagnostic> {
        let tokens = lex(text)?;
        let mut parser = Parser { table: self, tokens: &tokens, next: 0, end: text.len() };
        let root = parser.phrase(1)?;
        if let Some(extra) = parser.peek() {
            return Err(parser.error_at(extra.at, "Expected the phrase to end here"));
        }
        let mut totals = Totals::default();
        self.total(&root, &mut totals);
        if totals.events > self.max_events {
            return Err(Diagnostic {
                at: 0,
                message: format!(
                    "{} events exceed the limit of {} (max_events)",
                    totals.events, self.max_events
                ),
            });
        }
        let mut canonical = String::new();
        self.write(&root, &mut canonical);
        let id = fnv1a(self.version, &canonical);
        let tier = self.tier_for(totals.slots).map(|tier| tier.max(totals.tier));
        Ok(Program {
            circle: self.rune(root.form).circle,
            root,
            text: canonical,
            slots: totals.slots,
            mana: totals.mana,
            upkeep: totals.upkeep,
            tier,
            events: totals.events,
            id,
        })
    }

    /// The deck cards that cast this program on a card wand, in slot order, or `None`
    /// when a rune has no card or the shape needs the circle runtime.
    pub fn lower(&self, program: &Program) -> Option<Vec<u32>> {
        let mut cards = Vec::new();
        self.lower_phrase(&program.root, &mut cards)?;
        Some(cards)
    }

    fn lower_phrase(&self, phrase: &Phrase, cards: &mut Vec<u32>) -> Option<()> {
        // A deck bolt carries one payload.
        if phrase.events.len() > 1 {
            return None;
        }
        for &qualifier in &phrase.qualifiers {
            cards.push(self.rune(qualifier).card?);
        }
        let mut after = None;
        if let Some(effect) = &phrase.effect {
            if effect.material.is_some() {
                return None;
            }
            for &qualifier in &effect.qualifiers {
                cards.push(self.rune(qualifier).card?);
            }
            let rune = self.rune(effect.rune);
            let card = rune.card?;
            if rune.card_after { after = Some(card) } else { cards.push(card) }
        }
        if let Some((event, _)) = phrase.events.first() {
            cards.push(self.rune(*event).card?);
        }
        cards.push(self.rune(phrase.form).card?);
        cards.extend(after);
        if let Some((_, child)) = phrase.events.first() {
            self.lower_phrase(child, cards)?;
        }
        Some(())
    }

    fn total(&self, phrase: &Phrase, totals: &mut Totals) {
        let form = self.rune(phrase.form);
        let mut sustained = false;
        let mut part = |rune: &Rune, qualifiers: &[RuneId], totals: &mut Totals| {
            let mut scale = 1.0;
            for &id in qualifiers {
                let qualifier = self.rune(id);
                scale *= qualifier.scale;
                sustained |= qualifier.sustain;
                totals.count(qualifier);
            }
            totals.count(rune);
            (rune.mana * scale, rune.upkeep * scale)
        };
        let (mut mana, mut upkeep) = part(form, &phrase.qualifiers, totals);
        if let Some(effect) = &phrase.effect {
            let (effect_mana, effect_upkeep) = part(self.rune(effect.rune), &effect.qualifiers, totals);
            mana += effect_mana;
            upkeep += effect_upkeep;
            if let Some(material) = effect.material {
                let material = self.rune(material);
                totals.count(material);
                mana += material.mana;
            }
        }
        totals.mana += mana;
        if sustained {
            totals.upkeep += upkeep;
        }
        for (event, child) in &phrase.events {
            let event = self.rune(*event);
            totals.count(event);
            totals.mana += event.mana;
            totals.events += 1;
            self.total(child, totals);
        }
    }

    fn write(&self, phrase: &Phrase, out: &mut String) {
        out.push_str(&self.rune(phrase.form).key);
        self.write_qualifiers(&phrase.qualifiers, out);
        if let Some(effect) = &phrase.effect {
            out.push_str(" then ");
            out.push_str(&self.rune(effect.rune).key);
            if let Some(material) = effect.material {
                out.push('(');
                out.push_str(&self.rune(material).key);
                out.push(')');
            }
            self.write_qualifiers(&effect.qualifiers, out);
        }
        for (event, child) in &phrase.events {
            out.push_str(" then ");
            out.push_str(&self.rune(*event).key);
            out.push_str(" { ");
            self.write(child, out);
            out.push_str(" }");
        }
    }

    fn write_qualifiers(&self, qualifiers: &[RuneId], out: &mut String) {
        for (index, &id) in qualifiers.iter().enumerate() {
            out.push(if index == 0 { '[' } else { ',' });
            out.push_str(&self.rune(id).key);
        }
        if !qualifiers.is_empty() {
            out.push(']');
        }
    }
}

#[derive(Default)]
struct Totals {
    slots: u32,
    mana: f32,
    upkeep: f32,
    events: u32,
    tier: u8,
}

impl Totals {
    fn count(&mut self, rune: &Rune) {
        self.slots += 1;
        self.tier = self.tier.max(rune.tier);
    }
}

fn fnv1a(version: u32, text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in version.to_le_bytes().into_iter().chain(text.bytes()) {
        hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Word(String),
    Then,
    Open(char),
    Close(char),
    Comma,
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    at: usize,
}

fn lex(text: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(at, c)) = chars.peek() {
        let kind = match c {
            c if c.is_whitespace() => {
                chars.next();
                continue;
            }
            '>' => Kind::Then,
            '[' | '(' | '{' => Kind::Open(c),
            ']' | ')' | '}' => Kind::Close(c),
            ',' => Kind::Comma,
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut word = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if !(c.is_ascii_alphabetic() || c == '_') {
                        break;
                    }
                    word.push(c.to_ascii_uppercase());
                    chars.next();
                }
                let kind = if word == "THEN" { Kind::Then } else { Kind::Word(word) };
                tokens.push(Token { kind, at });
                continue;
            }
            other => {
                return Err(Diagnostic { at, message: format!("Unexpected character {other:?}") });
            }
        };
        chars.next();
        tokens.push(Token { kind, at });
    }
    Ok(tokens)
}

struct Parser<'a> {
    table: &'a Table,
    tokens: &'a [Token],
    next: usize,
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.next)
    }

    fn at(&self) -> usize {
        self.peek().map_or(self.end, |token| token.at)
    }

    fn error_at(&self, at: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic { at, message: message.into() }
    }

    fn eat(&mut self, kind: &Kind) -> bool {
        let hit = self.peek().is_some_and(|token| &token.kind == kind);
        self.next += hit as usize;
        hit
    }

    fn expect(&mut self, kind: Kind, what: &str) -> Result<(), Diagnostic> {
        if self.eat(&kind) { Ok(()) } else { Err(self.error_at(self.at(), format!("Expected {what}"))) }
    }

    /// The next word as a rune, with where it was written.
    fn rune(&mut self, what: &str) -> Result<(RuneId, usize), Diagnostic> {
        let at = self.at();
        let Some(Token { kind: Kind::Word(word), .. }) = self.peek() else {
            return Err(self.error_at(at, format!("Expected {what}")));
        };
        let id = self
            .table
            .find(word)
            .ok_or_else(|| self.error_at(at, format!("{word} is not a rune")))?;
        self.next += 1;
        Ok((id, at))
    }

    fn role(&mut self, role: Role, what: &str) -> Result<RuneId, Diagnostic> {
        let (id, at) = self.rune(what)?;
        let rune = self.table.rune(id);
        if rune.role == role {
            Ok(id)
        } else {
            Err(self.error_at(at, format!("{} is not {what}", rune.key)))
        }
    }

    fn qualifiers(&mut self, on: Attach, host: RuneId) -> Result<Vec<RuneId>, Diagnostic> {
        let mut list = Vec::new();
        if !self.eat(&Kind::Open('[')) {
            return Ok(list);
        }
        loop {
            let at = self.at();
            let id = self.role(Role::Qualifier, "a qualifier")?;
            let rune = self.table.rune(id);
            let host = self.table.rune(host);
            if rune.on != Attach::Either && rune.on != on {
                let part = if rune.on == Attach::Form { "a form" } else { "an effect" };
                return Err(self.error_at(at, format!("{} belongs on {part}", rune.key)));
            }
            if rune.sustain && host.upkeep <= 0.0 {
                return Err(self.error_at(at, format!("{} cannot be held, so {} has nothing to sustain", host.key, rune.key)));
            }
            if list.contains(&id) {
                return Err(self.error_at(at, format!("{} is already on {}", rune.key, host.key)));
            }
            list.push(id);
            if !self.eat(&Kind::Comma) {
                break;
            }
        }
        self.expect(Kind::Close(']'), "] to close the qualifiers")?;
        Ok(list)
    }

    fn phrase(&mut self, depth: u32) -> Result<Phrase, Diagnostic> {
        if depth > self.table.max_depth {
            return Err(self.error_at(
                self.at(),
                format!("Child phrases nest deeper than {} (max_depth)", self.table.max_depth),
            ));
        }
        let form = self.role(Role::Form, "a form")?;
        let qualifiers = self.qualifiers(Attach::Form, form)?;
        let mut phrase = Phrase { form, qualifiers, effect: None, events: Vec::new() };
        while self.eat(&Kind::Then) {
            let (id, at) = self.rune("an effect or an event")?;
            let rune = self.table.rune(id);
            match rune.role {
                Role::Effect => {
                    if phrase.effect.is_some() {
                        return Err(self.error_at(at, "A phrase has one effect; put the next in an event's child phrase"));
                    }
                    if !phrase.events.is_empty() {
                        return Err(self.error_at(at, "The effect comes before the events"));
                    }
                    let material = if self.eat(&Kind::Open('(')) {
                        if !rune.operand {
                            return Err(self.error_at(at, format!("{} takes no material", rune.key)));
                        }
                        let material = self.role(Role::Material, "a material")?;
                        self.expect(Kind::Close(')'), ") after the material")?;
                        Some(material)
                    } else if rune.operand {
                        return Err(self.error_at(self.at(), format!("{} needs a material, as in {}(STONE)", rune.key, rune.key)));
                    } else {
                        None
                    };
                    let qualifiers = self.qualifiers(Attach::Effect, id)?;
                    phrase.effect = Some(Effect { rune: id, material, qualifiers });
                }
                Role::Event => {
                    self.expect(Kind::Open('{'), "{ to open the child phrase")?;
                    let child = self.phrase(depth + 1)?;
                    self.expect(Kind::Close('}'), "} to close the child phrase")?;
                    phrase.events.push((id, child));
                }
                _ => {
                    return Err(self.error_at(at, format!("{} is not an effect or an event", rune.key)));
                }
            }
        }
        Ok(phrase)
    }
}
