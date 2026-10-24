//! Crown Chasm: a cavern 60 m across under the landing, a rock shelf where
//! the broken stair arrives, a bridge village on a pillar, the bell tower,
//! the waterfall, the listening stair back up, the observatory and its stair
//! to the surface, the hall of patient bells and the frozen choir.
use super::feature::Kind as F;
use super::layout::{Builder, hall, surface};
use super::{Mech, Shape};
use crate::WorldSampleConfig;
use crate::undercrown::plan::{Damage, District as D, PropKind, RoomKind as K, Style as S};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_2, PI};

pub(super) fn chasm(b: &mut Builder, cfg: &WorldSampleConfig, c: f32, l: f32, s: f32) {
    let v3 = Vec3::new;
    let _ = s;
    let chasm = b.room(D::CrownChasm, 1, K::Chasm, "Crown Chasm", S::Cliff, Damage::Collapsed, v3(40.0, c, -42.0), v3(100.0, c + 30.0, -6.0));
    b.space(chasm, Shape::Cave { c: v3(70.0, c + 4.0, -24.0), rad: v3(30.0, 26.0, 18.0), floor: c }, S::Cliff, None, false);
    // The shelf the broken stair reaches, and a ramp down to the floor.
    b.feature((66.0, c - 1.0, -14.0), (78.0, c + 8.0, -5.0), F::Pillar { top: c + 8.0, style: S::Cliff });
    b.feature((66.0, c - 1.0, -24.0), (69.0, c + 8.0, -14.0), F::Ramp { along_x: false, y0: c, y1: c + 8.0, style: S::Cliff, needs: None });
    // Waterfall into a plunge pool.
    b.feature((74.0, c - 2.0, -44.0), (78.0, c + 28.0, -39.0), F::Waterfall { pool: c - 1.0 });
    b.prop(chasm, PropKind::Marker, "Waterfall door, still shut", 72.0, c + 1.2, -35.0, S::Cliff);

    // Bridge village on a pillar, a bridge to the shelf, a ramp to the floor.
    let village = b.room(D::CrownChasm, 2, K::Village, "Bridge village", S::Cliff, Damage::Intact, v3(46.0, c + 8.0, -30.0), v3(60.0, c + 13.0, -16.0));
    b.feature((48.0, c - 1.0, -32.0), (60.0, c + 8.0, -29.0), F::Ramp { along_x: true, y0: c + 8.0, y1: c, style: S::Cliff, needs: None });
    b.feature((59.0, c + 6.0, -17.0), (67.0, c + 10.0, -13.5), F::Bridge { y: c + 8.0, needs: None });
    b.feature((46.0, c - 1.0, -30.0), (60.0, c + 8.0, -16.0), F::Pillar { top: c + 8.0, style: S::Cliff });
    b.feature((46.0, c + 8.0, -28.0), (60.0, c + 13.0, -18.0), F::Huts { y: c + 8.0, seed: cfg.seed.wrapping_add(31) });
    b.prop(village, PropKind::Cradle, "Repair the water lift", 58.0, c + 9.2, -27.5, S::Cliff);
    // Repairing the water lift spans the village to the tower's middle door.
    b.feature((59.0, c + 6.0, -29.5), (83.5, c + 10.0, -26.5), F::Bridge { y: c + 8.0, needs: Some(Mech::BRIDGE) });

    // The bell tower: a door at the floor, one onto the repaired bridge, one
    // from the chamber to the giant's pocket.
    let bell = b.room(D::CrownChasm, 5, K::Bell, "Bellkeeper's crown", S::Cliff, Damage::Intact, v3(82.5, c, -33.5), v3(93.5, c + 18.0, -22.5));
    let (tx, tz) = (88.0, -28.0);
    b.feature((tx - 6.5, c, tz - 6.5), (tx + 6.5, c + 19.0, tz + 6.5), F::Tower {
        x: tx + b.v(0.0, 0.0, 0.0).x,
        z: tz + b.v(0.0, 0.0, 0.0).z,
        r: 5.5,
        y0: c,
        chamber: c + 12.0,
        top: c + 18.0,
        doors: [(PI, c), (PI, c + 8.0), (-FRAC_PI_2, c + 12.0)],
    });
    b.prop(bell, PropKind::BellClapper, "Bell clapper", tx, c + 13.2, tz + 2.5, S::Cliff);
    let pocket = b.room(D::CrownChasm, 8, K::Passage, "Stone giant's pocket", S::Cliff, Damage::Intact, v3(84.0, c + 12.0, -50.0), v3(92.0, c + 17.0, -44.0));
    b.space(pocket, hall(84.0, 92.0, -50.0, -44.0, c + 12.0, 4.0, 1.0), S::Cliff, None, false);
    b.feature((86.5, c + 10.0, -45.0), (89.5, c + 14.0, -33.0), F::Bridge { y: c + 12.0, needs: None });
    b.tunnel((88.0, c + 12.0, -35.0), (88.0, c + 12.0, -45.0), S::Cliff, None, false);
    b.prop(pocket, PropKind::Plinth, "A letter and a tool", 90.0, c + 13.2, -48.5, S::Cliff);

    // The listening stair: the way back up. Its middle flight fell; the bell
    // brings it back.
    let lift = b.room(D::CrownChasm, 7, K::Lift, "Giant's listening stair", S::Cliff, Damage::Intact, v3(40.5, c, 4.5), v3(47.5, l + 4.0, 11.5));
    b.space(lift, Shape::Shaft { x: 44.0, z: 8.0, r: 3.5, y0: c, y1: l + 4.0, gap: Some((c + 12.0, c + 20.0, Mech::BELL)), landings: [Some(l), None] }, S::Cliff, None, false);
    b.tunnel_via((50.0, c, -16.0), (48.5, c, -5.0), (44.0, c, 6.0), S::Cliff);
    b.tunnel_via((42.0, l, 9.0), (32.0, l, 16.5), (22.0, l, 15.0), S::Rootbound);
    b.prop(lift, PropKind::Marker, "Hollow statue", 44.0, c + 1.2, 8.0, S::Cliff);

    // East wall: hall of patient bells, the frozen choir beyond it.
    let bells = b.room(D::CrownChasm, 3, K::Passage, "Hall of patient bells", S::Cliff, Damage::Intact, v3(104.0, c + 6.0, -22.0), v3(118.0, c + 14.0, -10.0));
    b.space(bells, hall(104.0, 118.0, -22.0, -10.0, c + 6.0, 6.0, 2.0), S::Cliff, None, false);
    b.tunnel((96.0, c, -18.0), (105.0, c + 6.0, -16.0), S::Cliff, None, false);
    b.prop(bells, PropKind::Plinth, "Bellkeeper's Gloves", 115.0, c + 7.2, -12.0, S::Cliff);
    let choir = b.room(D::FrozenChoir, 1, K::Choir, "Choir antechamber", S::Ice, Damage::Intact, v3(118.0, c + 4.0, -27.0), v3(138.0, c + 14.0, -9.0));
    b.space(choir, Shape::Cave { c: v3(128.0, c + 5.0, -18.0), rad: v3(10.0, 8.0, 9.0), floor: c + 4.0 }, S::Ice, None, false);
    b.tunnel((117.0, c + 6.0, -16.0), (121.0, c + 4.0, -17.0), S::Ice, None, false);
    b.feature((119.0, c + 4.0, -26.0), (137.0, c + 13.0, -10.0), F::IcePipes { floor: c + 4.0, top: c + 13.0, seed: cfg.seed.wrapping_add(41) });
    b.prop(choir, PropKind::Marker, "Ice organ valve", 128.0, c + 6.2, -18.0, S::Ice);

    // South: the freight cradle.
    let freight = b.room(D::CrownChasm, 4, K::Passage, "Freight cradle", S::Cliff, Damage::Collapsed, v3(60.0, c + 4.0, -56.0), v3(72.0, c + 10.0, -46.0));
    b.space(freight, hall(60.0, 72.0, -56.0, -46.0, c + 4.0, 5.0, 1.0), S::Cliff, None, false);
    b.tunnel((66.0, c, -38.0), (66.0, c + 4.0, -47.0), S::Cliff, None, false);
    b.prop(freight, PropKind::Cradle, "Receiving platform", 69.0, c + 5.2, -53.0, S::Cliff);

    // Observatory on the east cliff, its stair from the floor to the surface.
    let (ox, oz) = (106.0, -36.0);
    let exit = surface(cfg, b.v(ox, 0.0, oz).x, b.v(ox, 0.0, oz).z);
    let ostair = b.room(D::CrownChasm, 9, K::Lift, "Observatory stair", S::Archive, Damage::Intact, v3(102.5, c, -39.5), v3(109.5, exit + 1.0, -32.5));
    b.space(ostair, Shape::Shaft { x: ox, z: oz, r: 3.5, y0: c, y1: exit + 1.0, gap: None, landings: [Some(c + 20.0), None] }, S::Archive, None, false);
    b.tunnel((95.5, c, -28.5), (105.0, c, -35.0), S::Cliff, None, false);
    let obs = b.room(D::CrownChasm, 6, K::Observatory, "Clifftop observatory", S::Archive, Damage::Intact, v3(108.0, c + 20.0, -42.0), v3(120.0, c + 30.0, -30.0));
    b.space(obs, Shape::Round { x: 114.0, z: -36.0, r: 6.0, floor: c + 20.0, h: 6.0, dome: 4.0 }, S::Archive, None, false);
    b.tunnel((108.0, c + 20.0, -36.0), (110.0, c + 20.0, -36.0), S::Archive, None, false);
    b.prop(obs, PropKind::ObservatoryLens, "Observatory lens", 114.0, c + 21.2, -36.0, S::Archive);
    b.prop(ostair, PropKind::Marker, "Last window", 118.5, c + 21.2, -36.0, S::Archive);
}
