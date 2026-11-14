//! Spiral drum, avenue, colonnade and inner gallery on the vast shell.
use super::shell::Class;

pub fn twist(z: f32) -> f32 {
    z * 0.00035
}

pub fn clearance(radius: f32) -> f32 {
    (radius * 0.42).clamp(28.0, 120.0)
}

pub fn vault(row: i32) -> bool {
    row.rem_euclid(4) == 0
}

pub fn avenue(sector: i32, sectors: i32) -> bool {
    let sector = sector.rem_euclid(sectors);
    sector == 0 || sector == sectors / 2
}

/// Deck and ramp sit inward of the maze walls. `None` leaves the maze in charge.
pub fn relief(
    above: f32,
    sector: i32,
    row: i32,
    local_a: f32,
    local_z: f32,
    pitch: f32,
    row_len: f32,
) -> Option<Class> {
    let deck = -38.0..-35.5;
    if sector.rem_euclid(2) == 1 && deck.contains(&above) && !oculus(row, local_a, local_z, pitch, row_len) {
        return Some(Class::Crust);
    }
    if sector.rem_euclid(6) == 1 && row.rem_euclid(3) == 1 && (local_a - pitch * 0.5).abs() < 0.18 {
        let rise = (local_z / row_len).clamp(0.0, 1.0) * 36.0;
        if (above + rise).abs() < 1.3 && (-37.0..1.0).contains(&above) {
            return Some(Class::Crust);
        }
    }
    None
}

fn oculus(row: i32, local_a: f32, local_z: f32, pitch: f32, row_len: f32) -> bool {
    let well = if vault(row) { 0.40 } else { 0.20 };
    (local_z - row_len * 0.5).abs() < row_len * well
        && (local_a - pitch * 0.5).abs() < pitch * (well + 0.08)
}

/// A divider keeps stone at piers and, outside a vault, a lintel. Avenues have no cross-wall.
pub fn keeps_divider(
    around: bool,
    along: bool,
    sector: i32,
    sectors: i32,
    row: i32,
    local_a: f32,
    local_z: f32,
    above: f32,
    wall: f32,
    radius: f32,
) -> bool {
    if avenue(sector, sectors) && along && !around {
        return false;
    }
    let open = vault(row);
    let mut solid = false;
    if around {
        solid |= arcade(local_z, above, wall, open);
    }
    if along {
        solid |= arcade(local_a * radius, above, wall, open);
    }
    solid
}

fn arcade(span_m: f32, above: f32, wall: f32, vault: bool) -> bool {
    let pitch = if vault { 64.0 } else { 42.0 };
    let pier = if vault { 7.0 } else { 9.0 };
    span_m.rem_euclid(pitch) < pier || (!vault && above < -(wall - 12.0).max(16.0))
}
