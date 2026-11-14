//! Additive compiled portal layouts. Never writes terrain, saves, or crystal links.
use super::{Frame, Network, Portal, gates};
use crate::world::WorldType;
use engine_content::portal_layout::{Dimension, LAYOUT, Layout};
use glam::{DQuat, DVec2, DVec3};
use std::{borrow::Cow, sync::LazyLock};

pub fn build(layout: &Layout, dimension: Dimension) -> anyhow::Result<Network> {
    layout
        .validate()
        .map_err(|e| anyhow::anyhow!("Invalid portal layout: {e:?}"))?;
    let mut result = Network::default();
    for link in layout.links.iter().filter(|link| link.enabled) {
        let a = layout
            .nodes
            .iter()
            .find(|node| node.id == link.from)
            .expect("validated source");
        let b = layout
            .nodes
            .iter()
            .find(|node| node.id == link.to)
            .expect("validated destination");
        if a.dimension != dimension {
            continue;
        }
        let frame = |node: &engine_content::portal_layout::Node| Frame {
            center: DVec3::from_array(node.position),
            rotation: DQuat::from_rotation_y(node.yaw_degrees.to_radians()),
            half_size: DVec2::from_array(node.half_size),
        };
        // Stable across row reordering; reserved namespace avoids crystal/maze IDs.
        let hash = link.id.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        });
        let id = 0xf000000000000000 | ((hash & 0x07ffffffffffffff) << 1);
        let portal = Portal {
            id,
            source: gates::realm(),
            destination: gates::realm(),
            entrance: frame(a),
            exit: frame(b),
        };
        if link.two_way {
            result.portals.push(portal.reverse(id + 1));
        }
        result.portals.push(portal);
    }
    result.validate()?;
    Ok(result)
}

pub fn portals(kind: WorldType) -> &'static [Portal] {
    static SURFACE: LazyLock<Network> =
        LazyLock::new(|| build(&LAYOUT, Dimension::Surface).expect("Surface portal layout"));
    static HELL: LazyLock<Network> =
        LazyLock::new(|| build(&LAYOUT, Dimension::Hell).expect("Hell portal layout"));
    static MANYFOLD: LazyLock<Network> =
        LazyLock::new(|| build(&LAYOUT, Dimension::Manyfold).expect("Manyfold portal layout"));
    match kind {
        WorldType::Nether => &HELL.portals,
        WorldType::Manyfold => &MANYFOLD.portals,
        WorldType::Hall | WorldType::Realm(_) => &[],
        _ => &SURFACE.portals,
    }
}

pub fn append(kind: WorldType, network: &mut Network) {
    let portals = portals(kind);
    // Existing saved gates take priority. Never publish half a two-way route.
    if network.portals.len() + portals.len() <= super::MAX_PORTALS {
        network.portals.extend_from_slice(portals);
    }
}

pub fn combined(kind: WorldType, existing: &Network) -> Cow<'_, Network> {
    if portals(kind).is_empty() || existing.portals.len() + portals(kind).len() > super::MAX_PORTALS
    {
        return Cow::Borrowed(existing);
    }
    let mut network = existing.clone();
    append(kind, &mut network);
    Cow::Owned(network)
}
