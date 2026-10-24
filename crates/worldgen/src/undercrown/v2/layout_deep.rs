//! Below the chasm: the Drowned Borough (a flooded vaulted street with house
//! houses, the courthouse the sluice can drain, the reservoir, the ferry,
//! the duct over the water, the bathhouse, the aqueduct house) and, east of
//! the chasm, the Ashen Foundry's lava channel and the sealed Glass Archive.
use super::feature::Kind as F;
use super::furnish::Fit;
use super::layout::{Builder, hall};
use super::{Mech, Shape};
use crate::BlockId as B;
use crate::undercrown::plan::{Damage, District as D, PropKind, RoomKind as K, Style as S};
use glam::Vec3;

pub(super) fn borough(b: &mut Builder, bo: f32, c: f32, seed: u32) {
    let v3 = Vec3::new;
    // The chain lift from the chasm floor; its foot is barred until wound.
    let lift = b.room(D::DrownedBorough, 1, K::Lift, "Chain lift", S::Canal, Damage::Intact, v3(53.0, bo, -55.0), v3(59.0, c + 5.0, -49.0));
    b.space(lift, Shape::Shaft { x: 56.0, z: -52.0, r: 3.5, y0: bo, y1: c + 5.0, gap: None, landings: [Some(c), None] }, S::Canal, None, false);
    b.tunnel((56.0, c, -37.0), (56.0, c, -50.0), S::Cliff, None, false);
    b.prop(lift, PropKind::Winch, "Chain lift", 57.5, c + 1.2, -46.0, S::Canal);
    b.tunnel((56.0, bo, -54.0), (56.0, bo, -61.0), S::Canal, Some((Mech::WINCH, B::SilverBlock)), false);

    let street = b.room(D::DrownedBorough, 2, K::Cistern, "Street beneath the air bell", S::Canal, Damage::Flooded, v3(40.0, bo, -82.0), v3(86.0, bo + 14.0, -60.0));
    b.space(street, hall(40.0, 86.0, -82.0, -60.0, bo, 10.0, 4.0), S::Canal, Some((bo + 1.0, None)), false);
    // The houses: the reservoir and ferry passages run through two of them.
    b.fit((40.0, bo, -82.0), (86.0, bo + 11.0, -76.0), Fit::Houses { floor: bo, style: S::Canal, seed: seed.wrapping_add(51), passages: [3.5, 31.5] });
    // An outside stair to a balcony over the water.
    b.feature((61.0, bo - 1.0, -77.0), (68.0, bo + 3.4, -75.0), F::Ramp { along_x: true, y0: bo, y1: bo + 3.4, style: S::Canal, needs: None });
    b.feature((68.0, bo - 1.0, -77.0), (71.0, bo + 3.4, -74.0), F::Pillar { top: bo + 3.4, style: S::Canal });
    b.prop(street, PropKind::Plinth, "Jeweler's balcony", 69.5, bo + 4.6, -75.5, S::Canal);

    let court = b.room(D::DrownedBorough, 3, K::Vault, "Flooded courthouse", S::Canal, Damage::Flooded, v3(90.0, bo - 2.0, -80.0), v3(104.0, bo + 7.0, -64.0));
    b.space(court, hall(90.0, 104.0, -80.0, -64.0, bo - 2.0, 7.0, 2.0), S::Canal, Some((bo + 3.5, Some(Mech::SLUICE))), false);
    b.tunnel((85.0, bo, -72.0), (91.0, bo - 2.0, -72.0), S::Canal, None, false);
    b.prop(court, PropKind::Plinth, "Rimeglass Buckler", 100.0, bo - 0.8, -67.0, S::Canal);

    let reservoir = b.room(D::DrownedBorough, 4, K::Cistern, "Reservoir control", S::Canal, Damage::Flooded, v3(64.0, bo, -100.0), v3(80.0, bo + 8.0, -86.0));
    b.space(reservoir, hall(64.0, 80.0, -100.0, -86.0, bo, 6.0, 2.0), S::Canal, Some((bo + 0.8, None)), false);
    b.tunnel((71.5, bo, -78.0), (72.0, bo, -87.0), S::Canal, None, false);
    b.prop(reservoir, PropKind::SluiceGate, "Bypass sluice", 76.0, bo + 1.2, -96.0, S::Canal);

    let ferry = b.room(D::DrownedBorough, 5, K::Refuge, "Ferry landing", S::Canal, Damage::Intact, v3(34.0, bo + 1.0, -100.0), v3(46.0, bo + 6.5, -88.0));
    b.space(ferry, hall(34.0, 46.0, -100.0, -88.0, bo + 1.0, 4.5, 1.0), S::Canal, None, false);
    b.tunnel((43.5, bo, -78.0), (41.0, bo + 1.0, -89.0), S::Canal, None, false);
    b.prop(ferry, PropKind::Marker, "Ferryman", 37.0, bo + 2.2, -97.0, S::Canal);

    // The duct runs above the flood line from the street to the courthouse.
    let duct = b.room(D::DrownedBorough, 6, K::Passage, "Maintenance duct", S::Canal, Damage::Intact, v3(86.0, bo + 4.0, -62.0), v3(90.0, bo + 7.0, -56.0));
    b.space(duct, hall(86.0, 90.0, -62.0, -56.0, bo + 4.0, 3.0, 0.0), S::Canal, None, false);
    b.feature((78.0, bo - 1.0, -63.0), (86.0, bo + 4.0, -60.0), F::Ramp { along_x: true, y0: bo, y1: bo + 4.0, style: S::Canal, needs: None });
    b.tunnel((88.0, bo + 4.0, -61.0), (95.0, bo + 4.0, -66.0), S::Canal, None, false);
    b.prop(duct, PropKind::Marker, "Duct mark", 88.0, bo + 5.2, -57.0, S::Canal);

    let bath = b.room(D::DrownedBorough, 7, K::Bath, "Bathhouse of reverse flow", S::Canal, Damage::Flooded, v3(110.0, bo + 1.0, -78.0), v3(122.0, bo + 8.0, -66.0));
    b.space(bath, hall(110.0, 122.0, -78.0, -66.0, bo + 1.0, 5.0, 2.0), S::Canal, Some((bo + 2.0, None)), false);
    b.tunnel((103.5, bo - 2.0, -72.0), (111.0, bo + 1.0, -72.0), S::Canal, None, false);
    b.prop(bath, PropKind::Marker, "Flow marks", 119.0, bo + 3.2, -75.0, S::Canal);

    let aqueduct = b.room(D::DrownedBorough, 8, K::Passage, "Aqueduct inside a house", S::Canal, Damage::Intact, v3(24.0, bo + 2.0, -78.0), v3(38.0, bo + 8.0, -66.0));
    b.space(aqueduct, hall(24.0, 38.0, -78.0, -66.0, bo + 2.0, 5.0, 1.0), S::Canal, None, false);
    b.tunnel((41.0, bo, -70.0), (37.0, bo + 2.0, -70.0), S::Canal, None, false);
    b.feature((24.0, bo + 1.0, -73.0), (38.0, bo + 3.0, -71.0), F::Channel { y: bo + 3.0 });
    b.prop(aqueduct, PropKind::Marker, "Older channel", 27.0, bo + 3.2, -76.0, S::Canal);
}

pub(super) fn foundry(b: &mut Builder, c: f32) {
    let v3 = Vec3::new;
    let furnace = b.room(D::AshenFoundry, 1, K::Foundry, "Furnace mouth", S::Foundry, Damage::Intact, v3(108.0, c + 2.0, -6.0), v3(128.0, c + 13.0, 12.0));
    b.space(furnace, hall(108.0, 128.0, -6.0, 12.0, c + 2.0, 9.0, 2.0), S::Foundry, None, false);
    b.tunnel_via((92.0, c, -16.0), (99.0, c + 1.0, -5.5), (109.0, c + 2.0, -1.0), S::Foundry);
    // Two ways over the lava: cool it, or rebuild the freight bridge.
    b.feature((116.0, c, 2.0), (119.0, c + 4.0, 8.0), F::Bridge { y: c + 2.0, needs: Some(Mech::FREIGHT) });
    b.feature((108.0, c - 1.0, 3.0), (128.0, c + 2.0, 7.0), F::Lava { y: c + 2.0, needs: Mech::COOLING });
    b.prop(furnace, PropKind::SluiceGate, "Cooling gate", 112.0, c + 3.2, -3.0, S::Foundry);

    // The Glass Archive: sealed in tombstone, its door answers the lens.
    let archive = b.room(D::GlassArchive, 1, K::Vault, "Glass Archive", S::Archive, Damage::Intact, v3(110.0, c + 2.0, 18.0), v3(126.0, c + 13.0, 32.0));
    b.space(archive, hall(110.0, 126.0, 18.0, 32.0, c + 2.0, 8.0, 3.0), S::Archive, None, true);
    b.tunnel((118.0, c + 2.0, 11.0), (118.0, c + 2.0, 19.0), S::Archive, Some((Mech::LENS, B::TombStone)), true);
    b.feature((110.0, c + 2.0, 18.0), (126.0, c + 6.0, 24.0), F::Shelves { floor: c + 2.0, block: B::StarGlass });
    b.feature((115.0, c + 2.0, 25.0), (121.0, c + 5.0, 31.0), F::Reliquary { floor: c + 2.0 });
    b.prop(archive, PropKind::Plinth, "Archive reliquary", 118.0, c + 4.4, 28.0, S::Archive);
}
