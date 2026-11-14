//! Authoritative schema for Forge's portal layout. This does not create terrain or dimensions.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Dimension {
    Surface,
    Hell,
    Manyfold,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub dimension: Dimension,
    pub position: [f64; 3],
    pub yaw_degrees: f64,
    pub half_size: [f64; 2],
    /// Editor placement only. Never interpreted as world coordinates.
    pub canvas: [f64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LinkKind {
    Fold,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub id: String,
    pub kind: LinkKind,
    pub from: String,
    pub to: String,
    pub two_way: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub version: u32,
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub target: String,
    pub message: String,
}

fn issue(target: &str, message: &str) -> Diagnostic {
    Diagnostic {
        target: target.into(),
        message: message.into(),
    }
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn parse(text: &str) -> Result<Layout, Vec<Diagnostic>> {
    if text.len() > 262_144 {
        return Err(vec![issue("layout", "Layout exceeds 256 KiB")]);
    }
    let layout: Layout =
        ron::from_str(text).map_err(|error| vec![issue("layout", &error.to_string())])?;
    layout.validate()?;
    Ok(layout)
}

impl Layout {
    pub fn validate(&self) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        if self.version != 1 {
            errors.push(issue("layout", "Unsupported layout version"));
        }
        if self.nodes.len() > 64 || self.links.len() > 32 {
            return Err(vec![issue(
                "layout",
                "Use at most 64 openings and 32 links",
            )]);
        }
        let mut nodes = HashMap::new();
        for node in &self.nodes {
            if !id(&node.id) || nodes.insert(node.id.as_str(), node).is_some() {
                errors.push(issue(
                    &node.id,
                    "Opening IDs must be unique lowercase letters, digits or underscores (1–64)",
                ));
            }
            if node.label.trim().is_empty()
                || node.label.chars().count() > 80
                || node.label.chars().any(char::is_control)
            {
                errors.push(issue(
                    &node.id,
                    "Use an opening label of 1–80 visible characters",
                ));
            }
            if !node
                .position
                .iter()
                .all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
                || !node.yaw_degrees.is_finite()
                || node.yaw_degrees.abs() > 360.0
                || !node
                    .half_size
                    .iter()
                    .all(|n| n.is_finite() && *n >= 0.25 && *n <= 32.0)
                || !node
                    .canvas
                    .iter()
                    .all(|n| n.is_finite() && n.abs() <= 10000.0)
            {
                errors.push(issue(&node.id, "Check position (±1,000,000 m), facing (±360°), size (0.5–64 m), and graph placement"));
            }
        }
        let mut ids = HashSet::new();
        let mut outgoing = HashSet::new();
        for link in &self.links {
            if !id(&link.id) || !ids.insert(link.id.as_str()) {
                errors.push(issue(
                    &link.id,
                    "Link IDs must be unique lowercase letters, digits or underscores (1–64)",
                ));
            }
            let (Some(a), Some(b)) = (nodes.get(link.from.as_str()), nodes.get(link.to.as_str()))
            else {
                errors.push(issue(&link.id, "Link refers to a missing opening"));
                continue;
            };
            if a.id == b.id {
                errors.push(issue(&link.id, "Connect two different openings"));
            }
            if a.dimension != b.dimension {
                errors.push(issue(&link.id, "Fold links stay in one dimension. Cross-dimension travel requires a realm-transfer adapter"));
            }
            if a.half_size
                .iter()
                .zip(b.half_size)
                .any(|(a, b)| (a - b).abs() >= 1e-6)
            {
                errors.push(issue(
                    &link.id,
                    "Both openings need the same width and height; body scaling is unsupported",
                ));
            }
            if a.position
                .iter()
                .zip(b.position)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                < 1.0
            {
                errors.push(issue(
                    &link.id,
                    "Connected opening centres must be at least 1 metre apart",
                ));
            }
            if link.enabled {
                for source in std::iter::once(&link.from).chain(link.two_way.then_some(&link.to)) {
                    if !outgoing.insert(source) {
                        errors.push(issue(
                            &link.id,
                            "An opening can have only one enabled outgoing link",
                        ));
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn warnings(&self) -> Vec<Diagnostic> {
        let mut warnings = Vec::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut used = HashSet::new();
        for link in self.links.iter().filter(|link| link.enabled) {
            adjacency.entry(&link.from).or_default().push(&link.to);
            if link.two_way {
                adjacency.entry(&link.to).or_default().push(&link.from);
            }
            used.insert(&link.from);
            used.insert(&link.to);
        }
        for node in &self.nodes {
            if !used.contains(&node.id) {
                warnings.push(issue(&node.id, "Opening is not on an enabled route"));
            }
        }
        for link in self
            .links
            .iter()
            .filter(|link| link.enabled && !link.two_way)
        {
            let mut queue = vec![link.to.as_str()];
            let mut visited = HashSet::new();
            while let Some(next) = queue.pop() {
                if visited.insert(next) {
                    queue.extend(adjacency.get(next).into_iter().flatten().copied());
                }
            }
            if !visited.contains(link.from.as_str()) {
                warnings.push(issue(
                    &link.id,
                    "One-way route has no return path in this graph",
                ));
            }
        }
        warnings
    }
}

pub static LAYOUT: std::sync::LazyLock<Layout> = std::sync::LazyLock::new(|| {
    parse(include_str!("../../../assets/portal_layout.ron"))
        .expect("Invalid authored portal layout")
});
