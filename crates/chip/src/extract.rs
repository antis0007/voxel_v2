//! Reads connectivity back out of the drawn geometry, independent of which
//! net a wire claims to belong to. Two nets on one site are a short; a wire
//! crossing another on a different layer is not a connection unless a via
//! joins them; every pin of a net must land in one connected piece.
use crate::design::{Design, Site};
use crate::registry::Registry;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// Nets with no wire at all.
    pub unrouted: Vec<u16>,
    /// Nets whose wire does not join all of their pins.
    pub opens: Vec<u16>,
    /// Pairs of nets that share a site.
    pub shorts: Vec<(u16, u16)>,
    /// Geometry errors: (net, reason).
    pub illegal: Vec<(u16, String)>,
}

impl Extraction {
    pub fn clean(&self) -> bool {
        self.unrouted.is_empty() && self.opens.is_empty() && self.shorts.is_empty() && self.illegal.is_empty()
    }
    /// Nets a repair should rip up and reroute.
    pub fn broken(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self
            .opens
            .iter()
            .copied()
            .chain(self.shorts.iter().flat_map(|&(a, b)| [a, b]))
            .chain(self.illegal.iter().map(|(n, _)| *n))
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

pub fn extract(design: &Design, registry: &Registry) -> Extraction {
    let (w, h, layers) = (usize::from(design.width), usize::from(design.height), usize::from(design.layers));
    let index = |s: Site| (usize::from(s.layer) * h + usize::from(s.y)) * w + usize::from(s.x);
    let inside = |s: Site| usize::from(s.x) < w && usize::from(s.y) < h && usize::from(s.layer) < layers;
    let mut parent: Vec<usize> = (0..w * h * layers).collect();
    let mut owner = vec![0u16; w * h * layers];
    let mut out = Extraction::default();

    for net in &design.nets {
        if net.paths.is_empty() {
            out.unrouted.push(net.id);
            continue;
        }
        let pins: Vec<Site> = net.pins.iter().filter_map(|p| design.pin_site(registry, p)).collect();
        let mut bad = None;
        for path in &net.paths {
            for (k, &s) in path.iter().enumerate() {
                if !inside(s) {
                    bad = Some("leaves the die or the layer stack");
                    break;
                }
                if s.layer == 0 && design.part_at(registry, s.x, s.y).is_some() && !pins.contains(&s) {
                    bad = Some("runs through a module body on the contact layer");
                    break;
                }
                if k > 0 && !path[k - 1].adjacent(s) {
                    bad = Some("jumps: steps must be adjacent sites or a via");
                    break;
                }
                let i = index(s);
                match owner[i] {
                    0 => owner[i] = net.id,
                    o if o == net.id => {}
                    o => {
                        let pair = (o.min(net.id), o.max(net.id));
                        if !out.shorts.contains(&pair) {
                            out.shorts.push(pair);
                        }
                    }
                }
                if k > 0 {
                    let (a, b) = (find(&mut parent, index(path[k - 1])), find(&mut parent, i));
                    parent[a] = b;
                }
            }
            if bad.is_some() {
                break;
            }
        }
        if let Some(reason) = bad {
            out.illegal.push((net.id, reason.to_string()));
            continue;
        }
        let reached = pins.len() == net.pins.len() && pins.iter().all(|&p| owner[index(p)] == net.id);
        let root = pins.first().map(|&p| find(&mut parent, index(p)));
        let joined = pins.iter().all(|&p| Some(find(&mut parent, index(p))) == root);
        if !(reached && joined) {
            out.opens.push(net.id);
        }
    }
    out
}
