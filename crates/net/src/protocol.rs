//! Explicit little-endian wire format; no unbounded peer-controlled allocations.
use std::fmt;

pub const VERSION: u16 = 14;
pub const MAX_PLAYERS: usize = 16;
pub const MAX_PACKET: usize = 1024;
pub const TICK_HZ: u16 = 30;
/// Longest name on the wire, in bytes.
pub const MAX_NAME_BYTES: usize = 32;
/// Longest chat or command line on the wire, in bytes.
pub const MAX_TEXT_BYTES: usize = 240;
pub type PlayerId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldIdentity {
    pub seed: u64,
    pub generator: u32,
    /// Physical cell size in millimetres.
    pub cell_mm: u16,
    /// Digest of gameplay content and simulation rules, supplied by the game.
    pub content: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    pub sequence: u64,
    pub tick: u64,
    pub movement: [i16; 2],
    pub look: [i16; 2],
    pub buttons: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packet {
    /// Bounded application payload, accepted only after the compatibility handshake.
    Data(Vec<u8>),
    Hello(WorldIdentity),
    Welcome {
        player: PlayerId,
        tick: u64,
        /// Random per connection; the client signs it in `Introduce`.
        challenge: [u8; 32],
    },
    Input(Input),
    /// Full roster each tick; absent inputs mean neutral, never repeat held actions.
    Frame {
        tick: u64,
        players: Vec<(PlayerId, Option<Input>)>,
    },
    /// A player states the name they want. The host answers with `Roster`.
    Introduce {
        name: String,
        /// Ed25519 public key; rights follow its fingerprint, not the name.
        key: [u8; 32],
        /// `identity::PlayerKey::sign_introduce` over the welcome challenge.
        signature: [u8; 64],
    },
    /// Names for every joined player, sent when the roster changes.
    Roster { players: Vec<(PlayerId, String)> },
    /// Host to clients. `from` is zero for server notices.
    Chat { kind: ChatKind, from: PlayerId, text: String },
    /// Client to host. A leading slash is a command; see `crate::commands`.
    Command { text: String },
    Describe,
    World { world: WorldIdentity, kind: u8 },
}

/// How a chat line is presented. Kept explicit so a client cannot style a
/// server notice as player chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatKind {
    Say,
    Action,
    Whisper,
    System,
}

impl ChatKind {
    fn code(self) -> u8 {
        match self {
            Self::Say => 0,
            Self::Action => 1,
            Self::Whisper => 2,
            Self::System => 3,
        }
    }

    fn from_code(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Say),
            1 => Ok(Self::Action),
            2 => Ok(Self::Whisper),
            3 => Ok(Self::System),
            _ => Err(ProtocolError("unknown chat kind")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolError(pub &'static str);
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for ProtocolError {}

pub(crate) fn validate_world_kind(kind: u8) -> Result<(), ProtocolError> {
    // Planet uses the same wire code in direct discovery and lobby listings.
    if matches!(kind, 0..=2 | 4) {
        Ok(())
    } else {
        Err(ProtocolError("invalid world kind"))
    }
}

impl Packet {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut out = Vec::with_capacity(MAX_PACKET);
        out.extend_from_slice(b"VXMP");
        out.extend_from_slice(&VERSION.to_le_bytes());
        match self {
            Self::Describe => out.push(10),
            Self::World { world, kind } => {
                validate_world_kind(*kind)?;
                out.push(11); out.push(*kind);
                out.extend_from_slice(&world.seed.to_le_bytes());
                out.extend_from_slice(&world.generator.to_le_bytes());
                out.extend_from_slice(&world.cell_mm.to_le_bytes());
                out.extend_from_slice(&world.content);
            }
            Self::Data(bytes) => {
                if bytes.len() > 900 {
                    return Err(ProtocolError("application payload too large"));
                }
                out.push(5);
                out.extend_from_slice(bytes);
            }
            Self::Hello(world) => {
                out.push(1);
                out.extend_from_slice(&world.seed.to_le_bytes());
                out.extend_from_slice(&world.generator.to_le_bytes());
                out.extend_from_slice(&world.cell_mm.to_le_bytes());
                out.extend_from_slice(&world.content);
            }
            Self::Welcome { player, tick, challenge } => {
                out.push(2);
                out.extend_from_slice(&player.to_le_bytes());
                out.extend_from_slice(&tick.to_le_bytes());
                out.extend_from_slice(challenge);
            }
            Self::Input(input) => {
                out.push(3);
                write_input(&mut out, input)?;
            }
            Self::Frame { tick, players } => {
                if players.len() > MAX_PLAYERS {
                    return Err(ProtocolError("roster too large"));
                }
                out.push(4);
                out.extend_from_slice(&tick.to_le_bytes());
                out.push(players.len() as u8);
                // Presence is one bit per roster slot, so absent players cost
                // nothing beyond their bit. The frame tick is already on the
                // wire, so each present input carries only its own fields.
                let mut mask: u16 = 0;
                let mut previous = 0;
                for (slot, (id, input)) in players.iter().enumerate() {
                    if *id <= previous {
                        return Err(ProtocolError("invalid roster order"));
                    }
                    previous = *id;
                    if input.is_some() {
                        mask |= 1 << slot;
                    }
                }
                out.extend_from_slice(&mask.to_le_bytes());
                for (id, input) in players {
                    out.extend_from_slice(&id.to_le_bytes());
                    if let Some(input) = input {
                        write_input_body(&mut out, input)?;
                    }
                }
            }
            Self::Introduce { name, key, signature } => {
                out.push(6);
                write_text(&mut out, name, MAX_NAME_BYTES)?;
                out.extend_from_slice(key);
                out.extend_from_slice(signature);
            }
            Self::Roster { players } => {
                if players.len() > MAX_PLAYERS {
                    return Err(ProtocolError("roster too large"));
                }
                out.push(7);
                out.push(players.len() as u8);
                let mut previous = 0;
                for (id, name) in players {
                    if *id <= previous {
                        return Err(ProtocolError("invalid roster order"));
                    }
                    previous = *id;
                    out.extend_from_slice(&id.to_le_bytes());
                    write_text(&mut out, name, MAX_NAME_BYTES)?;
                }
            }
            Self::Chat { kind, from, text } => {
                out.push(8);
                out.push(kind.code());
                out.extend_from_slice(&from.to_le_bytes());
                write_text(&mut out, text, MAX_TEXT_BYTES)?;
            }
            Self::Command { text } => {
                out.push(9);
                write_text(&mut out, text, MAX_TEXT_BYTES)?;
            }
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_PACKET {
            return Err(ProtocolError("packet too large"));
        }
        let mut r = Reader(bytes);
        if r.take::<4>()? != *b"VXMP" || u16::from_le_bytes(r.take()?) != VERSION {
            return Err(ProtocolError("protocol mismatch"));
        }
        let packet = match r.take::<1>()?[0] {
            5 => {
                if r.0.len() > 900 {
                    return Err(ProtocolError("application payload too large"));
                }
                let bytes = r.0.to_vec();
                r.0 = &[];
                Self::Data(bytes)
            }
            10 => Self::Describe,
            11 => {
                let kind=r.take::<1>()?[0];
                validate_world_kind(kind)?;
                Self::World {kind,world:WorldIdentity {
                    seed:u64::from_le_bytes(r.take()?),generator:u32::from_le_bytes(r.take()?),
                    cell_mm:u16::from_le_bytes(r.take()?),content:r.take()?,
                }}
            }
            1 => Self::Hello(WorldIdentity {
                seed: u64::from_le_bytes(r.take()?),
                generator: u32::from_le_bytes(r.take()?),
                cell_mm: u16::from_le_bytes(r.take()?),
                content: r.take()?,
            }),
            2 => {
                let player = u32::from_le_bytes(r.take()?);
                if player == 0 {
                    return Err(ProtocolError("invalid player"));
                }
                Self::Welcome {
                    player,
                    tick: u64::from_le_bytes(r.take()?),
                    challenge: r.take()?,
                }
            }
            3 => Self::Input(r.input()?),
            4 => {
                let tick = u64::from_le_bytes(r.take()?);
                let count = r.take::<1>()?[0] as usize;
                if count > MAX_PLAYERS {
                    return Err(ProtocolError("roster too large"));
                }
                let mask = u16::from_le_bytes(r.take()?);
                if count < MAX_PLAYERS && mask >> count != 0 {
                    return Err(ProtocolError("presence beyond roster"));
                }
                let mut players = Vec::with_capacity(count);
                let mut previous = 0;
                for slot in 0..count {
                    let id = u32::from_le_bytes(r.take()?);
                    if id <= previous {
                        return Err(ProtocolError("invalid roster order"));
                    }
                    previous = id;
                    let input = if mask & (1u16 << slot) != 0 {
                        Some(r.input_body(tick)?)
                    } else {
                        None
                    };
                    players.push((id, input));
                }
                Self::Frame { tick, players }
            }
            6 => Self::Introduce {
                name: r.text(MAX_NAME_BYTES)?,
                key: r.take()?,
                signature: r.take()?,
            },
            7 => {
                let count = r.take::<1>()?[0] as usize;
                if count > MAX_PLAYERS {
                    return Err(ProtocolError("roster too large"));
                }
                let mut players = Vec::with_capacity(count);
                let mut previous = 0;
                for _ in 0..count {
                    let id = u32::from_le_bytes(r.take()?);
                    if id <= previous {
                        return Err(ProtocolError("invalid roster order"));
                    }
                    previous = id;
                    players.push((id, r.text(MAX_NAME_BYTES)?));
                }
                Self::Roster { players }
            }
            8 => {
                let kind = ChatKind::from_code(r.take::<1>()?[0])?;
                let from = u32::from_le_bytes(r.take()?);
                Self::Chat { kind, from, text: r.text(MAX_TEXT_BYTES)? }
            }
            9 => Self::Command { text: r.text(MAX_TEXT_BYTES)? },
            _ => return Err(ProtocolError("unknown packet")),
        };
        if !r.0.is_empty() {
            return Err(ProtocolError("trailing data"));
        }
        Ok(packet)
    }
}

fn write_input_body(out: &mut Vec<u8>, input: &Input) -> Result<(), ProtocolError> {
    // The frame tick is already on the wire, so a roster input carries only its
    // own fields. Sequence and buttons are narrowed to the widths the host
    // actually validates.
    let sequence = u32::try_from(input.sequence)
        .map_err(|_| ProtocolError("input sequence beyond u32"))?;
    out.extend_from_slice(&sequence.to_le_bytes());
    for value in input.movement.into_iter().chain(input.look) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    if input.buttons & !0x000f != 0 {
        return Err(ProtocolError("invalid buttons"));
    }
    out.push(input.buttons as u8);
    Ok(())
}

fn write_input(out: &mut Vec<u8>, input: &Input) -> Result<(), ProtocolError> {
    out.extend_from_slice(&input.tick.to_le_bytes());
    write_input_body(out, input)
}

/// UTF-8 with a `u16` byte length, bounded before anything is allocated.
fn write_text(out: &mut Vec<u8>, text: &str, limit: usize) -> Result<(), ProtocolError> {
    if text.len() > limit || text.len() > u16::MAX as usize {
        return Err(ProtocolError("text too long"));
    }
    out.extend_from_slice(&(text.len() as u16).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
    Ok(())
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        let bytes = self.0.get(..N).ok_or(ProtocolError("truncated packet"))?;
        let result = bytes
            .try_into()
            .map_err(|_| ProtocolError("truncated packet"))?;
        self.0 = &self.0[N..];
        Ok(result)
    }
    fn text(&mut self, limit: usize) -> Result<String, ProtocolError> {
        let len = u16::from_le_bytes(self.take()?) as usize;
        if len > limit {
            return Err(ProtocolError("text too long"));
        }
        let bytes = self.0.get(..len).ok_or(ProtocolError("truncated packet"))?;
        let text = std::str::from_utf8(bytes).map_err(|_| ProtocolError("text is not utf-8"))?;
        self.0 = &self.0[len..];
        Ok(text.to_owned())
    }
    fn input_body(&mut self, tick: u64) -> Result<Input, ProtocolError> {
        let sequence = u32::from_le_bytes(self.take()?) as u64;
        let input = Input {
            sequence,
            tick,
            movement: [
                i16::from_le_bytes(self.take()?),
                i16::from_le_bytes(self.take()?),
            ],
            look: [
                i16::from_le_bytes(self.take()?),
                i16::from_le_bytes(self.take()?),
            ],
            buttons: self.take::<1>()?[0] as u16,
        };
        if input.sequence == 0 || input.buttons & !0x000f != 0 {
            return Err(ProtocolError("invalid input"));
        }
        Ok(input)
    }

    fn input(&mut self) -> Result<Input, ProtocolError> {
        let tick = u64::from_le_bytes(self.take()?);
        self.input_body(tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(sequence: u64, tick: u64, buttons: u16) -> Input {
        Input {
            sequence,
            tick,
            movement: [120, -80],
            look: [42, -11],
            buttons,
        }
    }

    #[test]
    fn roster_frame_round_trips_and_keeps_absent_players() {
        let players: Vec<(PlayerId, Option<Input>)> = (1..=MAX_PLAYERS as u32)
            .map(|id| {
                let input = (id % 3 != 0).then(|| intent(id as u64, 900, 0b0101));
                (id, input)
            })
            .collect();
        let packet = Packet::Frame {
            tick: 900,
            players: players.clone(),
        };
        let bytes = packet.encode().expect("encode");
        assert_eq!(Packet::decode(&bytes).expect("decode"), packet);
    }

    #[test]
    fn absent_players_cost_only_their_id() {
        let present: Vec<(PlayerId, Option<Input>)> =
            (1..=MAX_PLAYERS as u32).map(|id| (id, None)).collect();
        let absent = Packet::Frame { tick: 1, players: present }.encode().expect("encode");
        let full: Vec<(PlayerId, Option<Input>)> = (1..=MAX_PLAYERS as u32)
            .map(|id| (id, Some(intent(id as u64, 1, 0))))
            .collect();
        let loaded = Packet::Frame { tick: 1, players: full }.encode().expect("encode");

        // 7 byte header, 8 byte tick, 1 byte count, 2 byte presence mask.
        assert_eq!(absent.len(), 18 + MAX_PLAYERS * 4);
        // A present input adds 13 bytes: u32 sequence, four i16 axes, u8 buttons.
        assert_eq!(loaded.len(), 18 + MAX_PLAYERS * (4 + 13));
        // The previous layout cost 31 bytes per present player (512 for a full
        // roster), so the narrowed form must stay well under that.
        assert!(loaded.len() < 512, "full roster frame grew to {}", loaded.len());
    }

    #[test]
    fn presence_beyond_the_roster_is_refused() {
        let players = vec![(1u32, Some(intent(1, 5, 0)))];
        let mut bytes = Packet::Frame { tick: 5, players }.encode().expect("encode");
        // The mask sits after the tag, tick and count.
        let mask_at = 4 + 2 + 1 + 8 + 1;
        bytes[mask_at..mask_at + 2].copy_from_slice(&0b10u16.to_le_bytes());
        assert_eq!(Packet::decode(&bytes), Err(ProtocolError("presence beyond roster")));
    }

    #[test]
    fn sequence_wider_than_the_wire_is_refused() {
        let input = intent(u32::MAX as u64 + 1, 7, 0);
        assert_eq!(
            Packet::Input(input).encode(),
            Err(ProtocolError("input sequence beyond u32"))
        );
    }

    #[test]
    fn reserved_buttons_are_refused_both_ways() {
        assert_eq!(
            Packet::Input(intent(1, 7, 0x0010)).encode(),
            Err(ProtocolError("invalid buttons"))
        );
        let mut bytes = Packet::Input(intent(1, 7, 0)).encode().expect("encode");
        *bytes.last_mut().expect("buttons byte") = 0x20;
        assert_eq!(Packet::decode(&bytes), Err(ProtocolError("invalid input")));
    }

    #[test]
    fn names_chat_and_commands_round_trip() {
        let roster = Packet::Roster {
            players: vec![(1, "Ana".to_owned()), (2, "Bo (2)".to_owned())],
        };
        assert_eq!(Packet::decode(&roster.encode().expect("encode")).expect("decode"), roster);

        let chat = Packet::Chat {
            kind: ChatKind::Whisper,
            from: 7,
            text: "good run".to_owned(),
        };
        assert_eq!(Packet::decode(&chat.encode().expect("encode")).expect("decode"), chat);

        let command = Packet::Command { text: "/time set 0.5".to_owned() };
        assert_eq!(
            Packet::decode(&command.encode().expect("encode")).expect("decode"),
            command
        );

        let introduce = Packet::Introduce { name: "Ana".to_owned(), key: [1; 32], signature: [2; 64] };
        assert_eq!(
            Packet::decode(&introduce.encode().expect("encode")).expect("decode"),
            introduce
        );
    }

    #[test]
    fn text_limits_are_enforced_before_the_wire() {
        assert_eq!(
            Packet::Chat {
                kind: ChatKind::Say,
                from: 1,
                text: "x".repeat(MAX_TEXT_BYTES + 1),
            }
            .encode(),
            Err(ProtocolError("text too long"))
        );
        assert_eq!(
            Packet::Introduce { name: "x".repeat(MAX_NAME_BYTES + 1), key: [0; 32], signature: [0; 64] }.encode(),
            Err(ProtocolError("text too long"))
        );
        let mut bytes = Packet::Chat { kind: ChatKind::Say, from: 1, text: "hi".into() }
            .encode()
            .expect("encode");
        // Claim more text than the packet holds.
        let len_at = bytes.len() - 2 - 2;
        bytes[len_at..len_at + 2].copy_from_slice(&(MAX_TEXT_BYTES as u16 + 1).to_le_bytes());
        assert_eq!(Packet::decode(&bytes), Err(ProtocolError("text too long")));
    }
}

