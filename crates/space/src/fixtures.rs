//! Small valid systems that tests mutate into invalid ones.
use crate::data::{BinaryDef, BodyDef, BodyKind, OrbitDef, SystemDef};

pub fn orbit(period_s: f64) -> OrbitDef {
    OrbitDef {
        period_s,
        e: 0.1,
        i_deg: 0.0,
        node_deg: 0.0,
        peri_deg: 0.0,
        mean_anomaly_deg: 0.0,
    }
}

pub fn body(id: &str, kind: BodyKind, parent: Option<&str>, orbit: Option<OrbitDef>) -> BodyDef {
    BodyDef {
        id: id.into(),
        kind,
        mass_kg: 1.0e20,
        radius_m: 1.0e4,
        parent: parent.map(String::from),
        orbit,
        binary: None,
        rotation: None,
    }
}

pub fn system(bodies: Vec<BodyDef>) -> SystemDef {
    SystemDef {
        id: "test".into(),
        time_scale: 1.0,
        bodies,
    }
}

pub fn star() -> BodyKind {
    BodyKind::Star {
        temperature_k: 5800.0,
    }
}

/// Root star `s` with planet `p` orbiting it.
pub fn two_body() -> SystemDef {
    system(vec![
        body("s", star(), None, None),
        body("p", BodyKind::Planet, Some("s"), Some(orbit(1000.0))),
    ])
}

/// Root barycenter `pair` of stars `a` and `b`, with circumbinary planet `p`.
pub fn binary() -> SystemDef {
    let mut pair = body("pair", BodyKind::Barycenter, None, None);
    pair.mass_kg = 0.0;
    pair.radius_m = 0.0;
    pair.binary = Some(BinaryDef {
        a: "a".into(),
        b: "b".into(),
        orbit: orbit(5000.0),
    });
    system(vec![
        pair,
        body("a", star(), Some("pair"), None),
        body("b", star(), Some("pair"), None),
        body("p", BodyKind::Planet, Some("pair"), Some(orbit(90_000.0))),
    ])
}

/// Star `s` orbited by barycenter `pair` of planets `a` and `b`, listed children first.
pub fn binary_around_star() -> SystemDef {
    let mut root = body("s", star(), None, None);
    root.mass_kg = 2.0e30;
    root.radius_m = 7.0e8;
    let mut pair = body("pair", BodyKind::Barycenter, Some("s"), Some(orbit(3.0e7)));
    // Derived from the components; compile ignores the authored value.
    pair.mass_kg = 123.0;
    pair.radius_m = 0.0;
    let mut shared = orbit(1.0e6);
    shared.e = 0.4;
    shared.i_deg = 7.0;
    shared.node_deg = 35.0;
    shared.peri_deg = 60.0;
    shared.mean_anomaly_deg = 15.0;
    pair.binary = Some(BinaryDef {
        a: "a".into(),
        b: "b".into(),
        orbit: shared,
    });
    let mut a = body("a", BodyKind::Planet, Some("pair"), None);
    a.mass_kg = 1.0e27;
    let mut b = body("b", BodyKind::GasGiant, Some("pair"), None);
    b.mass_kg = 5.0e26;
    system(vec![a, b, pair, root])
}
