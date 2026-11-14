//! 3x5 bitmap caption font for tile names on the sheet.

/// Five rows of three bits, top row first, for one character.
fn glyph(c: char) -> [u8; 5] {
    let rows: &str = match c.to_ascii_uppercase() {
        'A' => "010101111101101", 'B' => "110101110101110", 'C' => "011100100100011",
        'D' => "110101101101110", 'E' => "111100110100111", 'F' => "111100110100100",
        'G' => "011100101101011", 'H' => "101101111101101", 'I' => "111010010010111",
        'J' => "001001001101010", 'K' => "101101110101101", 'L' => "100100100100111",
        'M' => "101111111101101", 'N' => "110101101101101", 'O' => "010101101101010",
        'P' => "110101110100100", 'Q' => "010101101110011", 'R' => "110101110101101",
        'S' => "011100010001110", 'T' => "111010010010010", 'U' => "101101101101111",
        'V' => "101101101101010", 'W' => "101101111111101", 'X' => "101101010101101",
        'Y' => "101101010010010", 'Z' => "111001010100111",
        '0' => "111101101101111", '1' => "010110010010111", '2' => "110001010100111",
        '3' => "110001010001110", '4' => "101101111001001", '5' => "111100110001110",
        '6' => "011100111101111", '7' => "111001010010010", '8' => "111101111101111",
        '9' => "111101111001110",
        '-' => "000000111000000", '.' => "000000000000010", '_' => "000000000000111",
        ':' => "000010000010000", '/' => "001001010100100", '=' => "000111000111000",
        _ => "000000000000000",
    };
    let mut out = [0u8; 5];
    for (r, row) in out.iter_mut().enumerate() {
        for (i, ch) in rows[r * 3..r * 3 + 3].chars().enumerate() {
            *row |= u8::from(ch == '1') << (2 - i);
        }
    }
    out
}

/// Draws `text` at integer pixel scale over a dark plate; clips to the image.
pub fn label(rgba: &mut [u8], width: u32, x: u32, y: u32, scale: u32, text: &str) {
    let height = (rgba.len() as u32 / 4) / width;
    let plate_w = text.chars().count() as u32 * 4 * scale + scale;
    let mut put = |px: u32, py: u32, c: [u8; 3]| {
        if px < width && py < height {
            let i = ((py * width + px) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(&c);
        }
    };
    for py in 0..(5 + 2) * scale {
        for px in 0..plate_w {
            put(x + px, y + py, [14, 14, 18]);
        }
    }
    for (n, c) in text.chars().enumerate() {
        for (r, row) in glyph(c).iter().enumerate() {
            for col in 0..3u32 {
                if row >> (2 - col) & 1 == 1 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            put(x + scale + (n as u32 * 4 + col) * scale + sx, y + scale + r as u32 * scale + sy, [236, 232, 220]);
                        }
                    }
                }
            }
        }
    }
}
