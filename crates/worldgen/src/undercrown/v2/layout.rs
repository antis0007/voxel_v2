//! The authored Undercrown, generator 32. Offsets are metres from the anchor
//! (the well); heights hang off four levels: the surface S, the landing L,
//! the chasm floor C and the borough B. Room and prop ids are the generator
//! 19 save identities; only geometry and positions changed.
use super::feature::{Feature, Kind as F};
use super::furnish::Fit;
use super::index::Index;
use super::{Layout, Mech, Shape, Space, Tunnel};
use crate::undercrown::plan::{Damage, District, Prop, PropKind, RoomKind, RoomSpec, Style};
use crate::undercrown::state::ObjectId;
use crate::{BlockId as B, WorldSampleConfig};
use glam::Vec3;

pub(super) struct Builder {
    ax: f32,
    az: f32,
    rooms: Vec<RoomSpec>,
    props: Vec<Prop>,
    spaces: Vec<Space>,
    tunnels: Vec<Tunnel>,
    features: Vec<Feature>,
}

impl Builder {
    pub(super) fn v(&self, x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(self.ax + x, y, self.az + z)
    }

    /// Register a room for naming and save identity; returns its index.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn room(&mut self, district: District, n: u16, kind: RoomKind, name: &'static str, style: Style, damage: Damage, lo: Vec3, hi: Vec3) -> usize {
        let id = ObjectId::new(district.index(), n, 0).unwrap_or_else(|| unreachable!("authored id"));
        let (lo, hi) = (self.v(lo.x, lo.y, lo.z), self.v(hi.x, hi.y, hi.z));
        self.rooms.push(RoomSpec { id, kind, name, x0: lo.x, x1: hi.x, z0: lo.z, z1: hi.z, floor: lo.y, headroom: hi.y - lo.y, style, damage });
        self.rooms.len() - 1
    }

    pub(super) fn prop(&mut self, room: usize, kind: PropKind, label: &'static str, x: f32, y: f32, z: f32, style: Style) {
        let r = self.rooms[room].id;
        let id = ObjectId::new(r.district(), r.room(), 1).unwrap_or_else(|| unreachable!("authored prop id"));
        let pos = self.v(x, y, z);
        self.props.push(Prop { id, kind, label, pos, yaw: 0.0 });
        // A pedestal under the prop, its top 0.6 m below the prop point.
        let base = pos - Vec3::new(0.6, 1.8, 0.6);
        self.features.push(Feature::new(base, base + Vec3::new(1.2, 1.2, 1.2), F::Pedestal { style }));
    }

    pub(super) fn space(&mut self, room: usize, shape: Shape, style: Style, water: Option<(f32, Option<Mech>)>, sealed: bool) {
        let shape = match shape {
            Shape::Hall { x0, x1, z0, z1, floor, h, rise } => Shape::Hall { x0: x0 + self.ax, x1: x1 + self.ax, z0: z0 + self.az, z1: z1 + self.az, floor, h, rise },
            Shape::Round { x, z, r, floor, h, dome } => Shape::Round { x: x + self.ax, z: z + self.az, r, floor, h, dome },
            Shape::Cave { c, rad, floor } => Shape::Cave { c: self.v(c.x, c.y, c.z), rad, floor },
            Shape::Shaft { x, z, r, y0, y1, gap, landings } => Shape::Shaft { x: x + self.ax, z: z + self.az, r, y0, y1, gap, landings },
        };
        self.spaces.push(Space { room, shape, style, water, sealed });
    }

    pub(super) fn tunnel(&mut self, a: (f32, f32, f32), b: (f32, f32, f32), style: Style, gate: Option<(Mech, B)>, sealed: bool) {
        let (a, b) = (self.v(a.0, a.1, a.2), self.v(b.0, b.1, b.2));
        self.tunnels.push(Tunnel { a, b, half: 1.5, h: 3.4, style, gate, sealed });
    }

    /// Index of an authored room, for spaces that enlarge it.
    pub(super) fn room_of(&self, district: District, n: u16) -> Option<usize> {
        self.rooms.iter().position(|r| r.id.district() == district.index() && r.id.room() == n)
    }

    /// A passage bent once at `mid`: two straight runs.
    pub(super) fn tunnel_via(&mut self, a: (f32, f32, f32), mid: (f32, f32, f32), b: (f32, f32, f32), style: Style) {
        self.tunnel(a, mid, style, None, false);
        self.tunnel(mid, b, style, None, false);
    }

    pub(super) fn feature(&mut self, lo: (f32, f32, f32), hi: (f32, f32, f32), kind: F) {
        let (lo, hi) = (self.v(lo.0, lo.1, lo.2), self.v(hi.0, hi.1, hi.2));
        self.features.push(Feature::new(lo, hi, kind));
    }

    pub(super) fn fit(&mut self, lo: (f32, f32, f32), hi: (f32, f32, f32), fit: Fit) {
        self.feature(lo, hi, F::Fit(fit));
    }

    /// A column centred on (`x`, `z`) from `floor` to `top`.
    pub(super) fn column(&mut self, x: f32, z: f32, floor: f32, top: f32, style: Style) {
        self.fit((x - 1.1, floor, z - 1.1), (x + 1.1, top, z + 1.1), Fit::Column { floor, top, style });
    }

    /// A small fixture centred on (`x`, `z`): a lamp post or a brazier.
    pub(super) fn fixture(&mut self, x: f32, z: f32, floor: f32, top: f32, fit: Fit) {
        self.fit((x - 0.5, floor, z - 0.5), (x + 0.5, top, z + 0.5), fit);
    }
}

pub(super) fn hall(x0: f32, x1: f32, z0: f32, z1: f32, floor: f32, h: f32, rise: f32) -> Shape {
    Shape::Hall { x0, x1, z0, z1, floor, h, rise }
}

pub(super) fn surface(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    let (h, _) = crate::distant_surface(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z));
    (h - 1.0) * cfg.voxel_meters()
}

pub fn author(cfg: &WorldSampleConfig) -> (Vec<RoomSpec>, Vec<Prop>, Layout) {
    let (ax, az, ground, s) = super::site::site(cfg);
    build(cfg, ax, az, s, ground)
}

/// The whole complex with its anchor at (`ax`, `az`), its reference level
/// `s` (the landing is 30 m under it) and the ground at the well `ground`.
pub(super) fn build(cfg: &WorldSampleConfig, ax: f32, az: f32, s: f32, ground: f32) -> (Vec<RoomSpec>, Vec<Prop>, Layout) {
    use District as D;
    use RoomKind as K;
    use Style as S;
    let l = s - 30.0;
    let c = l - 34.0;
    let bo = c - 14.0;
    let mut b = Builder { ax, az, rooms: Vec::new(), props: Vec::new(), spaces: Vec::new(), tunnels: Vec::new(), features: Vec::new() };
    let v3 = Vec3::new;

    // Rootbound Halls, at the landing.
    let well = b.room(D::RootboundHalls, 1, K::WellHead, "Ruined well", S::Rootbound, Damage::Collapsed, v3(-3.0, l, -3.0), v3(3.0, ground + 1.0, 3.0));
    b.space(well, Shape::Shaft { x: 0.0, z: 0.0, r: 3.5, y0: l, y1: ground + 1.0, gap: None, landings: [None, None] }, S::Rootbound, None, false);
    let landing = b.room(D::RootboundHalls, 2, K::Landing, "First landing", S::Rootbound, Damage::Intact, v3(-8.0, l, -12.0), v3(10.0, l + 8.5, 6.0));
    b.space(landing, hall(-8.0, 10.0, -12.0, 6.0, l, 6.0, 2.5), S::Rootbound, None, false);
    b.prop(well, PropKind::Marker, "Blue lamp", 4.5, l + 1.2, 3.5, S::Rootbound);
    b.prop(landing, PropKind::Marker, "For those who carry the burden, the road descends.", -6.0, l + 1.2, -10.0, S::Rootbound);
    let hallw = b.room(D::RootboundHalls, 3, K::CounterweightHall, "Counterweight hall", S::Cliff, Damage::Intact, v3(16.0, l - 10.0, -18.0), v3(38.0, l + 13.0, 8.0));
    b.space(hallw, hall(16.0, 38.0, -18.0, 8.0, l - 10.0, 20.0, 3.0), S::Cliff, None, false);
    b.tunnel((8.0, l, -3.0), (18.0, l, -3.0), S::Rootbound, None, false);
    b.feature((16.0, l - 10.0, -18.0), (20.0, l, 8.0), F::Pillar { top: l, style: S::Cliff });
    b.feature((20.0, l - 10.0, -18.0), (36.0, l, -14.0), F::Ramp { along_x: true, y0: l, y1: l - 10.0, style: S::Cliff, needs: None });
    b.feature((22.0, l - 10.0, -10.0), (34.0, l + 13.0, 6.0), F::Counterweight { floor: l - 10.0, top: l + 12.0 });
    b.prop(hallw, PropKind::Plinth, "Counterweight Ring", 31.0, l - 8.8, 4.0, S::Cliff);
    // The broken stair: a collapse chamber. The first time down is a 4 m
    // drop; repairing the freight platform builds a ramp back up.
    let stair = b.room(D::RootboundHalls, 4, K::Passage, "Broken stair", S::Cliff, Damage::Collapsed, v3(54.0, l - 23.0, 0.0), v3(62.0, l - 14.0, 6.0));
    b.space(stair, hall(54.0, 62.0, 0.0, 6.0, l - 23.0, 9.0, 0.0), S::Cliff, None, false);
    b.tunnel((36.0, l - 10.0, 3.0), (54.5, l - 19.0, 3.0), S::Cliff, None, false);
    b.tunnel((61.5, l - 23.0, 3.0), (72.0, c + 8.0, 3.0), S::Cliff, None, false);
    b.tunnel((72.0, c + 8.0, 3.0), (72.0, c + 8.0, -9.0), S::Cliff, None, false);
    b.feature((54.0, l - 24.0, 1.5), (61.0, l - 18.0, 4.5), F::Ramp { along_x: true, y0: l - 19.0, y1: l - 23.0, style: S::Cliff, needs: Some(Mech::STAIR) });
    b.feature((57.0, l - 23.0, 4.5), (60.0, l - 21.5, 6.0), F::Pillar { top: l - 21.5, style: S::Cliff });
    b.prop(stair, PropKind::Cradle, "Freight platform", 51.0, l - 16.3, 3.0, S::Cliff);
    let service = b.room(D::RootboundHalls, 5, K::Passage, "Service corridor", S::Rootbound, Damage::Intact, v3(10.0, l, 10.0), v3(24.0, l + 5.5, 22.0));
    b.space(service, hall(10.0, 24.0, 10.0, 22.0, l, 4.5, 1.0), S::Rootbound, None, false);
    b.tunnel((6.0, l, 4.0), (13.0, l, 13.0), S::Rootbound, None, false);
    b.prop(service, PropKind::Marker, "Corridor keeper", 21.0, l + 1.2, 19.5, S::Rootbound);
    let door = b.room(D::RootboundHalls, 6, K::Passage, "Door behind the first camp", S::Rootbound, Damage::Intact, v3(-22.0, l, -10.0), v3(-12.0, l + 5.5, 0.0));
    b.space(door, hall(-22.0, -12.0, -10.0, 0.0, l, 4.5, 1.0), S::Rootbound, None, false);
    b.tunnel((-7.0, l, -4.0), (-14.0, l, -4.0), S::Rootbound, None, false);
    b.prop(door, PropKind::Marker, "The back of the landing door", -20.0, l + 1.2, -2.0, S::Rootbound);
    let kitchen = b.room(D::RootboundHalls, 7, K::Kitchen, "Pilgrims' kitchen", S::Rootbound, Damage::Intact, v3(-6.0, l, -28.0), v3(6.0, l + 5.5, -17.0));
    b.space(kitchen, hall(-6.0, 6.0, -28.0, -17.0, l, 4.5, 1.0), S::Rootbound, None, false);
    b.tunnel((0.0, l, -11.0), (0.0, l, -19.0), S::Rootbound, None, false);
    b.feature((2.0, l, -27.0), (5.0, l + 1.0, -24.0), F::Pillar { top: l + 1.0, style: S::Rootbound });
    b.prop(kitchen, PropKind::Marker, "Hearth", -4.0, l + 1.2, -26.0, S::Rootbound);
    let chapel = b.room(D::RootboundHalls, 8, K::Chapel, "Chapel growing through the prison", S::Rootbound, Damage::Overgrown, v3(-28.0, l - 3.0, -32.0), v3(-12.0, l + 6.5, -16.0));
    b.space(chapel, hall(-28.0, -12.0, -32.0, -16.0, l - 3.0, 7.0, 2.5), S::Rootbound, None, false);
    b.tunnel_via((-17.0, l, -9.0), (-15.5, l - 1.5, -13.5), (-18.0, l - 3.0, -18.0), S::Rootbound);
    b.feature((-28.0, l - 3.0, -32.0), (-12.0, l + 6.5, -16.0), F::Mushrooms { floor: l - 3.0, seed: cfg.seed.wrapping_add(21) });
    b.prop(chapel, PropKind::Plinth, "Confiscated locker", -14.0, l - 1.8, -30.0, S::Rootbound);

    // Lantern Gardens, just under the landing.
    let orchard = b.room(D::LanternGardens, 1, K::Garden, "Lantern orchard mouth", S::Fungal, Damage::Overgrown, v3(5.0, l - 2.0, 27.0), v3(31.0, l + 10.0, 49.0));
    b.space(orchard, Shape::Cave { c: v3(18.0, l + 1.0, 38.0), rad: v3(13.0, 9.0, 11.0), floor: l - 2.0 }, S::Fungal, None, false);
    b.tunnel((17.0, l, 21.0), (18.0, l - 2.0, 30.0), S::Fungal, None, false);
    b.feature((8.0, l - 2.0, 30.0), (29.0, l + 9.0, 46.0), F::Mushrooms { floor: l - 2.0, seed: cfg.seed.wrapping_add(22) });
    b.prop(orchard, PropKind::Marker, "Gardener", 13.0, l + 0.2, 36.0, S::Fungal);
    let seeds = b.room(D::LanternGardens, 2, K::Vault, "Seed bank", S::Fungal, Damage::Intact, v3(32.0, l - 1.0, 31.0), v3(42.0, l + 4.5, 41.0));
    b.space(seeds, hall(32.0, 42.0, 31.0, 41.0, l - 1.0, 4.5, 1.0), S::Fungal, None, false);
    b.tunnel((27.0, l - 2.0, 37.0), (33.0, l - 1.0, 36.0), S::Fungal, None, false);
    b.feature((32.0, l - 1.0, 31.0), (42.0, l + 3.0, 41.0), F::Shelves { floor: l - 1.0, block: B::GlowCap });
    b.prop(seeds, PropKind::Marker, "Living lock", 40.5, l + 0.2, 33.0, S::Fungal);

    super::layout_chasm::chasm(&mut b, cfg, c, l, s);
    super::layout_deep::borough(&mut b, bo, c, cfg.seed);
    super::layout_deep::foundry(&mut b, c);
    super::layout_dress::rootbound(&mut b, l);
    super::layout_dress::gardens(&mut b, l, cfg.seed);
    super::layout_dress::chasm(&mut b, c);
    super::layout_dress::borough(&mut b, bo);
    super::layout_dress::foundry(&mut b, c);

    let Builder { rooms, props, spaces, tunnels, features, .. } = b;
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for r in &rooms {
        min = min.min(Vec3::new(r.x0, r.floor, r.z0));
        max = max.max(Vec3::new(r.x1, r.floor + r.headroom, r.z1));
    }
    for t in &tunnels {
        let (lo, hi) = t.bounds();
        min = min.min(lo);
        max = max.max(hi);
    }
    for sp in &spaces {
        let (lo, hi) = sp.bounds();
        min = min.min(lo);
        max = max.max(hi);
    }
    for f in &features {
        min = min.min(f.lo);
        max = max.max(f.hi);
    }
    let (min, max) = (min - 3.0, max + 3.0);
    let feature_boxes: Vec<_> = features.iter().map(|f| f.bounds()).collect();
    let space_boxes: Vec<_> = spaces.iter().map(|sp| sp.bounds()).collect();
    let tunnel_boxes: Vec<_> = tunnels.iter().map(|t| t.bounds()).collect();
    let index = Index::build(min, max, &feature_boxes, &space_boxes, &tunnel_boxes);
    let clear = props.iter().map(|p| p.pos).collect();
    (rooms, props, Layout { anchor: Vec3::new(ax, l + 1.0, az), spaces, tunnels, features, min, max, clear, index })
}

