//! Per-world waypoints: position, short label and palette colour.
use egui::Color32;
use glam::IVec3;

pub const MAX_WAYPOINTS: usize = 256;
/// Name limit in bytes; the minimap shows only the first character.
pub const MAX_NAME: usize = 24;

pub const PALETTE: [Color32; 8] = [
    Color32::from_rgb(232, 182, 72),
    Color32::from_rgb(86, 196, 170),
    Color32::from_rgb(225, 96, 84),
    Color32::from_rgb(98, 146, 232),
    Color32::from_rgb(178, 118, 222),
    Color32::from_rgb(236, 236, 228),
    Color32::from_rgb(236, 134, 58),
    Color32::from_rgb(132, 204, 88),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Marker,
    Death,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Waypoint {
    pub name: String,
    pub pos: IVec3,
    pub colour: u8,
    pub kind: Kind,
}

impl Waypoint {
    /// One glyph for the minimap; death markers always show a cross.
    pub fn glyph(&self) -> String {
        match self.kind {
            Kind::Death => "✕".into(),
            Kind::Marker => self
                .name
                .chars()
                .next()
                .map_or_else(|| "·".into(), |c| c.to_uppercase().collect()),
        }
    }

    pub fn color(&self) -> Color32 {
        PALETTE[self.colour as usize % PALETTE.len()]
    }
}

/// Cut `name` to `MAX_NAME` bytes on a character boundary.
pub fn clip_name(name: &str) -> String {
    let mut end = name.len().min(MAX_NAME);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Waypoints {
    pub list: Vec<Waypoint>,
}

impl Waypoints {
    /// Add a numbered marker; `None` when the list is full.
    pub fn add(&mut self, pos: IVec3) -> Option<usize> {
        if self.list.len() >= MAX_WAYPOINTS {
            return None;
        }
        let number = (1..)
            .find(|n: &usize| !self.list.iter().any(|w| w.name == n.to_string()))
            .unwrap_or(0);
        self.list.push(Waypoint {
            name: number.to_string(),
            pos,
            colour: (self.list.len() % PALETTE.len()) as u8,
            kind: Kind::Marker,
        });
        Some(self.list.len() - 1)
    }

    /// Only the latest death is kept, as a fresh marker in place of the old one.
    pub fn set_death(&mut self, pos: IVec3) {
        self.list.retain(|w| w.kind != Kind::Death);
        if self.list.len() >= MAX_WAYPOINTS {
            self.list.pop();
        }
        self.list.push(Waypoint {
            name: "Death".into(),
            pos,
            colour: 2,
            kind: Kind::Death,
        });
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.list.len() {
            self.list.remove(index);
            true
        } else {
            false
        }
    }

    pub fn cycle_colour(&mut self, index: usize) {
        if let Some(w) = self.list.get_mut(index) {
            w.colour = (w.colour + 1) % PALETTE.len() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_waypoints_number_reuses_gaps_and_caps() {
        let mut w = Waypoints::default();
        assert_eq!(w.add(IVec3::ZERO), Some(0));
        assert_eq!(w.add(IVec3::ONE), Some(1));
        assert!(w.remove(0));
        w.add(IVec3::X);
        assert_eq!(w.list[1].name, "1");
        while w.add(IVec3::ZERO).is_some() {}
        assert_eq!(w.list.len(), MAX_WAYPOINTS);
    }

    #[test]
    fn map_waypoints_keep_one_death_marker() {
        let mut w = Waypoints::default();
        w.set_death(IVec3::new(1, 2, 3));
        w.set_death(IVec3::new(4, 5, 6));
        let deaths: Vec<_> = w.list.iter().filter(|w| w.kind == Kind::Death).collect();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].pos, IVec3::new(4, 5, 6));
        assert_eq!(deaths[0].glyph(), "✕");
    }

    #[test]
    fn map_waypoint_names_clip_on_char_boundary() {
        let long = "é".repeat(20);
        let clipped = clip_name(&long);
        assert!(clipped.len() <= MAX_NAME);
        assert_eq!(clipped.chars().count(), 12);
    }
}
