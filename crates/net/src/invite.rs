//! Short invite codes for a host's IPv4 address. Seven bytes (address, port, checksum) in Crockford base32.

use std::{fmt, net::{Ipv4Addr, SocketAddrV4}};

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CHARACTERS: usize = 12;

#[derive(Debug, PartialEq, Eq)]
pub enum InviteError {
    Length(usize),
    Character(char),
    Checksum,
    Port,
}

impl fmt::Display for InviteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(found) => write!(f, "invite code has {found} characters, expected {CHARACTERS}"),
            Self::Character(c) => write!(f, "invite code has an invalid character '{c}'"),
            Self::Checksum => f.write_str("invite code does not match; check for a typo"),
            Self::Port => f.write_str("invite code has port 0"),
        }
    }
}

impl std::error::Error for InviteError {}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_mul(31).wrapping_add(b))
}

/// Encodes a host address as `XXXX-XXXX-XXXX`.
pub fn encode(address: SocketAddrV4) -> String {
    let mut bytes = [0u8; 7];
    bytes[..4].copy_from_slice(&address.ip().octets());
    bytes[4..6].copy_from_slice(&address.port().to_be_bytes());
    bytes[6] = checksum(&bytes[..6]);
    // 56 data bits are left-aligned in 60 bits, so the last character carries 4 padding zeros.
    let value = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b)) << 4;
    let mut code = String::with_capacity(CHARACTERS + 2);
    for index in 0..CHARACTERS {
        if index > 0 && index % 4 == 0 {
            code.push('-');
        }
        let digit = (value >> (55 - 5 * index)) & 31;
        code.push(char::from(ALPHABET[digit as usize]));
    }
    code
}

/// Decodes a code. Case, spaces and dashes are ignored; O reads as 0, I and L as 1.
pub fn decode(code: &str) -> Result<SocketAddrV4, InviteError> {
    let mut value = 0u64;
    let mut count = 0usize;
    for c in code.chars().filter(|c| !matches!(c, '-' | ' ')) {
        let digit = match c.to_ascii_uppercase() {
            'O' => 0,
            'I' | 'L' => 1,
            upper => ALPHABET.iter().position(|&a| char::from(a) == upper).ok_or(InviteError::Character(c))? as u8,
        };
        value = (value << 5) | u64::from(digit);
        count += 1;
    }
    if count != CHARACTERS {
        return Err(InviteError::Length(count));
    }
    value >>= 4;
    let mut bytes = [0u8; 7];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (value >> (8 * (6 - index))) as u8;
    }
    if checksum(&bytes[..6]) != bytes[6] {
        return Err(InviteError::Checksum);
    }
    let port = u16::from_be_bytes([bytes[4], bytes[5]]);
    if port == 0 {
        return Err(InviteError::Port);
    }
    Ok(SocketAddrV4::new(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(a: u8, b: u8, c: u8, d: u8, port: u16) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), port)
    }

    #[test]
    fn round_trips_addresses_across_the_range() {
        for addr in [
            address(192, 168, 1, 20, 27020),
            address(10, 0, 0, 1, 1),
            address(0, 0, 0, 0, 65535),
            address(255, 255, 255, 255, 27020),
        ] {
            let code = encode(addr);
            assert_eq!(code.len(), 14, "{code}");
            assert_eq!(decode(&code), Ok(addr), "{code}");
        }
    }

    #[test]
    fn code_has_four_character_groups_from_the_alphabet() {
        let code = encode(address(192, 168, 1, 20, 27020));
        let groups: Vec<_> = code.split('-').collect();
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().all(|g| g.len() == 4));
        assert!(code.chars().filter(|c| *c != '-').all(|c| ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn decoding_ignores_case_spaces_and_dashes() {
        let addr = address(192, 168, 1, 20, 27020);
        let code = encode(addr);
        assert_eq!(decode(&code.to_lowercase()), Ok(addr));
        assert_eq!(decode(&code.replace('-', " ")), Ok(addr));
        assert_eq!(decode(&code.replace('-', "")), Ok(addr));
    }

    #[test]
    fn crockford_look_alikes_read_as_digits() {
        let code = encode(address(192, 168, 1, 20, 27020));
        let with_o = code.replace('0', "O").replace('1', "I");
        assert_eq!(decode(&with_o), Ok(address(192, 168, 1, 20, 27020)));
    }

    #[test]
    fn a_mistyped_character_fails_the_checksum() {
        let code = encode(address(192, 168, 1, 20, 27020));
        let first = code.chars().next().unwrap();
        let other = if first == '2' { '3' } else { '2' };
        let typo = format!("{other}{}", &code[1..]);
        assert_eq!(decode(&typo), Err(InviteError::Checksum));
    }

    #[test]
    fn wrong_length_and_bad_characters_are_named() {
        assert_eq!(decode("ABCD-EFGH"), Err(InviteError::Length(8)));
        assert_eq!(decode("ABCD-EFGH-JK@Z"), Err(InviteError::Character('@')));
    }

    #[test]
    fn port_zero_is_refused() {
        // Build a valid-looking code with port 0 by hand: the checksum must still hold.
        let mut bytes = [192u8, 168, 1, 20, 0, 0, 0];
        bytes[6] = checksum(&bytes[..6]);
        let value = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b)) << 4;
        let code: String = (0..CHARACTERS)
            .map(|i| char::from(ALPHABET[((value >> (55 - 5 * i)) & 31) as usize]))
            .collect();
        assert_eq!(decode(&code), Err(InviteError::Port));
    }
}
