//! Load-time checks that make a star system safe to compile.
use crate::data::{BinaryDef, BodyDef, BodyKind, OrbitDef, RotationPeriod, SystemDef};
use std::collections::HashMap;
use std::fmt;

/// Why a systems file was rejected.
#[derive(Clone, Debug, PartialEq)]
pub enum SystemError {
    DuplicateId(String),
    UnknownParent {
        body: String,
        parent: String,
    },
    /// The named body's parent chain never reaches the root.
    Cycle(String),
    /// The named barycenter, or a non-barycenter that carries a binary.
    BadBinary(String),
    /// A numeric field is out of range. `body` is the system id for system-wide fields.
    BadOrbit {
        body: String,
        reason: &'static str,
    },
    /// The system has this many parentless bodies instead of one.
    Roots(usize),
    Parse(String),
}

impl fmt::Display for SystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemError::DuplicateId(id) => write!(f, "duplicate body id `{id}`"),
            SystemError::UnknownParent { body, parent } => {
                write!(f, "body `{body}` names unknown parent `{parent}`")
            }
            SystemError::Cycle(id) => write!(f, "parent chain of body `{id}` has a cycle"),
            SystemError::BadBinary(id) => write!(f, "invalid binary definition on `{id}`"),
            SystemError::BadOrbit { body, reason } => write!(f, "`{body}`: {reason}"),
            SystemError::Roots(n) => write!(f, "expected one root body, found {n}"),
            SystemError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for SystemError {}

type Index<'a> = HashMap<&'a str, usize>;

/// Checks one system's hierarchy, binaries and numeric ranges.
pub fn validate(def: &SystemDef) -> Result<(), SystemError> {
    let index = index_bodies(def)?;
    check_parents(def, &index)?;
    check_acyclic(def, &index)?;
    let roots = def.bodies.iter().filter(|b| b.parent.is_none()).count();
    if roots != 1 {
        return Err(SystemError::Roots(roots));
    }
    positive(
        def.time_scale,
        &def.id,
        "time_scale must be finite and positive",
    )?;
    for body in &def.bodies {
        check_body(def, &index, body)?;
    }
    Ok(())
}

fn index_bodies(def: &SystemDef) -> Result<Index<'_>, SystemError> {
    let mut index = Index::new();
    for (i, body) in def.bodies.iter().enumerate() {
        if index.insert(body.id.as_str(), i).is_some() {
            return Err(SystemError::DuplicateId(body.id.clone()));
        }
    }
    Ok(index)
}

fn check_parents(def: &SystemDef, index: &Index<'_>) -> Result<(), SystemError> {
    for body in &def.bodies {
        if let Some(parent) = &body.parent
            && !index.contains_key(parent.as_str())
        {
            return Err(SystemError::UnknownParent {
                body: body.id.clone(),
                parent: parent.clone(),
            });
        }
    }
    Ok(())
}

/// A chain longer than the body count must revisit a body.
fn check_acyclic(def: &SystemDef, index: &Index<'_>) -> Result<(), SystemError> {
    for body in &def.bodies {
        let mut at = body;
        let mut steps = 0;
        while let Some(&i) = at.parent.as_deref().and_then(|p| index.get(p)) {
            steps += 1;
            if steps > def.bodies.len() {
                return Err(SystemError::Cycle(body.id.clone()));
            }
            at = &def.bodies[i];
        }
    }
    Ok(())
}

fn check_body(def: &SystemDef, index: &Index<'_>, body: &BodyDef) -> Result<(), SystemError> {
    let id = body.id.as_str();
    non_negative(body.mass_kg, id, "mass_kg must be finite and not negative")?;
    non_negative(
        body.radius_m,
        id,
        "radius_m must be finite and not negative",
    )?;
    if let BodyKind::Star { temperature_k } = body.kind {
        positive(
            temperature_k,
            id,
            "temperature_k must be finite and positive",
        )?;
    }
    let is_barycenter = body.kind == BodyKind::Barycenter;
    if is_barycenter != body.binary.is_some() {
        return Err(SystemError::BadBinary(body.id.clone()));
    }
    if let Some(binary) = &body.binary {
        check_binary(def, index, body, binary)?;
    }
    let component_of = body
        .parent
        .as_deref()
        .filter(|_| is_component(def, index, body));
    match (&body.orbit, body.parent.is_some(), component_of) {
        (Some(_), true, Some(barycenter)) => {
            return Err(SystemError::BadBinary(barycenter.to_string()));
        }
        (Some(orbit), true, None) => check_orbit(orbit, id)?,
        (None, true, Some(_)) | (None, false, _) => {}
        (Some(_), false, _) => return Err(bad(id, "root body cannot have an orbit")),
        (None, true, None) => return Err(bad(id, "body needs an orbit around its parent")),
    }
    check_rotation(body, component_of.is_some())
}

fn is_component(def: &SystemDef, index: &Index<'_>, body: &BodyDef) -> bool {
    body.parent
        .as_deref()
        .and_then(|p| index.get(p))
        .and_then(|&i| def.bodies[i].binary.as_ref())
        .is_some_and(|b| b.a == body.id || b.b == body.id)
}

fn check_binary(
    def: &SystemDef,
    index: &Index<'_>,
    barycenter: &BodyDef,
    binary: &BinaryDef,
) -> Result<(), SystemError> {
    let invalid = || SystemError::BadBinary(barycenter.id.clone());
    if binary.a == binary.b {
        return Err(invalid());
    }
    for name in [&binary.a, &binary.b] {
        let component = index.get(name.as_str()).map(|&i| &def.bodies[i]);
        if component.and_then(|c| c.parent.as_deref()) != Some(barycenter.id.as_str()) {
            return Err(invalid());
        }
    }
    check_orbit(&binary.orbit, &barycenter.id)
}

fn check_orbit(orbit: &OrbitDef, who: &str) -> Result<(), SystemError> {
    positive(orbit.period_s, who, "period_s must be finite and positive")?;
    if !(0.0..=0.999).contains(&orbit.e) {
        return Err(bad(who, "e must be in 0 to 0.999"));
    }
    let angles = [
        orbit.i_deg,
        orbit.node_deg,
        orbit.peri_deg,
        orbit.mean_anomaly_deg,
    ];
    if angles.iter().any(|a| !a.is_finite()) {
        return Err(bad(who, "orbit angles must be finite"));
    }
    Ok(())
}

/// A binary component locks to its partner on the binary's shared period.
fn check_rotation(body: &BodyDef, component: bool) -> Result<(), SystemError> {
    let Some(rotation) = &body.rotation else {
        return Ok(());
    };
    let id = body.id.as_str();
    if !rotation.tilt_deg.is_finite() || !rotation.north_deg.is_finite() {
        return Err(bad(id, "rotation angles must be finite"));
    }
    match rotation.period {
        RotationPeriod::Seconds(s) => {
            positive(s, id, "rotation period must be finite and positive")
        }
        RotationPeriod::Locked if body.orbit.is_none() && !component => Err(bad(
            id,
            "locked rotation needs an orbit or a binary partner",
        )),
        RotationPeriod::Locked => Ok(()),
    }
}

fn bad(who: &str, reason: &'static str) -> SystemError {
    SystemError::BadOrbit {
        body: who.to_string(),
        reason,
    }
}

fn positive(v: f64, who: &str, reason: &'static str) -> Result<(), SystemError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(bad(who, reason))
    }
}

fn non_negative(v: f64, who: &str, reason: &'static str) -> Result<(), SystemError> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err(bad(who, reason))
    }
}

#[cfg(test)]
mod tests;
