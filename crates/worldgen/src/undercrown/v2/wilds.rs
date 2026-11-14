//! Generator 291: the natural Undercrown round the authored rooms.
//!
//! - The Rift: a ravine west of the Rootbound Halls, from under the landing
//!   down to sea level, with a stream, a stone span at the landing level, a
//!   stair down its west wall and a balcony half way.
//! - The Hollow Sea: a lake cavern south of the borough. The Rift's stream
//!   runs into it; the ferry landing's dock looks over it from a height no
//!   swimmer climbs, so the borough still needs the chain lift.
//! - The sunken grove: a mushroom cavern under the halls, off the Rift floor.
//! - Caverns and the Rift wear bedded rock instead of masonry courses.
//!
//! Nothing here joins the Crown Chasm, so the chasm stays one-way until the
//! bell or the freight platform, as before.
use super::feature::Kind as F;
use super::layout::Builder;
use super::{Shape, Space, palette};
use crate::undercrown::plan::{Damage, District as D, PropKind, RoomKind as K, Style as S};
use crate::{BlockId as B, hash2, value_noise3};
use glam::Vec3;

/// First generator with the Rift, the Hollow Sea and the sunken grove.
pub const VERSION: u32 = 291;
/// Half width the Rift is authored at, metres.
const HALF: f32 = 6.0;

pub(super) fn author(b: &mut Builder, seed: u32, l: f32, c: f32, bo: f32) {
    let v3 = Vec3::new;
    // Sea level, the Rift's channel bed and its banks.
    let sea = bo - 3.0;
    let bed = bo - 4.0;
    let bank = bed + 2.0;
    let top = l + 8.0;

    // The Rift, north to south, breaking into the sea at its south end.
    let rift = b.room(D::HollowSea, 1, K::Chasm, "Rift", S::Cliff, Damage::Collapsed, v3(-58.0, bed, -142.0), v3(-20.0, top, 42.0));
    b.space(rift, Shape::Rift { a: v3(-46.0, bed, 34.0), b: v3(-28.0, bed, -136.0), half: HALF, top }, S::Cliff, Some((sea, None)), false);
    // The spring at its head.
    b.feature((-47.5, bed - 1.0, 33.0), (-44.5, top - 1.0, 36.0), F::Waterfall { pool: bed });

    // From the door behind the first camp: a passage whose floor carries on
    // across the Rift as a stone span, to the stair in the west wall.
    b.tunnel((-21.0, l, -5.0), (-64.0, l, -5.0), S::Cliff, None, false);
    let stair = b.room(D::HollowSea, 2, K::Lift, "Rift stair", S::Cliff, Damage::Intact, v3(-70.0, bank, -9.0), v3(-62.0, l + 4.0, -1.0));
    b.space(stair, Shape::Shaft { x: -66.0, z: -5.0, r: 3.5, y0: bank, y1: l + 4.0, gap: None, landings: [Some(l), Some(c + 4.0)] }, S::Cliff, None, false);
    // Half way down, a balcony over the drop.
    b.tunnel((-64.0, c + 4.0, -5.0), (-45.0, c + 4.0, -5.0), S::Cliff, None, false);
    b.prop(stair, PropKind::Plinth, "Ledge cache", -46.5, c + 5.2, -5.0, S::Cliff);
    // The foot of the stair, onto the west bank.
    b.tunnel((-64.0, bank, -5.0), (-45.5, bank, -5.0), S::Cliff, None, false);

    // The sunken grove, east of the Rift floor and under the halls.
    let grove = b.room(D::LanternGardens, 3, K::Garden, "Sunken grove", S::Fungal, Damage::Overgrown, v3(-28.0, bank, -42.0), v3(24.0, bo + 20.0, 18.0));
    b.space(grove, Shape::Cave { c: v3(-2.0, bo + 6.0, -12.0), rad: v3(24.0, 14.0, 28.0), floor: bank }, S::Fungal, None, false);
    b.space(grove, Shape::Cave { c: v3(8.0, bo - 1.0, -22.0), rad: v3(8.0, 5.0, 7.0), floor: bo - 5.0 }, S::Fungal, Some((bank, None)), false);
    b.tunnel((-39.5, bank, -12.0), (-18.0, bank, -12.0), S::Fungal, None, false);
    b.feature((-22.0, bank, -34.0), (-2.0, bo + 12.0, 10.0), F::Mushrooms { floor: bank, seed: seed.wrapping_add(23) });
    b.feature((-2.0, bank, -12.0), (18.0, bo + 12.0, 10.0), F::Mushrooms { floor: bank, seed: seed.wrapping_add(24) });
    b.feature((12.0, bank, -34.0), (18.0, bo + 6.0, -28.0), F::Shards { floor: bank, seed: seed.wrapping_add(25) });
    b.prop(grove, PropKind::Plinth, "Seed cache", 4.0, bank + 2.2, -4.0, S::Fungal);

    // The Hollow Sea: two overlapping caverns over one water level.
    let hollow = b.room(D::HollowSea, 3, K::Cistern, "Hollow Sea", S::Sea, Damage::Flooded, v3(-50.0, bo - 7.0, -182.0), v3(62.0, bo + 26.0, -118.0));
    b.space(hollow, Shape::Cave { c: v3(-6.0, bo + 7.0, -152.0), rad: v3(44.0, 19.0, 30.0), floor: bo - 7.0 }, S::Sea, Some((sea, None)), false);
    b.space(hollow, Shape::Cave { c: v3(36.0, bo + 5.0, -146.0), rad: v3(26.0, 14.0, 22.0), floor: bo - 7.0 }, S::Sea, Some((sea, None)), false);
    // The ferry landing's dock: a passage climbing to the east cavern's
    // widest ring, where its floor runs out over the water as a jetty.
    b.tunnel((40.0, bo + 1.0, -99.0), (40.0, bo + 5.0, -132.0), S::Canal, None, false);
    // A fall from the roof.
    b.feature((4.0, bo - 4.0, -136.0), (8.0, bo + 22.0, -132.0), F::Waterfall { pool: sea - 1.0 });

    // The isle, with the cache, a stand of crystal and the ferry's wreck.
    let isle = b.room(D::HollowSea, 4, K::Refuge, "Sea isle", S::Sea, Damage::Intact, v3(-22.0, sea, -166.0), v3(2.0, bo + 6.0, -142.0));
    b.feature((-22.0, bo - 8.0, -166.0), (2.0, sea + 2.0, -142.0), F::Crag { top: sea + 2.0, flat: 0.55, style: S::Sea });
    b.feature((-8.0, sea + 2.0, -160.0), (-3.0, sea + 9.0, -148.0), F::Shards { floor: sea + 2.0, seed: seed.wrapping_add(26) });
    b.feature((0.0, sea - 1.0, -159.0), (15.0, sea + 6.0, -153.0), F::Wreck { y: sea - 1.0 });
    b.prop(isle, PropKind::Plinth, "Sea cache", -14.0, sea + 3.2, -154.0, S::Sea);
    // Stacks standing out of the water, and crystal on the sea bed.
    b.feature((-42.0, bo - 8.0, -152.0), (-33.0, bo + 10.0, -143.0), F::Crag { top: bo + 10.0, flat: 0.3, style: S::Sea });
    b.feature((12.0, bo - 8.0, -172.0), (22.0, bo + 13.0, -162.0), F::Crag { top: bo + 13.0, flat: 0.3, style: S::Sea });
    b.feature((46.0, bo - 8.0, -150.0), (54.0, bo + 6.0, -142.0), F::Crag { top: bo + 6.0, flat: 0.35, style: S::Sea });
    b.feature((16.0, bo - 7.0, -152.0), (32.0, sea, -138.0), F::Shards { floor: bo - 7.0, seed: seed.wrapping_add(27) });
    b.feature((-34.0, bo - 7.0, -176.0), (-20.0, sea, -166.0), F::Shards { floor: bo - 7.0, seed: seed.wrapping_add(28) });
}

/// Bedded rock: the style's wall stone with thin darker beds that dip and
/// break up, in metres so every voxel scale shares them.
pub(super) fn rock(style: S, m: Vec3) -> B {
    let [wall, alt, ..] = palette(style);
    let bed = m.y * 0.45 + value_noise3(m.x * 0.035, m.y * 0.02, m.z * 0.035, 7311) * 7.0;
    let dark = hash2(bed.floor() as i32, 17, 7313) > 0.8 && value_noise3(m.x * 0.09, m.y * 0.09, m.z * 0.09, 7315) > 0.3;
    if dark { alt } else { wall }
}

/// Crystal veins that light a cavern too wide for wall lamps.
fn vein(m: Vec3) -> bool {
    let ridge = (value_noise3(m.x * 0.08, m.y * 0.08, m.z * 0.08, 7317) - 0.5).abs();
    ridge < 0.03 && value_noise3(m.x * 0.03, m.y * 0.03, m.z * 0.03, 7319) > 0.5
}

/// Shell of a cavern or the Rift: sand at the sea's waterline and below,
/// gravel on the Rift's bed and banks, veined bedded rock elsewhere. `None`
/// for built rooms, which keep their masonry.
pub(super) fn natural(s: &Space, m: Vec3) -> Option<B> {
    if s.sealed {
        return None;
    }
    match s.shape {
        Shape::Cave { .. } => {
            if let (S::Sea, Some((level, _))) = (s.style, s.water)
                && m.y < level + 1.0
            {
                return Some(B::Sand);
            }
        }
        Shape::Rift { a, .. } => {
            if m.y < a.y + 2.0 {
                return Some(B::Gravel);
            }
        }
        _ => return None,
    }
    let lit = match s.style {
        S::Cliff => super::glow::seam(s.style, m) || (matches!(s.shape, Shape::Rift { .. }) && vein(m)),
        S::Sea => vein(m),
        _ => false,
    };
    Some(if lit { B::Crystal } else { rock(s.style, m) })
}
