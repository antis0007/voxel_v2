//! Authored Undercrown plan: rooms, props and links. Generator 19–31 use the
//! box plan below; generator 32 authors real spaces in `v2`.
use super::state::ObjectId;
use crate::{BlockId, WorldSampleConfig};
use glam::Vec3;

/// Reserve this much solid rock around the authored footprint.
pub const RESERVE_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum District {
    RootboundHalls,
    CrownChasm,
    DrownedBorough,
    LanternGardens,
    AshenFoundry,
    GlassArchive,
    FrozenChoir,
    HollowSea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Rootbound,
    Canal,
    Cliff,
    Fungal,
    Foundry,
    Archive,
    Ice,
    Sea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    Intact,
    Collapsed,
    Flooded,
    Overgrown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    WellHead,
    Landing,
    CounterweightHall,
    Cistern,
    Passage,
    Refuge,
    Chasm,
    Village,
    Bell,
    Observatory,
    Garden,
    Foundry,
    Choir,
    Kitchen,
    Vault,
    Lift,
    Chapel,
    Bath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Marker,
    Counterweight,
    Cradle,
    SluiceGate,
    Winch,
    Plinth,
    BellClapper,
    ObservatoryLens,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Inspect,
    Operate,
    Repair,
    Open,
    Close,
    Claim,
    Restore,
    Ring,
}

#[derive(Clone, Copy, Debug)]
pub struct RoomSpec {
    pub id: ObjectId,
    pub kind: RoomKind,
    pub name: &'static str,
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub floor: f32,
    pub headroom: f32,
    pub style: Style,
    pub damage: Damage,
}

impl District {
    /// Save identity. Explicit so reordering the enum does not renumber objects.
    pub fn index(self) -> u8 {
        match self {
            Self::RootboundHalls => 0,
            Self::CrownChasm => 1,
            Self::DrownedBorough => 2,
            Self::LanternGardens => 3,
            Self::AshenFoundry => 4,
            Self::GlassArchive => 5,
            Self::FrozenChoir => 6,
            Self::HollowSea => 7,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub id: ObjectId,
    pub kind: PropKind,
    pub label: &'static str,
    pub pos: Vec3,
    pub yaw: f32,
}

impl Prop {
    pub fn action(self, state: u32) -> Action {
        match self.kind {
            PropKind::Marker => Action::Inspect,
            PropKind::Counterweight | PropKind::Winch => Action::Operate,
            PropKind::Cradle => Action::Repair,
            PropKind::SluiceGate => {
                if state & super::state::flags::OPEN != 0 {
                    Action::Close
                } else {
                    Action::Open
                }
            }
            PropKind::Plinth => Action::Claim,
            PropKind::BellClapper => Action::Ring,
            PropKind::ObservatoryLens => Action::Restore,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Passage,
    Sightline,
    Water,
    Freight,
}

#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub from: ObjectId,
    pub to: ObjectId,
    pub kind: LinkKind,
}

/// The complete authored footprint for one world. Positions are metres.
#[derive(Clone, Debug)]
pub struct Plan {
    pub anchor: Vec3,
    bounds: (Vec3, Vec3),
    pub rooms: Vec<RoomSpec>,
    pub links: Vec<Link>,
    pub props: Vec<Prop>,
    /// Generator 32 geometry; `None` for the generator 19 box plan.
    pub v2: Option<Box<super::v2::Layout>>,
}

impl Plan {
    pub fn empty() -> Self {
        Self {
            anchor: Vec3::ZERO,
            bounds: (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            rooms: Vec::new(),
            links: Vec::new(),
            props: Vec::new(),
            v2: None,
        }
    }

    /// v18 and earlier stay empty. v19 authors the Undercrown outside the
    /// locked generation grid so existing v18 hashes stay put.
    pub fn for_config(cfg: &WorldSampleConfig) -> Self {
        if cfg.generator_version < super::FIRST_VERSION {
            return Self::empty();
        }
        if cfg.generator_version >= super::v2::VERSION {
            let (rooms, props, layout) = super::v2::author(cfg);
            return Self {
                anchor: layout.anchor,
                bounds: (layout.min, layout.max),
                rooms,
                links: Vec::new(),
                props,
                v2: Some(Box::new(layout)),
            };
        }
        authored(cfg)
    }

    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty() && self.props.is_empty()
    }

    fn has_bounds(&self) -> bool {
        self.bounds.0.x <= self.bounds.1.x
    }

    pub fn contains(&self, m: Vec3) -> bool {
        self.has_bounds()
            && m.x >= self.bounds.0.x
            && m.x <= self.bounds.1.x
            && m.y >= self.bounds.0.y
            && m.y <= self.bounds.1.y
            && m.z >= self.bounds.0.z
            && m.z <= self.bounds.1.z
    }

    pub fn reserved(&self, m: Vec3) -> bool {
        self.has_bounds()
            && m.x >= self.bounds.0.x - RESERVE_MARGIN
            && m.x <= self.bounds.1.x + RESERVE_MARGIN
            && m.y >= self.bounds.0.y - RESERVE_MARGIN
            && m.y <= self.bounds.1.y + RESERVE_MARGIN
            && m.z >= self.bounds.0.z - RESERVE_MARGIN
            && m.z <= self.bounds.1.z + RESERVE_MARGIN
    }

    /// True when a box touches the reserved footprint.
    pub fn overlaps(&self, lo: Vec3, hi: Vec3) -> bool {
        let (a, b) = (self.bounds.0 - RESERVE_MARGIN, self.bounds.1 + RESERVE_MARGIN);
        self.has_bounds() && lo.cmple(b).all() && hi.cmpge(a).all()
    }

    pub fn location(&self, m: Vec3) -> Option<&'static str> {
        self.room_at(m).map(|room| room.name)
    }

    /// Smallest room containing the column wins, so a house inside the chasm
    /// dresses itself instead of becoming more chasm air.
    pub fn room_at(&self, m: Vec3) -> Option<&RoomSpec> {
        let mut best: Option<&RoomSpec> = None;
        for room in &self.rooms {
            if !column_contains(room, m) {
                continue;
            }
            let tighter = best
                .map(|current| volume(room) < volume(current))
                .unwrap_or(true);
            if tighter {
                best = Some(room);
            }
        }
        best
    }

    /// Sample the authored footprint. Points outside a room jacket return
    /// nothing, so the hill above a deep hall stays natural terrain.
    pub fn sample(&self, cfg: &WorldSampleConfig, m: Vec3, _height: i32) -> Option<BlockId> {
        if self.is_empty() || !self.contains(m) {
            return None;
        }
        if let Some(layout) = &self.v2 {
            // Generator 32 samples voxel centres.
            return layout.sample(cfg, m + Vec3::splat(cfg.voxel_meters() * 0.5), super::v2::Mech::default());
        }
        if let Some(room) = self.room_at(m) {
            return Some(dress(room, m));
        }
        if self.rooms.iter().any(|room| jacket(room, m)) {
            return Some(BlockId::Stone);
        }
        None
    }

    /// Stable test fixture, also used by the app's interaction tests. Never generated in a real world.
    pub fn fixture() -> Self {
        let room = RoomSpec {
            id: ObjectId::new(0, 0, 0).expect("fixture room id"),
            kind: RoomKind::Landing,
            name: "Fixture landing",
            x0: -10.0,
            x1: 10.0,
            z0: -10.0,
            z1: 10.0,
            floor: 30.0,
            headroom: 8.0,
            style: Style::Rootbound,
            damage: Damage::Intact,
        };
        let prop = Prop {
            id: ObjectId::new(0, 0, 1).expect("fixture prop id"),
            kind: PropKind::Marker,
            label: "Test marker",
            pos: Vec3::new(0.5, 31.0, 0.5),
            yaw: 0.0,
        };
        Self {
            anchor: Vec3::new(0.5, 30.0, 0.5),
            bounds: (Vec3::new(-12.0, 28.0, -12.0), Vec3::new(12.0, 40.0, 12.0)),
            rooms: vec![room],
            links: Vec::new(),
            props: vec![prop],
            v2: None,
        }
    }
}

fn volume(room: &RoomSpec) -> f32 {
    (room.x1 - room.x0) * (room.z1 - room.z0) * room.headroom
}

fn column_contains(room: &RoomSpec, m: Vec3) -> bool {
    m.x >= room.x0
        && m.x <= room.x1
        && m.z >= room.z0
        && m.z <= room.z1
        && m.y >= room.floor - 1.0
        && m.y <= room.floor + room.headroom + 1.0
}

fn jacket(room: &RoomSpec, m: Vec3) -> bool {
    let pad = 2.0;
    m.x >= room.x0 - pad
        && m.x <= room.x1 + pad
        && m.z >= room.z0 - pad
        && m.z <= room.z1 + pad
        && m.y >= room.floor - 2.0
        && m.y <= room.floor + room.headroom + 2.0
}

fn floor_block(room: &RoomSpec) -> BlockId {
    match room.kind {
        RoomKind::Garden => BlockId::FungalSoil,
        RoomKind::Foundry => BlockId::Basalt,
        RoomKind::Choir => BlockId::Ice,
        RoomKind::Village => BlockId::Planks,
        RoomKind::Cistern | RoomKind::Bath => BlockId::Sand,
        RoomKind::Kitchen => BlockId::Cobblestone,
        _ => match room.style {
            Style::Canal | Style::Sea => BlockId::Limestone,
            Style::Foundry => BlockId::Basalt,
            Style::Ice => BlockId::Ice,
            Style::Fungal => BlockId::FungalSoil,
            _ => BlockId::StoneBricks,
        },
    }
}

fn dress(room: &RoomSpec, m: Vec3) -> BlockId {
    let ceiling = room.floor + room.headroom;
    if m.y < room.floor {
        return floor_block(room);
    }
    if m.y >= ceiling {
        return match room.style {
            Style::Cliff => BlockId::Limestone,
            Style::Foundry => BlockId::Basalt,
            _ => BlockId::StoneBricks,
        };
    }
    let lx = m.x - room.x0;
    let lz = m.z - room.z0;
    let ly = m.y - room.floor;
    match room.kind {
        RoomKind::Cistern | RoomKind::Bath if ly < 1.6 => BlockId::Water,
        RoomKind::Foundry if ly < 1.0 && lx > 3.0 && lx < 7.0 && lz > 3.0 && lz < 7.0 => {
            BlockId::Lava
        }
        RoomKind::Garden if ly < 1.0 && ((m.x as i32).wrapping_mul(3) + m.z as i32) % 7 == 0 => {
            BlockId::GlowCap
        }
        RoomKind::Choir if ly < 3.0 && (lx < 1.2 || lz < 1.2) => BlockId::Ice,
        RoomKind::Chapel if ly < 4.0 && lx < 1.2 && lz < 1.2 => BlockId::Log,
        RoomKind::Kitchen if ly < 1.0 && lx > 2.0 && lx < 4.0 && lz > 2.0 && lz < 4.0 => {
            BlockId::Cobblestone
        }
        RoomKind::Village if ly < 3.0 && lx > 2.4 && lx < 6.0 && lz > 2.0 && lz < 6.0 => {
            BlockId::Planks
        }
        RoomKind::Chasm if lx > 8.0 && lx < 10.0 && lz > 4.0 && lz < 6.0 => BlockId::Water,
        RoomKind::Bell if ly < 2.0 && (lx < 1.0 || lz < 1.0) => BlockId::Cobblestone,
        RoomKind::Observatory if ly < 2.0 && lx > 3.0 && lx < 5.0 && lz > 3.0 && lz < 5.0 => {
            BlockId::Crystal
        }
        RoomKind::Landing if ly < 1.2 && lx > 1.0 && lx < 1.6 && lz > 1.0 && lz < 1.6 => {
            BlockId::Torch
        }
        RoomKind::Vault if room.damage == Damage::Collapsed && ly < 2.0 && lx < 3.0 => {
            BlockId::Gravel
        }
        _ => BlockId::Air,
    }
}

fn surface_meters(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let (height, _) =
        super::super::distant_surface(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    (height - 1.0) * cfg.voxel_meters()
}

struct Draft {
    district: District,
    room: u16,
    kind: RoomKind,
    name: &'static str,
    style: Style,
    damage: Damage,
    x0: f32,
    z0: f32,
    x1: f32,
    z1: f32,
    floor: f32,
    headroom: f32,
    prop: Option<(&'static str, PropKind, f32, f32)>,
}

fn authored(cfg: &WorldSampleConfig) -> Plan {
    let anchor_x = 160.0;
    let anchor_z = 80.0;
    let surface = surface_meters(cfg, anchor_x, anchor_z);
    let exit_surface = surface_meters(cfg, anchor_x + 108.0, anchor_z - 36.0);
    let landing = surface - 42.0;
    let chasm = landing - 18.0;
    let borough = chasm - 18.0;
    let drafts = drafts(landing, chasm, borough, surface, exit_surface);
    let mut rooms = Vec::with_capacity(drafts.len());
    let mut props = Vec::new();
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for draft in drafts {
        let room = RoomSpec {
            id: ObjectId::new(draft.district.index(), draft.room, 0).expect("room id"),
            kind: draft.kind,
            name: draft.name,
            x0: anchor_x + draft.x0,
            x1: anchor_x + draft.x1,
            z0: anchor_z + draft.z0,
            z1: anchor_z + draft.z1,
            floor: draft.floor,
            headroom: draft.headroom,
            style: draft.style,
            damage: draft.damage,
        };
        min.x = min.x.min(room.x0);
        min.y = min.y.min(room.floor - 2.0);
        min.z = min.z.min(room.z0);
        max.x = max.x.max(room.x1);
        max.y = max.y.max(room.floor + room.headroom + 2.0);
        max.z = max.z.max(room.z1);
        if let Some((label, kind, ox, oz)) = draft.prop {
            let stand = match draft.kind {
                RoomKind::Cistern | RoomKind::Bath => 2.2,
                _ => 1.2,
            };
            props.push(Prop {
                id: ObjectId::new(draft.district.index(), draft.room, 1).expect("prop id"),
                kind,
                label,
                pos: Vec3::new(room.x0 + ox, room.floor + stand, room.z0 + oz),
                yaw: 0.0,
            });
        }
        rooms.push(room);
    }
    let mut links = Vec::new();
    for (i, left) in rooms.iter().enumerate() {
        for right in rooms.iter().skip(i + 1) {
            if overlaps(left, right) {
                links.push(Link {
                    from: left.id,
                    to: right.id,
                    kind: if matches!(left.kind, RoomKind::Chasm | RoomKind::Observatory)
                        || matches!(right.kind, RoomKind::Chasm | RoomKind::Observatory)
                    {
                        LinkKind::Sightline
                    } else if matches!(left.kind, RoomKind::Cistern | RoomKind::Bath)
                        || matches!(right.kind, RoomKind::Cistern | RoomKind::Bath)
                    {
                        LinkKind::Water
                    } else if matches!(left.kind, RoomKind::Lift | RoomKind::Bell)
                        || matches!(right.kind, RoomKind::Lift | RoomKind::Bell)
                    {
                        LinkKind::Freight
                    } else {
                        LinkKind::Passage
                    },
                });
            }
        }
    }
    Plan {
        anchor: Vec3::new(anchor_x + 2.0, landing + 1.0, anchor_z + 2.0),
        bounds: (min, max),
        rooms,
        links,
        props,
        v2: None,
    }
}

fn overlaps(a: &RoomSpec, b: &RoomSpec) -> bool {
    a.x0 <= b.x1
        && b.x0 <= a.x1
        && a.z0 <= b.z1
        && b.z0 <= a.z1
        && a.floor - 1.0 <= b.floor + b.headroom
        && b.floor - 1.0 <= a.floor + a.headroom
}

fn drafts(landing: f32, chasm: f32, borough: f32, surface: f32, exit_surface: f32) -> Vec<Draft> {
    let well_head = (surface - 1.0 - landing).max(8.0);
    let lift_head = (chasm - borough + 6.0).max(8.0);
    let climb = (exit_surface - 2.0 - (chasm + 10.0)).max(6.0);
    vec![
        d(
            District::RootboundHalls,
            1,
            RoomKind::WellHead,
            "Ruined well",
            Style::Rootbound,
            Damage::Collapsed,
            0.0,
            0.0,
            4.0,
            4.0,
            landing,
            well_head,
            Some(("Blue lamp", PropKind::Marker, 2.0, 2.0)),
        ),
        d(
            District::RootboundHalls,
            2,
            RoomKind::Landing,
            "First landing",
            Style::Rootbound,
            Damage::Intact,
            -6.0,
            -12.0,
            10.0,
            2.0,
            landing,
            5.0,
            Some((
                "For those who carry the burden, the road descends.",
                PropKind::Marker,
                3.0,
                3.0,
            )),
        ),
        d(
            District::RootboundHalls,
            3,
            RoomKind::CounterweightHall,
            "Counterweight hall",
            Style::Cliff,
            Damage::Intact,
            8.0,
            -12.0,
            22.0,
            2.0,
            landing,
            5.0,
            Some(("Counterweight Ring", PropKind::Plinth, 4.0, 4.0)),
        ),
        d(
            District::RootboundHalls,
            4,
            RoomKind::Passage,
            "Broken stair",
            Style::Cliff,
            Damage::Collapsed,
            20.0,
            -10.0,
            32.0,
            0.0,
            landing,
            6.0,
            Some(("Freight platform", PropKind::Cradle, 3.0, 3.0)),
        ),
        d(
            District::RootboundHalls,
            5,
            RoomKind::Passage,
            "Service corridor",
            Style::Rootbound,
            Damage::Intact,
            8.0,
            0.0,
            20.0,
            16.0,
            landing,
            4.0,
            Some(("Corridor keeper", PropKind::Marker, 3.0, 4.0)),
        ),
        d(
            District::RootboundHalls,
            6,
            RoomKind::Passage,
            "Door behind the first camp",
            Style::Rootbound,
            Damage::Intact,
            -14.0,
            -12.0,
            -4.0,
            -2.0,
            landing,
            4.0,
            Some(("The back of the landing door", PropKind::Marker, 3.0, 3.0)),
        ),
        d(
            District::RootboundHalls,
            7,
            RoomKind::Kitchen,
            "Pilgrims' kitchen",
            Style::Rootbound,
            Damage::Intact,
            -6.0,
            -22.0,
            6.0,
            -12.0,
            landing,
            4.0,
            Some(("Hearth", PropKind::Marker, 7.0, 6.0)),
        ),
        d(
            District::RootboundHalls,
            8,
            RoomKind::Chapel,
            "Chapel growing through the prison",
            Style::Rootbound,
            Damage::Overgrown,
            20.0,
            -22.0,
            34.0,
            -10.0,
            landing,
            5.0,
            Some(("Confiscated locker", PropKind::Plinth, 4.0, 4.0)),
        ),
        d(
            District::CrownChasm,
            1,
            RoomKind::Chasm,
            "Crown Chasm",
            Style::Cliff,
            Damage::Collapsed,
            30.0,
            -40.0,
            78.0,
            -8.0,
            chasm,
            22.0,
            Some(("Waterfall door, still shut", PropKind::Marker, 8.0, 6.0)),
        ),
        d(
            District::CrownChasm,
            2,
            RoomKind::Village,
            "Bridge village",
            Style::Cliff,
            Damage::Intact,
            40.0,
            -30.0,
            58.0,
            -16.0,
            chasm + 8.0,
            5.0,
            Some(("Repair the water lift", PropKind::Cradle, 1.5, 1.5)),
        ),
        d(
            District::CrownChasm,
            3,
            RoomKind::Passage,
            "Hall of patient bells",
            Style::Cliff,
            Damage::Intact,
            58.0,
            -18.0,
            74.0,
            -8.0,
            chasm + 6.0,
            5.0,
            Some(("Bellkeeper's Gloves", PropKind::Plinth, 4.0, 3.0)),
        ),
        d(
            District::CrownChasm,
            4,
            RoomKind::Passage,
            "Freight cradle",
            Style::Cliff,
            Damage::Collapsed,
            58.0,
            -8.0,
            74.0,
            4.0,
            chasm + 4.0,
            5.0,
            Some(("Receiving platform", PropKind::Cradle, 4.0, 4.0)),
        ),
        d(
            District::CrownChasm,
            5,
            RoomKind::Bell,
            "Bellkeeper's crown",
            Style::Cliff,
            Damage::Intact,
            76.0,
            -32.0,
            94.0,
            -14.0,
            chasm + 4.0,
            12.0,
            Some(("Bell clapper", PropKind::BellClapper, 4.0, 4.0)),
        ),
        d(
            District::CrownChasm,
            6,
            RoomKind::Observatory,
            "Clifftop observatory",
            Style::Archive,
            Damage::Intact,
            94.0,
            -42.0,
            108.0,
            -28.0,
            chasm + 10.0,
            6.0,
            Some(("Observatory lens", PropKind::ObservatoryLens, 5.0, 5.0)),
        ),
        d(
            District::CrownChasm,
            7,
            RoomKind::Lift,
            "Giant's listening stair",
            Style::Cliff,
            Damage::Intact,
            28.0,
            -16.0,
            36.0,
            -6.0,
            chasm,
            landing - chasm + 4.0,
            Some(("Hollow statue", PropKind::Marker, 3.0, 3.0)),
        ),
        d(
            District::CrownChasm,
            8,
            RoomKind::Passage,
            "Stone giant's pocket",
            Style::Cliff,
            Damage::Intact,
            60.0,
            -38.0,
            70.0,
            -28.0,
            chasm + 12.0,
            4.0,
            Some(("A letter and a tool", PropKind::Plinth, 3.0, 3.0)),
        ),
        d(
            District::CrownChasm,
            9,
            RoomKind::Lift,
            "Observatory stair",
            Style::Archive,
            Damage::Intact,
            100.0,
            -40.0,
            106.0,
            -34.0,
            chasm + 10.0,
            climb,
            Some(("Last window", PropKind::Marker, 2.0, 2.0)),
        ),
        d(
            District::DrownedBorough,
            1,
            RoomKind::Lift,
            "Chain lift",
            Style::Canal,
            Damage::Intact,
            46.0,
            -44.0,
            54.0,
            -36.0,
            borough,
            lift_head,
            Some(("Chain lift", PropKind::Winch, 3.0, 3.0)),
        ),
        d(
            District::DrownedBorough,
            2,
            RoomKind::Cistern,
            "Street beneath the air bell",
            Style::Canal,
            Damage::Flooded,
            34.0,
            -62.0,
            62.0,
            -44.0,
            borough,
            6.0,
            Some(("Jeweler's balcony", PropKind::Plinth, 6.0, 4.0)),
        ),
        d(
            District::DrownedBorough,
            3,
            RoomKind::Vault,
            "Flooded courthouse",
            Style::Canal,
            Damage::Flooded,
            62.0,
            -58.0,
            78.0,
            -44.0,
            borough + 2.0,
            5.0,
            Some(("Rimeglass Buckler", PropKind::Plinth, 4.0, 4.0)),
        ),
        d(
            District::DrownedBorough,
            4,
            RoomKind::Cistern,
            "Reservoir control",
            Style::Canal,
            Damage::Flooded,
            62.0,
            -74.0,
            80.0,
            -58.0,
            borough,
            5.0,
            Some(("Bypass sluice", PropKind::SluiceGate, 4.0, 4.0)),
        ),
        d(
            District::DrownedBorough,
            5,
            RoomKind::Refuge,
            "Ferry landing",
            Style::Canal,
            Damage::Intact,
            34.0,
            -74.0,
            48.0,
            -62.0,
            borough + 1.0,
            4.0,
            Some(("Ferryman", PropKind::Marker, 4.0, 3.0)),
        ),
        d(
            District::DrownedBorough,
            6,
            RoomKind::Passage,
            "Maintenance duct",
            Style::Canal,
            Damage::Intact,
            48.0,
            -62.0,
            56.0,
            -44.0,
            borough + 1.0,
            3.5,
            Some(("Duct mark", PropKind::Marker, 3.0, 4.0)),
        ),
        d(
            District::DrownedBorough,
            7,
            RoomKind::Bath,
            "Bathhouse of reverse flow",
            Style::Canal,
            Damage::Flooded,
            78.0,
            -58.0,
            92.0,
            -46.0,
            borough + 1.0,
            4.0,
            Some(("Flow marks", PropKind::Marker, 3.0, 3.0)),
        ),
        d(
            District::DrownedBorough,
            8,
            RoomKind::Passage,
            "Aqueduct inside a house",
            Style::Canal,
            Damage::Intact,
            22.0,
            -58.0,
            36.0,
            -46.0,
            borough + 2.0,
            4.0,
            Some(("Older channel", PropKind::Marker, 4.0, 3.0)),
        ),
        d(
            District::LanternGardens,
            1,
            RoomKind::Garden,
            "Lantern orchard mouth",
            Style::Fungal,
            Damage::Overgrown,
            8.0,
            14.0,
            26.0,
            30.0,
            landing - 2.0,
            6.0,
            Some(("Gardener", PropKind::Marker, 4.0, 4.0)),
        ),
        d(
            District::LanternGardens,
            2,
            RoomKind::Vault,
            "Seed bank",
            Style::Fungal,
            Damage::Intact,
            24.0,
            18.0,
            36.0,
            30.0,
            landing - 1.0,
            4.0,
            Some(("Living lock", PropKind::Marker, 3.0, 3.0)),
        ),
        d(
            District::AshenFoundry,
            1,
            RoomKind::Foundry,
            "Furnace mouth",
            Style::Foundry,
            Damage::Intact,
            72.0,
            0.0,
            88.0,
            14.0,
            chasm + 2.0,
            6.0,
            Some(("Cooling gate", PropKind::SluiceGate, 4.0, 4.0)),
        ),
        d(
            District::FrozenChoir,
            1,
            RoomKind::Choir,
            "Choir antechamber",
            Style::Ice,
            Damage::Intact,
            92.0,
            -18.0,
            108.0,
            -6.0,
            chasm + 4.0,
            6.0,
            Some(("Ice organ valve", PropKind::Marker, 4.0, 4.0)),
        ),
    ]
}

fn d(
    district: District,
    room: u16,
    kind: RoomKind,
    name: &'static str,
    style: Style,
    damage: Damage,
    x0: f32,
    z0: f32,
    x1: f32,
    z1: f32,
    floor: f32,
    headroom: f32,
    prop: Option<(&'static str, PropKind, f32, f32)>,
) -> Draft {
    Draft {
        district,
        room,
        kind,
        name,
        style,
        damage,
        x0,
        z0,
        x1,
        z1,
        floor,
        headroom,
        prop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use crate::WorldType;

    fn cfg(version: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: version,
            seed: 17,
            world_type: WorldType::Hills,
            voxel_scale: scale,
        }
    }

    #[test]
    fn empty_plan_is_inert() {
        let plan = Plan::empty();
        assert!(plan.is_empty());
        assert!(!plan.contains(Vec3::ZERO));
        assert!(!plan.reserved(Vec3::ZERO));
        assert_eq!(plan.location(Vec3::ZERO), None);
        let sample = plan.sample(
            &cfg(super::super::FIRST_VERSION, VoxelScalePreset::OneMeter),
            Vec3::ZERO,
            0,
        );
        assert_eq!(sample, None);
    }

    #[test]
    fn for_config_stays_empty_before_the_undercrown_and_then_authors_varied_rooms() {
        for version in [1, 18] {
            assert!(Plan::for_config(&cfg(version, VoxelScalePreset::OneMeter)).is_empty());
        }
        let plan = Plan::for_config(&cfg(
            super::super::FIRST_VERSION,
            VoxelScalePreset::OneMeter,
        ));
        assert!(plan.rooms.len() >= 24);
        assert!(plan.props.len() >= 24);
        assert!(plan.links.len() >= 20);
        let mut kinds = std::collections::HashSet::new();
        let mut districts = std::collections::BTreeSet::new();
        let mut ids = std::collections::BTreeSet::new();
        for room in &plan.rooms {
            kinds.insert(std::mem::discriminant(&room.kind));
            districts.insert(room.id.district());
            assert!(ids.insert(room.id));
            assert_eq!(room.id.prop(), 0);
        }
        assert!(kinds.len() >= 12);
        assert!(districts.len() >= 5);
        for prop in &plan.props {
            assert!(ids.insert(prop.id));
            assert!(plan.room_at(prop.pos).is_some());
            assert_eq!(
                plan.sample(
                    &cfg(super::super::FIRST_VERSION, VoxelScalePreset::OneMeter),
                    prop.pos,
                    0
                ),
                Some(BlockId::Air)
            );
        }
        let mut seen = vec![false; plan.rooms.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(index) = stack.pop() {
            for (other, _) in plan.rooms.iter().enumerate() {
                if !seen[other] && overlaps(&plan.rooms[index], &plan.rooms[other]) {
                    seen[other] = true;
                    stack.push(other);
                }
            }
        }
        assert!(seen.iter().all(|connected| *connected));
        let sample_at = |kind: RoomKind, block: BlockId| {
            let room = plan.rooms.iter().find(|room| room.kind == kind).unwrap();
            let mut found = false;
            let mut y = room.floor + 0.5;
            while y < room.floor + room.headroom.min(4.0) {
                let mut z = room.z0 + 0.5;
                while z < room.z1 {
                    let mut x = room.x0 + 0.5;
                    while x < room.x1 {
                        if plan.sample(
                            &cfg(super::super::FIRST_VERSION, VoxelScalePreset::OneMeter),
                            Vec3::new(x, y, z),
                            0,
                        ) == Some(block)
                        {
                            found = true;
                        }
                        x += 1.0;
                    }
                    z += 1.0;
                }
                y += 1.0;
            }
            assert!(found, "{kind:?} should contain {block:?}");
        };
        sample_at(RoomKind::Cistern, BlockId::Water);
        sample_at(RoomKind::Foundry, BlockId::Lava);
        sample_at(RoomKind::Garden, BlockId::GlowCap);
        sample_at(RoomKind::Choir, BlockId::Ice);
        sample_at(RoomKind::Chapel, BlockId::Log);
        let well = plan
            .rooms
            .iter()
            .find(|room| room.kind == RoomKind::WellHead)
            .unwrap();
        assert_eq!(
            plan.sample(
                &cfg(super::super::FIRST_VERSION, VoxelScalePreset::OneMeter),
                Vec3::new(
                    (well.x0 + well.x1) * 0.5,
                    well.floor + well.headroom - 1.0,
                    (well.z0 + well.z1) * 0.5
                ),
                0,
            ),
            Some(BlockId::Air)
        );
        assert_eq!(plan.location(plan.anchor), Some("Ruined well"));
    }

    #[test]
    fn fixture_is_deterministic_and_scale_stable() {
        let plan = Plan::fixture();
        let again = Plan::fixture();
        assert_eq!(plan.anchor, again.anchor);
        assert_eq!(plan.rooms.len(), again.rooms.len());
        assert_eq!(plan.props.len(), again.props.len());
        assert_eq!(plan.rooms[0].x0, again.rooms[0].x0);
        assert_eq!(plan.props[0].pos, again.props[0].pos);
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let cfg = cfg(super::super::FIRST_VERSION, scale);
            assert!(plan.contains(plan.props[0].pos));
            assert_eq!(plan.sample(&cfg, plan.props[0].pos, 0), Some(BlockId::Air));
        }
    }

    #[test]
    fn fixture_props_are_unique_and_inside_their_room() {
        let plan = Plan::fixture();
        let room = plan.rooms[0];
        let prop = plan.props[0];
        assert_eq!(prop.id.district(), room.id.district());
        assert_eq!(prop.id.room(), room.id.room());
        assert_ne!(prop.id.prop(), 0);
        assert!(prop.pos.x >= room.x0 && prop.pos.x <= room.x1);
        assert!(prop.pos.z >= room.z0 && prop.pos.z <= room.z1);
        assert!(prop.pos.y >= room.floor && prop.pos.y < room.floor + room.headroom);
    }
}
