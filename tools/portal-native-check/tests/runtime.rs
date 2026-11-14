extern crate self as engine_content;
#[path = "../../../crates/content/src/portal_layout.rs"]
pub mod portal_layout;
pub mod world {
    #[derive(Clone, Copy)]
    pub enum WorldType {
        Superflat,
        Hills,
        Ridged,
        Planet,
        Nether,
        Manyfold,
        Hall,
    }
}
pub mod portals {
    include!(concat!(env!("OUT_DIR"), "/portal_core.rs"));
}

use glam::{DQuat, DVec2, DVec3};
use portal_layout::{Dimension, Layout, Link, LinkKind, Node};
use portals::{Transit, authored, gates};
fn sample() -> Layout {
    Layout {
        version: 1,
        nodes: vec![
            Node {
                id: "a".into(),
                label: "Entry".into(),
                dimension: Dimension::Surface,
                position: [0.0, 3.0, 0.0],
                yaw_degrees: 0.0,
                half_size: [2.0, 2.0],
                canvas: [50.0, 50.0],
            },
            Node {
                id: "b".into(),
                label: "Exit".into(),
                dimension: Dimension::Surface,
                position: [12.0, 3.0, 0.0],
                yaw_degrees: 90.0,
                half_size: [2.0, 2.0],
                canvas: [350.0, 50.0],
            },
        ],
        links: vec![Link {
            id: "fold_1".into(),
            kind: LinkKind::Fold,
            from: "a".into(),
            to: "b".into(),
            enabled: true,
            two_way: true,
        }],
    }
}
fn transit(network: &portals::Network, clear: bool, body: DVec2) -> Transit {
    network.transit(
        &gates::realm(),
        DVec3::new(0.0, 3.0, 1.0),
        DVec3::new(0.0, 3.0, -1.0),
        DVec3::NEG_Z * 5.0,
        DQuat::IDENTITY,
        body,
        |_, _, _, _| clear,
    )
}
#[test]
fn layout_save_reload_builds_real_bidirectional_portals() {
    let source = sample();
    let saved = ron::to_string(&source).unwrap();
    let loaded = portal_layout::parse(&saved).unwrap();
    let network = authored::build(&loaded, Dimension::Surface).unwrap();
    assert_eq!(network.portals.len(), 2);
    let Transit::Crossed(crossing) = transit(&network, true, DVec2::splat(0.2)) else {
        panic!("Expected crossing")
    };
    assert!((crossing.position - DVec3::new(13.0, 3.0, 0.0)).length() < 1e-8);
    assert!((crossing.velocity - DVec3::X * 5.0).length() < 1e-8);
    let back = network.transit(
        &gates::realm(),
        DVec3::new(13.0, 3.0, 0.0),
        DVec3::new(11.0, 3.0, 0.0),
        -crossing.velocity,
        DQuat::IDENTITY,
        DVec2::splat(0.2),
        |_, _, _, _| true,
    );
    let Transit::Crossed(back) = back else {
        panic!("Expected return")
    };
    assert!((back.position - DVec3::new(0.0, 3.0, 1.0)).length() < 1e-8);
}
#[test]
fn blocked_or_unloaded_exit_cannot_be_crossed() {
    let network = authored::build(&sample(), Dimension::Surface).unwrap();
    assert!(matches!(
        transit(&network, false, DVec2::splat(0.2)),
        Transit::Blocked
    ));
    assert!(matches!(
        transit(&network, true, DVec2::splat(3.0)),
        Transit::Blocked
    ));
}
#[test]
fn direction_availability_and_dimension_control_runtime_routes() {
    let mut graph = sample();
    graph.links[0].two_way = false;
    let network = authored::build(&graph, Dimension::Surface).unwrap();
    assert_eq!(network.portals.len(), 1);
    assert!(matches!(
        network.transit(
            &gates::realm(),
            DVec3::new(13.0, 3.0, 0.0),
            DVec3::new(11.0, 3.0, 0.0),
            DVec3::NEG_X,
            DQuat::IDENTITY,
            DVec2::splat(0.2),
            |_, _, _, _| true
        ),
        Transit::Unchanged
    ));
    assert_eq!(graph.warnings().len(), 1);
    assert!(
        authored::build(&graph, Dimension::Hell)
            .unwrap()
            .portals
            .is_empty()
    );
    graph.links[0].enabled = false;
    assert!(
        authored::build(&graph, Dimension::Surface)
            .unwrap()
            .portals
            .is_empty()
    );
}
#[test]
fn stable_identity_ignores_canvas_and_row_order() {
    let mut graph = sample();
    let before = authored::build(&graph, Dimension::Surface).unwrap();
    graph.nodes.reverse();
    graph.nodes[0].canvas = [900.0, 430.0];
    let after = authored::build(&graph, Dimension::Surface).unwrap();
    assert_eq!(
        before.portals.iter().map(|p| p.id).collect::<Vec<_>>(),
        after.portals.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert_eq!(
        before.portals[0].entrance.center,
        after.portals[0].entrance.center
    );
}
#[test]
fn invalid_or_cross_dimension_routes_never_reach_network() {
    let mut graph = sample();
    graph.nodes[1].dimension = Dimension::Hell;
    assert!(authored::build(&graph, Dimension::Surface).is_err());
    graph.nodes[1].dimension = Dimension::Surface;
    graph.links[0].to = "missing".into();
    assert!(authored::build(&graph, Dimension::Surface).is_err());
}
#[test]
fn default_catalog_leaves_existing_worlds_unchanged() {
    assert!(portal_layout::LAYOUT.nodes.is_empty());
    assert!(authored::portals(world::WorldType::Hall).is_empty());
    let existing = portals::Network::default();
    assert!(matches!(
        authored::combined(world::WorldType::Superflat, &existing),
        std::borrow::Cow::Borrowed(_)
    ));
}
