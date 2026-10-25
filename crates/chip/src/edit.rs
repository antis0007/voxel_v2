//! Floorplan and netlist edits. Each keeps the floorplan legal (inside the
//! die, no overlap) and returns the nets whose wires it left stale.
use crate::design::{Design, EditError, Net, Part, Pin, Site, rotated_size};
use crate::registry::Registry;

impl Design {
    fn check_fit(
        &self,
        registry: &Registry,
        module: &str,
        x: u8,
        y: u8,
        rot: u8,
        ignore: Option<u16>,
    ) -> Result<(), EditError> {
        let spec = registry.module(module).ok_or(EditError::UnknownModule)?;
        let (w, h) = rotated_size(spec, rot);
        if u16::from(x) + u16::from(w) > u16::from(self.width)
            || u16::from(y) + u16::from(h) > u16::from(self.height)
        {
            return Err(EditError::OutsideDie);
        }
        for other in self.parts.iter().filter(|p| Some(p.id) != ignore) {
            if let Some((ox, oy, ow, oh)) = self.rect(registry, other)
                && x < ox + ow
                && ox < x + w
                && y < oy + oh
                && oy < y + h
            {
                return Err(EditError::Overlaps(other.id));
            }
        }
        Ok(())
    }

    pub fn place(&mut self, registry: &Registry, module: &str, x: u8, y: u8, rot: u8) -> Result<u16, EditError> {
        self.check_fit(registry, module, x, y, rot % 4, None)?;
        let id = self.fresh_id();
        self.parts.push(Part { id, module: module.to_string(), x, y, rot: rot % 4 });
        Ok(id)
    }

    /// Move and/or rotate. Returns the nets whose wires are now stale.
    pub fn relocate(&mut self, registry: &Registry, id: u16, x: u8, y: u8, rot: u8) -> Result<Vec<u16>, EditError> {
        let module = self.part(id).ok_or(EditError::UnknownPart)?.module.clone();
        self.check_fit(registry, &module, x, y, rot % 4, Some(id))?;
        if let Some(p) = self.parts.iter_mut().find(|p| p.id == id) {
            (p.x, p.y, p.rot) = (x, y, rot % 4);
        }
        Ok(self.invalidate_around(registry, id))
    }

    /// Remove a part and its pins. Returns the nets left stale.
    pub fn remove(&mut self, _registry: &Registry, id: u16) -> Result<Vec<u16>, EditError> {
        self.part(id).ok_or(EditError::UnknownPart)?;
        let mut stale: Vec<u16> = self
            .nets
            .iter()
            .filter(|n| n.pins.iter().any(|p| p.part == id))
            .map(|n| n.id)
            .collect();
        self.parts.retain(|p| p.id != id);
        for net in &mut self.nets {
            net.pins.retain(|p| p.part != id);
        }
        self.nets.retain(|n| n.pins.len() >= 2);
        stale.retain(|s| self.nets.iter().any(|n| n.id == *s));
        for &s in &stale {
            self.clear_wire(s);
        }
        Ok(stale)
    }

    /// Nets touching the part, plus nets whose wires now run through its
    /// body on the contact layer. Their wires are cleared (manual ones too:
    /// a moved pin no longer meets the drawn path).
    fn invalidate_around(&mut self, registry: &Registry, id: u16) -> Vec<u16> {
        let Some(rect) = self.part(id).and_then(|p| self.rect(registry, p)) else {
            return Vec::new();
        };
        let (x, y, w, h) = rect;
        let stale: Vec<u16> = self
            .nets
            .iter()
            .filter(|n| {
                n.pins.iter().any(|p| p.part == id)
                    || n.paths.iter().flatten().any(|s| {
                        s.layer == 0 && s.x >= x && s.x < x + w && s.y >= y && s.y < y + h
                    })
            })
            .map(|n| n.id)
            .collect();
        for &s in &stale {
            self.clear_wire(s);
        }
        stale
    }

    pub fn clear_wire(&mut self, net: u16) {
        if let Some(n) = self.nets.iter_mut().find(|n| n.id == net) {
            n.paths.clear();
            n.manual = false;
        }
    }

    /// Join two ports. Merges their nets if both already belong to one.
    /// Returns the resulting net, whose wire is cleared for rerouting.
    pub fn connect(&mut self, registry: &Registry, a: Pin, b: Pin) -> Result<u16, EditError> {
        for pin in [&a, &b] {
            let part = self.part(pin.part).ok_or(EditError::UnknownPart)?;
            let spec = registry.module(&part.module).ok_or(EditError::UnknownModule)?;
            spec.port(&pin.port).ok_or(EditError::UnknownPort)?;
        }
        let find = |d: &Self, pin: &Pin| d.nets.iter().position(|n| n.pins.contains(pin));
        let id = match (find(self, &a), find(self, &b)) {
            (Some(i), Some(j)) if i == j => self.nets[i].id,
            (Some(i), Some(j)) => {
                let (keep, drop) = (i.min(j), i.max(j));
                let moved = self.nets.remove(drop);
                self.nets[keep].pins.extend(moved.pins);
                self.nets[keep].id
            }
            (Some(i), None) => {
                self.nets[i].pins.push(b);
                self.nets[i].id
            }
            (None, Some(j)) => {
                self.nets[j].pins.push(a);
                self.nets[j].id
            }
            (None, None) => {
                let id = self.fresh_id();
                let name = format!("{} {}", a.port.to_uppercase(), id);
                self.nets.push(Net { id, name, pins: vec![a, b], paths: Vec::new(), manual: false });
                id
            }
        };
        if let Some(n) = self.nets.iter_mut().find(|n| n.id == id) {
            n.pins.sort();
            n.pins.dedup();
        }
        self.clear_wire(id);
        Ok(id)
    }

    /// Detach one port from its net.
    pub fn disconnect(&mut self, pin: &Pin) -> Option<u16> {
        let i = self.nets.iter().position(|n| n.pins.contains(pin))?;
        let id = self.nets[i].id;
        self.nets[i].pins.retain(|p| p != pin);
        if self.nets[i].pins.len() < 2 {
            self.nets.remove(i);
            return None;
        }
        self.clear_wire(id);
        Some(id)
    }

    /// Replace a net's wire with a player-drawn path; `compile` checks it.
    pub fn draw(&mut self, net: u16, paths: Vec<Vec<Site>>) {
        if let Some(n) = self.nets.iter_mut().find(|n| n.id == net) {
            n.paths = paths;
            n.manual = true;
        }
    }

    pub fn net_of(&self, pin: &Pin) -> Option<&Net> {
        self.nets.iter().find(|n| n.pins.contains(pin))
    }
}
