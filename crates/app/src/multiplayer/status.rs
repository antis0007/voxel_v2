//! Host-owned world border, which the rules encoding leaves out.
use anyhow::{Context, Result};

/// Host to every joiner: border radius in metres, `u32` LE, 0 = none.
pub const BORDER: u8 = 29;

pub fn border_message(metres: u32) -> Vec<u8> {
    let mut bytes = vec![BORDER];
    bytes.extend_from_slice(&metres.to_le_bytes());
    bytes
}

pub fn decode_border(body: &[u8]) -> Result<u32> {
    let raw: [u8; 4] = body.try_into().context("invalid world border")?;
    Ok(u32::from_le_bytes(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_border_round_trips() {
        for metres in [0, 200, u32::MAX] {
            let bytes = border_message(metres);
            assert_eq!(bytes[0], BORDER);
            assert_eq!(decode_border(&bytes[1..]).unwrap(), metres);
        }
        assert!(decode_border(&[1, 2, 3]).is_err());
    }
}
