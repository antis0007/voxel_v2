//! Bookcase studies and repeatable supply research connect production to permanent knowledge.
use crate::player::Player;
use crate::world::{BlockId, World, adventure, interaction::Prompt};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Lesson {
    Water,
    Wards,
    Celestial,
    Thresholds,
    Horticulture,
    Prospecting = 5,
    Provisioning = 6,
    Wayfaring = 7,
}

pub(crate) const METEOR: u32 = 29;
pub(crate) const LEGACY_MAX_AFFINITY: u8 = 3;
pub(crate) const CONDUCTION_OBSERVED: u8 = 1 << 4;
pub(crate) const LESSON_MASK: u8 = 0xff;

mod curriculum;
pub(crate) use curriculum::catalog;

impl Lesson {
    pub const ALL: [Self; 8] = [
        Self::Water, Self::Wards, Self::Celestial, Self::Thresholds, Self::Horticulture,
        Self::Prospecting, Self::Provisioning, Self::Wayfaring,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Water => "Hydromancy",
            Self::Wards => "Protective Magic",
            Self::Celestial => "Celestial Magic",
            Self::Thresholds => "Threshold Survey",
            Self::Horticulture => "Horticulture",
            Self::Prospecting => "Prospecting",
            Self::Provisioning => "Provisioning",
            Self::Wayfaring => "Wayfaring",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Water => "Water Transfer",
            Self::Wards => "Ice Wall and Aegis Field",
            Self::Celestial => "Meteor and Plasma Cross",
            Self::Thresholds => "Frame diagnostics and sanctuary bearings",
            Self::Horticulture => "+1 ripe crop per harvest",
            Self::Prospecting => "+1 XP per experience-bearing block",
            Self::Provisioning => "+25% food saturation",
            Self::Wayfaring => "15% less hunger drain",
        }
    }

    pub(crate) fn for_spell(spell: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|lesson| lesson.spells().contains(&spell))
    }

    pub fn cost(self) -> u32 {
        self.project().map_or(u32::MAX, |row| row.xp)
    }

    pub fn project(self) -> Option<&'static curriculum::Project> {
        catalog().iter().find(|row| row.lesson == self)
    }

    pub fn research_xp(self) -> u32 {
        self.project().map_or(0, |row| (row.xp / 4).max(1))
    }

    pub fn ready(self, lessons: u8, horticulture: bool) -> bool {
        self.project().is_some_and(|row| row.requires.is_none_or(|parent| parent.learned(lessons, horticulture)))
    }

    pub fn learned(self, lessons: u8, horticulture: bool) -> bool {
        match self {
            Self::Horticulture => horticulture,
            _ => lessons & (1 << self as u8) != 0,
        }
    }

    fn spells(self) -> &'static [u32] {
        match self {
            Self::Water => &[95],
            Self::Wards => &[75, adventure::concentration::AEGIS],
            Self::Celestial => &[METEOR, adventure::concentration::PLASMA],
            _ => &[],
        }
    }
}

pub(crate) fn available(world: &World) -> bool {
    world.ca_view.is_none() && world.rules.mode.interacts()
        && !world.rules.mode.creative_tools() && !world.adventure.creative
}

pub(crate) fn prompt(world: &World, player: &Player) -> Option<Prompt> {
    adventure::chronicle::bookcase(world, player)?;
    if super::super::undercrown::focus(
        world, player.eye(world.voxel_scale) * world.voxel_scale.meters(), player.forward_dir(),
    ).is_some() {
        return None;
    }
    Some(Prompt { title: "Bookcase", action: "[F] Read field guide" })
}

pub(crate) fn guide_focused(world: &World, player: &Player) -> bool {
    prompt(world, player).is_some()
        && adventure::chronicle::prompt(world, player).is_none()
        && crate::world::interaction::target(world, player).is_none()
}

pub(crate) fn buy(world: &mut World, player: &Player, lesson: Lesson) -> bool {
    purchase(world, player, lesson, false, false)
}

pub(crate) fn buy_materials(world: &mut World, player: &Player, lesson: Lesson, research: bool) -> bool {
    purchase(world, player, lesson, true, research)
}

fn purchase(world: &mut World, player: &Player, lesson: Lesson, materials: bool, research: bool) -> bool {
    if !available(world) || !guide_focused(world, player) {
        world.expedition.say("Study at a bookcase");
        return false;
    }
    // Reject stale purchase/research intents instead of charging for a different action.
    if lesson.learned(world.progression.lessons, world.progression.horticulture) != research {
        return false;
    }
    if !lesson.ready(world.progression.lessons, world.progression.horticulture) {
        world.expedition.say("Prerequisite study required");
        return false;
    }
    let Some(project) = lesson.project() else { return false };
    if materials {
        // Stage the entire payment; partial supplies never consume anything.
        let mut inventory = world.inventory.clone();
        for &(item, count) in &project.samples {
            if !inventory.remove_item(item, count) {
                world.expedition.say("Study supplies missing");
                return false;
            }
        }
        world.inventory = inventory;
        world.sync_from_inventory();
    } else if world.progression.xp < lesson.cost() {
        world.expedition.say(format!("{} XP required", lesson.cost()));
        return false;
    } else {
        world.progression.xp -= lesson.cost();
    }
    if research {
        let xp = lesson.research_xp();
        super::grant_xp(world, xp);
        world.stats.add(crate::world::stats::Stat::Research, 1);
        world.expedition.say(format!("Research · +{xp} XP"));
        return true;
    }
    match lesson {
        Lesson::Horticulture => world.progression.horticulture = true,
        _ => world.progression.lessons |= 1 << lesson as u8,
    }
    restore_spells(world);
    world.stats.add(crate::world::stats::Stat::Studies, 1);
    world.expedition.say(format!("{} learned", lesson.title()));
    true
}

#[derive(Clone, Copy)]
pub(crate) struct View {
    pub xp: u32,
    pub lessons: u8,
    pub horticulture: bool,
    pub can_study: bool,
    pub supplies: u8,
    pub supplied: [[u32; 4]; 8],
    pub sanctuary: Option<(i32, i32)>,
}

pub(crate) fn view(world: &World, player: &Player) -> View {
    let cfg = world.sample_config();
    let lessons = world.progression.lessons;
    let mut supplied = [[0; 4]; 8];
    for row in catalog() {
        for (index, &(item, _)) in row.samples.iter().enumerate() {
            supplied[row.lesson as usize][index] = world.inventory.count_item(item);
        }
    }
    // The porter's record teaches the survey method for finding another sanctuary.
    let sanctuary = (Lesson::Thresholds.learned(lessons, false)
        && world.progression.lore & (1 << 12) != 0
        && engine_worldgen::features::surface(&cfg).is_some()
        && engine_worldgen::hell_gate::enabled(&cfg))
        .then(|| {
            let center = engine_worldgen::hell_gate::center(&cfg, player.pos * cfg.voxel_meters());
            (center.x as i32, center.z as i32)
        });
    View {
        xp: world.progression.xp,
        lessons,
        horticulture: world.progression.horticulture,
        can_study: available(world) && guide_focused(world, player),
        supplies: catalog().iter().filter(|row| row.samples.iter()
            .all(|&(item, count)| world.inventory.count_item(item) >= count))
            .fold(0, |mask, row| mask | (1 << row.lesson as u8)),
        supplied,
        sanctuary,
    }
}

pub(crate) fn food_crop(block: BlockId) -> bool {
    matches!(block, BlockId::RedCap | BlockId::GlowCap | BlockId::WheatCrop | BlockId::CarrotCrop | BlockId::BeetCrop
        | BlockId::Potato | BlockId::Onion | BlockId::Cabbage | BlockId::RiceCrop)
}

pub(crate) fn restore_spells(world: &mut World) {
    world.adventure.study_lessons = world.progression.lessons;
}

pub(crate) fn grant_spells(log: &mut adventure::discovery::SpellLog, lessons: u8) {
    for lesson in Lesson::ALL {
        if lesson.learned(lessons, false) {
            for &spell in lesson.spells() {
                log.add(spell);
            }
        }
    }
}
