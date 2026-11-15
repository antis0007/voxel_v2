use super::inventory::draw_item_icon;
use super::presentation::{AMBER, MUTED};
use super::recipe_book;
use crate::keybinds::{Action, hint};
use crate::world::{BlockId as B, adventure, inventory::Item};
use egui::{Color32, RichText, Sense, vec2};

const SPELL_INDEX: usize = 7;
const CIRCLE: usize = 2;
const GATES: usize = 3;

pub(super) fn open_circle(ctx: &egui::Context) {
    recipe_book::open_magic_guide(ctx);
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("magic_guide_entry"), CIRCLE);
        d.remove::<String>(egui::Id::new("magic_guide_query"));
    });
}
const ENTRIES: [(&str, &str, &str); SPELL_INDEX + 1] = [
    ("Start here", "Early game", "craft wand workbench components materials copper crystal fibre"),
    ("Casting and decks", "Early game", "spell cards cast equip gear slots modifiers triggers mana recharge"),
    ("Teleportation Circle", "Early game", "recall temporary return waypoint anchor draw circle link teleportation circles"),
    ("Crystal Gates", "Build a route", "teleportation circles permanent link travel portal frame waypoint"),
    ("Study and discovery", "Explore and study", "spells lessons bookcase hydromancy water wards celestial unlock XP"),
    ("Grimoire bindings", "Midgame", "enchanting table reservoir focus concentration salvage bind"),
    ("Working flowers", "Midgame workshop", "mana machinery collector flower automation farm chest"),
    ("Spell Index", "Reference", "spell cards spells mana tier"),
];

pub(super) fn draw(ui: &mut egui::Ui, cells_per_metre: i32) {
    let selected_id = egui::Id::new("magic_guide_entry");
    let query_id = egui::Id::new("magic_guide_query");
    let (mut selected, mut query) = ui.ctx().data(|d| {
        (d.get_temp::<usize>(selected_id).unwrap_or(0),
         d.get_temp::<String>(query_id).unwrap_or_default())
    });
    ui.heading(RichText::new("Field Guide to Magic").color(AMBER));
    ui.add(egui::TextEdit::singleline(&mut query).hint_text("Find a spell, topic or material"));
    let needle = query.trim().to_lowercase();
    let matches: Vec<_> = ENTRIES.iter().enumerate().filter(|(_, (title, stage, keywords))| {
        needle.split_whitespace().all(|word| {
            title.to_lowercase().contains(word) || stage.to_lowercase().contains(word)
                || keywords.to_lowercase().contains(word)
        })
    }).map(|(index, _)| index)
        .chain((!needle.is_empty() && spell_results(&needle).next().is_some()).then_some(SPELL_INDEX))
        .collect();
    let mut matches = matches;
    matches.dedup();
    if matches.is_empty() {
        ui.weak("No matching chapters.");
        ui.ctx().data_mut(|d| d.insert_temp(query_id, query));
        return;
    }
    if !matches.contains(&selected) {
        selected = matches[0];
    }
    ui.horizontal_wrapped(|ui| {
        for &index in &matches {
            ui.selectable_value(&mut selected, index, ENTRIES[index].0);
        }
    });
    ui.separator();
    let (title, stage, _) = ENTRIES[selected];
    ui.label(RichText::new(stage).color(MUTED));
    ui.heading(title);
    ui.push_id(selected, |ui| {
        if selected == SPELL_INDEX {
            spell_index(ui, &needle);
        } else if let Some(next) = article(ui, selected, cells_per_metre) {
            selected = next;
            query.clear();
        }
    });
    ui.ctx().data_mut(|d| {
        d.insert_temp(selected_id, selected);
        d.insert_temp(query_id, query);
    });
}

fn spell_results(query: &str) -> impl Iterator<Item = &'static adventure::spell_defs::SpellDef> {
    use adventure::spell_defs::{Palette, SPELLS};
    SPELLS.iter().filter(move |spell| {
        spell.palette != Palette::Hidden && query.split_whitespace().all(|word| {
            spell.name.to_lowercase().contains(word) || spell.tag.to_lowercase().contains(word)
        })
    })
}

fn spell_index(ui: &mut egui::Ui, query: &str) {
    use adventure::spell_defs::{Unlock, card};
    let mut rows = spell_results(query).peekable();
    if rows.peek().is_none() {
        rows = spell_results("").peekable();
    }
    for row in rows {
        egui::CollapsingHeader::new(row.name).id_salt(row.id).show(ui, |ui| {
            spell(ui, row.id);
            let unlock = match row.unlock {
                Unlock::FirstWand => "Tier I",
                Unlock::SecondWandOrRare => "Tier II or a rare discovery",
                Unlock::ThirdWandOrTwoRare => "Tier III or two rare discoveries",
                Unlock::ThirdWand => "Tier III",
                Unlock::RareBit => "Reliquary discovery",
                Unlock::Never => "Unavailable",
            };
            let card = card::read(row.id, 0, 0);
            ui.label(format!("{unlock} · {}", card.cost));
            if !card.stats.is_empty() {
                ui.label(card.stats);
            }
            if let Some(description) = adventure::spell_defs::card::description(row.id) {
                ui.label(description.text.as_str());
            }
        });
    }
}

fn recipe(ui: &mut egui::Ui, item: Item) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        draw_item_icon(ui.painter(), r, item);
        if ui.link(format!("{} recipe", item.name())).clicked() {
            recipe_book::request(ui.ctx(), item, false);
        }
    });
}

fn steps(ui: &mut egui::Ui, instructions: &[&str]) {
    for (index, instruction) in instructions.iter().enumerate() {
        ui.label(format!("{}. {instruction}", index + 1));
    }
}

fn spell(ui: &mut egui::Ui, id: u32) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        super::icons::paint(ui.painter(), r, super::icons::Icon::Spell(id));
        ui.strong(adventure::spell_name(id));
    });
}

fn deck_example(ui: &mut egui::Ui, cards: &[u32]) {
    ui.horizontal_wrapped(|ui| {
        for (i, &id) in cards.iter().enumerate() {
            if i > 0 {
                ui.label("→");
            }
            spell(ui, id);
        }
    });
}

fn gate_diagram(ui: &mut egui::Ui, cells_per_metre: i32) {
    let width = 2 * cells_per_metre;
    let height = 3 * cells_per_metre;
    let cell_size = (84.0 / (width + 2) as f32).clamp(9.0, 20.0);
    egui::Grid::new("field_guide_crystal_frame").spacing([1.0, 1.0]).min_col_width(cell_size).min_row_height(cell_size).show(ui, |ui| {
        for y in 0..height + 2 {
            for x in 0..width + 2 {
                let (r, response) = ui.allocate_exact_size(vec2(cell_size, cell_size), Sense::hover());
                let top_or_bottom = y == 0 || y == height + 1;
                let side = x == 0 || x == width + 1;
                if top_or_bottom && side {
                    response.on_hover_text("Optional corner");
                } else if top_or_bottom || side {
                    draw_item_icon(ui.painter(), r, Item::Block(B::Crystal));
                    response.on_hover_text("Resonant crystal block");
                } else {
                    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(45, 30, 75));
                    response.on_hover_text("Clear opening");
                }
            }
            ui.end_row();
        }
    });
    let blocks = 2 * (width + height);
    ui.weak(format!("2 m × 3 m opening · {width} × {height} cells · {blocks} crystal blocks per frame"));
}

fn article(ui: &mut egui::Ui, entry: usize, cells_per_metre: i32) -> Option<usize> {
    let mut next = None;
    match entry {
        0 => {
            steps(ui, &[
                "Build and place an Artificer workbench.",
                "Open Gear beside the workbench and choose Craft lumen wand.",
                "Select the wand in the hotbar. Open Gear to arrange the spell cards.",
            ]);
            recipe(ui, Item::Block(B::Workbench));
            ui.strong("Lumen Wand Components");
            for &(resource, units) in adventure::wand_recipe(0) {
                ui.label(format!("{} × {}", units as f32 / 64.0, Item::Resource(resource as u8).name()));
            }
            ui.collapsing("Component Sources", |ui| {
                ui.label("Copper · Mine copper ore and process it into ingots. Open the ingot recipe for its production chain.");
                ui.label("Resonant Crystal · Stone Pickaxe or Better");
                recipe(ui, Item::Tool(7));
                ui.label("Lumen fibre · Harvest Glow Caps. Check carried ingredients in Gear.");
                recipe(ui, Item::Resource(6));
            });
            if ui.link("Next · Casting and decks").clicked() { next = Some(1); }
        }
        1 => {
            let cast = hint(Action::Cast);
            let skip = hint(Action::NextSpell);
            ui.label(format!("Select the wand in the hotbar. [{cast}] Cast · [{skip}] Next Spell"));
            ui.label("Gear shows the slots and the next cast. Drag a known card into a slot. Check mana cost and recharge before closing Gear.");
            spell(ui, 0);
            ui.label("Practice with Spark in the first slot. Aim at open ground and cast once. Watch the slot cursor and mana bar.");
            ui.collapsing("Modifiers, payloads and concentration", |ui| {
                ui.label("Put a modifier before a compatible bolt. Compare the next-cast readout before and after.");
                ui.label("Put a payload after a trigger bolt. The trigger releases the payload at impact. The payload adds to the cast's mana cost.");
                deck_example(ui, &[18, 0]);
                ui.label(format!("Hold [{cast}] to sustain a learned concentration spell. Use it as a single card and watch its mana upkeep."));
            });
            if ui.link("Teleportation Circle").clicked() { next = Some(CIRCLE); }
            if ui.link("Learn more spells").clicked() { next = Some(4); }
        }
        CIRCLE => {
            let id = adventure::fieldcraft::RECALL;
            spell(ui, id);
            let cast = hint(Action::Cast);
            let next_spell = hint(Action::NextSpell);
            let sneak = hint(Action::Sneak);
            let card = adventure::spell_defs::card::read(id, 0, 0);
            ui.strong(format!("Starting spell · Tier I wand · {} per cast", card.cost));
            ui.label(format!("Gear → Effects → Teleportation Circle · [{next_spell}] Next Spell"));
            ui.strong("Permanent Link");
            steps(ui, &[
                &format!("Hold [{sneak}] and tap [{cast}] on clear ground at home."),
                "Sneak-cast again at the mine or dungeon to link it.",
                &format!("Stand inside either circle. Tap [{cast}] to travel."),
            ]);
            ui.weak("Single spell card · One pair per world");
            ui.weak("Sneak-cast elsewhere to move the far end. Sneak-cast inside to clear.");
            ui.strong("Quick Return");
            ui.label(format!("Outside a link, tap [{cast}] to mark. Release and cast again to return."));
            ui.strong("Home Escape");
            ui.label(format!("Hold [{cast}] 3 s anywhere in the realm to return to the home circle."));
            ui.weak("One way · A hit restarts the hold");
            ui.weak(card.stats);
            if ui.link("Start here · Craft a wand").clicked() { next = Some(0); }
            if ui.link("Walk-through portals · Crystal Gates").clicked() { next = Some(GATES); }
        }
        GATES => {
            let interact = hint(Action::Interact);
            ui.strong("Two-way travel · Same world");
            gate_diagram(ui, cells_per_metre);
            steps(ui, &[
                "Build two matching upright Resonant Crystal frames. Leave the openings clear.",
                &format!("Aim at a crystal block on the first frame within 5 m. [{interact}] Attune."),
                &format!("Aim at the second frame. [{interact}] Link. Walk through either gate."),
            ]);
            ui.weak("Optional corners · No spell required");
            ui.label("Resonant Crystal · Mine underground with a Stone Pickaxe or better.");
            recipe(ui, Item::Block(B::Crystal));
            if ui.link("Teleportation Circle").clicked() { next = Some(CIRCLE); }
        }
        4 => {
            let starters = adventure::discovery::STARTER_SPELLS.into_iter()
                .chain([adventure::fieldcraft::RECALL])
                .map(adventure::spell_name)
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(format!("Starting spells · {starters}"));
            ui.label("Spell finds · Site clears, first guardian defeats, wand upgrades");
            ui.label("Bookcase · XP or supplies → Lessons");
            ui.label("Learned projects · Supplies → Research XP");
            ui.weak("Hover a locked card in Gear for its source and wand tier.");
            recipe(ui, Item::Block(B::Bookcase));
            recipe_book::study(ui);
            if ui.link("Next · Grimoire bindings").clicked() { next = Some(5); }
        }
        5 => {
            let table = &*adventure::enchanting::TABLE;
            ui.label(format!("Hold a tier III wand and interact with an Enchanting Table. A binding costs {} XP and {} × {}.", table.xp, table.crystals, table.crystal().name()));
            for binding in &table.bindings {
                ui.label(format!("{} · {}", binding.name, binding.effect));
            }
            ui.label("The binding keeps the wand's cards and teaches Salvage Draw. Equip that spell by itself to pull visible dropped items toward you.");
            recipe(ui, Item::Block(B::EnchantingTable));
            if ui.link("Next · Working flowers").clicked() { next = Some(6); }
        }
        _ => {
            ui.strong("Mana flower");
            ui.label("Plant on grass, dirt or grove moss under open sky. A mature flower sends 1 mana every 2 s to a receiver within 8 m, or restores mana to a player in range.");
            let feed = crate::world::farming::table::by_seed(B::ManaFlower).and_then(|(_, crop)| match crop.aura {
                Some(crate::world::farming::table::Aura::Mana { feed, .. }) => feed,
                _ => None,
            });
            if let Some(feed) = feed {
                ui.label(format!("Drop {} within {} m of it. Every 2 s it eats one and sends {} mana, even under a roof.",
                    feed.item.name(), feed.reach_metres, feed.mana_per_item));
            }
            recipe(ui, Item::Block(B::ManaFlower));
            ui.strong("Collector flower");
            ui.label("Plant on grass, dirt, grove moss or sand, touching a chest. Once mature, it gathers dropped stacks within 6 m while the chest has space.");
            recipe(ui, Item::Block(B::CollectorFlower));
            ui.label("Try dropping one item beside a mature Collector flower. Check the chest after the next pulse.");
        }
    }
    next
}

