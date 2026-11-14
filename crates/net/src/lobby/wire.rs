//! Lobby directory and relay messages. A frame is a u32 little-endian body length, then the body.
//! The body is a one-byte tag and the fields in order; text is a u16 length and UTF-8.

use crate::protocol::ProtocolError;
use std::io::{self, Read, Write};

pub const PROTOCOL: u16 = 3;
/// Protocol 2 has the same relay and admission messages, without host/listing MOTDs.
pub const LEGACY_PROTOCOL: u16 = 2;
pub const MAX_BODY: usize = 64 * 1024;
pub const MAX_NAME: usize = 48;
pub const MAX_MOTD: usize = 96;
pub const MAX_REASON: usize = 96;
pub const MAX_CODE: usize = 16;
pub const MAX_QUERY: usize = 64;
pub const MAX_PAGE: u8 = 32;
pub const MAX_PLAYERS: u8 = 16;

pub type Digest = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Survival,
    Creative,
}

impl Mode {
    fn code(self) -> u8 {
        match self {
            Self::Survival => 1,
            Self::Creative => 2,
        }
    }

    fn from_code(code: u8) -> Result<Self, ProtocolError> {
        match code {
            1 => Ok(Self::Survival),
            2 => Ok(Self::Creative),
            _ => Err(ProtocolError("unknown lobby mode")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Players,
    Newest,
    Name,
}

impl Sort {
    fn code(self) -> u8 {
        match self {
            Self::Players => 0,
            Self::Newest => 1,
            Self::Name => 2,
        }
    }

    fn from_code(code: u8) -> Result<Self, ProtocolError> {
        match code {
            0 => Ok(Self::Players),
            1 => Ok(Self::Newest),
            2 => Ok(Self::Name),
            _ => Err(ProtocolError("unknown lobby sort")),
        }
    }
}

/// A browse request. Empty `text` matches every session. `mode` and `version` of `None` match any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub mode: Option<Mode>,
    pub version: Option<u32>,
    pub include_full: bool,
    pub sort: Sort,
    pub offset: u16,
    pub limit: u8,
}

/// What a joiner needs to build the host's world: the seed, the world kind and the voxel scale.
/// The generator version travels as `version`, and the executable fingerprint as `digest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldParams {
    pub seed: u32,
    pub kind: u8,
    pub scale: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostInfo {
    pub name: String,
    pub world: String,
    pub mode: Mode,
    pub version: u32,
    pub digest: Digest,
    pub max_players: u8,
    pub unlisted: bool,
    pub world_params: WorldParams,
    /// Server message, shown in a listing the way a MOTD is.
    pub motd: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub code: String,
    pub name: String,
    pub world: String,
    pub mode: Mode,
    pub version: u32,
    pub players: u8,
    pub max_players: u8,
    pub age_secs: u32,
    /// Server message, carried so a listing reads like a server browser.
    pub motd: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// Client opens a connection and states the protocol it speaks.
    Hello { protocol: u16 },
    Ready { protocol: u16 },
    Browse(Query),
    Listing { total: u32, entries: Vec<Entry> },
    Host(HostInfo),
    HostDirect { info: HostInfo, port: u16 },
    HostSteam { info: HostInfo, identity: u64 },
    Presence { players: u8 },
    Hosted { code: String },
    /// The lobby refuses a build whose version or digest differs from the host's.
    Join { code: String, version: u32, digest: Digest },
    Joined { peer: u32, version: u32, digest: Digest, world: WorldParams },
    Direct { address: std::net::SocketAddr, version: u32, digest: Digest, world: WorldParams },
    Steam { identity: u64, version: u32, digest: Digest, world: WorldParams },
    Ping,
    /// Host to one peer through the lobby.
    ToPeer { peer: u32, bytes: Vec<u8> },
    /// Host removes a peer. The lobby closes that peer's connection.
    Kick { peer: u32 },
    /// Lobby to host, tagged with the peer that sent the bytes.
    FromPeer { peer: u32, bytes: Vec<u8> },
    PeerJoined { peer: u32 },
    PeerLeft { peer: u32 },
    /// Client to host through the lobby.
    ToHost { bytes: Vec<u8> },
    FromHost { bytes: Vec<u8> },
    Refused { reason: String },
    Closed { reason: String },
}

struct Writer(Vec<u8>);

impl Writer {
    fn text(&mut self, text: &str, max: usize) -> Result<(), ProtocolError> {
        if text.len() > max || text.chars().any(char::is_control) {
            return Err(ProtocolError("lobby text out of bounds"));
        }
        self.0.extend_from_slice(&(text.len() as u16).to_le_bytes());
        self.0.extend_from_slice(text.as_bytes());
        Ok(())
    }

    fn data(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        self.0.extend_from_slice(bytes);
    }

    fn digest(&mut self, digest: &Digest) {
        self.0.extend_from_slice(digest);
    }

    fn params(&mut self, world: &WorldParams) {
        self.0.extend_from_slice(&world.seed.to_le_bytes());
        self.0.push(world.kind);
        self.0.push(world.scale);
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ProtocolError> {
        if self.0.len() < count {
            return Err(ProtocolError("truncated lobby message"));
        }
        let (head, tail) = self.0.split_at(count);
        self.0 = tail;
        Ok(head)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn text(&mut self, max: usize) -> Result<String, ProtocolError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(ProtocolError("lobby text too long"));
        }
        let raw = self.take(len)?;
        let text = std::str::from_utf8(raw).map_err(|_| ProtocolError("lobby text is not UTF-8"))?;
        if text.chars().any(char::is_control) {
            return Err(ProtocolError("lobby text has control characters"));
        }
        Ok(text.to_owned())
    }

    fn data(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let len = self.u32()? as usize;
        Ok(self.take(len)?.to_vec())
    }

    fn digest(&mut self) -> Result<Digest, ProtocolError> {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(self.take(32)?);
        Ok(digest)
    }

    fn params(&mut self) -> Result<WorldParams, ProtocolError> {
        Ok(WorldParams { seed: self.u32()?, kind: self.u8()?, scale: self.u8()? })
    }

    fn finish(&self) -> Result<(), ProtocolError> {
        if self.0.is_empty() { Ok(()) } else { Err(ProtocolError("trailing bytes in lobby message")) }
    }
}

/// Encodes a message body. Fails when a field breaks the bounds that the decoder enforces.
pub fn encode(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    encode_version(message, PROTOCOL)
}

pub fn encode_version(message: &Message, protocol: u16) -> Result<Vec<u8>, ProtocolError> {
    check_version(protocol)?;
    let mut w = Writer(Vec::new());
    match message {
        Message::Hello { protocol } => {
            w.0.push(1);
            w.0.extend_from_slice(&protocol.to_le_bytes());
        }
        Message::Browse(query) => {
            w.0.push(2);
            w.text(&query.text, MAX_QUERY)?;
            w.0.push(query.mode.map_or(0, Mode::code));
            w.0.extend_from_slice(&query.version.unwrap_or(0).to_le_bytes());
            w.0.push(u8::from(query.include_full));
            w.0.push(query.sort.code());
            w.0.extend_from_slice(&query.offset.to_le_bytes());
            if query.limit > MAX_PAGE {
                return Err(ProtocolError("lobby page too large"));
            }
            w.0.push(query.limit);
        }
        Message::Host(info) | Message::HostDirect { info, .. } | Message::HostSteam { info, .. } => {
            w.0.push(match message {
                Message::HostDirect { .. } => 10,
                Message::HostSteam { .. } => 12,
                _ => 3,
            });
            w.text(&info.name, MAX_NAME)?;
            w.text(&info.world, MAX_NAME)?;
            w.0.push(info.mode.code());
            w.0.extend_from_slice(&info.version.to_le_bytes());
            w.digest(&info.digest);
            w.0.push(info.max_players);
            w.0.push(u8::from(info.unlisted));
            w.params(&info.world_params);
            if protocol >= 3 { w.text(&info.motd, MAX_MOTD)?; }
            if let Message::HostDirect { port, .. } = message {
                if *port == 0 { return Err(ProtocolError("invalid direct UDP port")); }
                w.0.extend_from_slice(&port.to_le_bytes());
            }
            if let Message::HostSteam { identity, .. } = message {
                if *identity == 0 { return Err(ProtocolError("invalid Steam identity")); }
                w.0.extend_from_slice(&identity.to_le_bytes());
            }
        }
        Message::Presence { players } => w.0.extend_from_slice(&[11, *players]),
        Message::Join { code, version, digest } => {
            w.0.push(4);
            w.text(code, MAX_CODE)?;
            w.0.extend_from_slice(&version.to_le_bytes());
            w.digest(digest);
        }
        Message::Ping => w.0.push(6),
        Message::ToPeer { peer, bytes } => {
            w.0.push(7);
            w.0.extend_from_slice(&peer.to_le_bytes());
            w.data(bytes);
        }
        Message::ToHost { bytes } => {
            w.0.push(8);
            w.data(bytes);
        }
        Message::Kick { peer } => {
            w.0.push(9);
            w.0.extend_from_slice(&peer.to_le_bytes());
        }
        Message::Ready { protocol } => {
            w.0.push(64);
            w.0.extend_from_slice(&protocol.to_le_bytes());
        }
        Message::Listing { total, entries } => {
            w.0.push(65);
            w.0.extend_from_slice(&total.to_le_bytes());
            w.0.push(u8::try_from(entries.len()).map_err(|_| ProtocolError("too many lobby entries"))?);
            for entry in entries {
                w.text(&entry.code, MAX_CODE)?;
                w.text(&entry.name, MAX_NAME)?;
                w.text(&entry.world, MAX_NAME)?;
                w.0.push(entry.mode.code());
                w.0.extend_from_slice(&entry.version.to_le_bytes());
                w.0.push(entry.players);
                w.0.push(entry.max_players);
                w.0.extend_from_slice(&entry.age_secs.to_le_bytes());
                if protocol >= 3 { w.text(&entry.motd, MAX_MOTD)?; }
            }
        }
        Message::Hosted { code } => {
            w.0.push(66);
            w.text(code, MAX_CODE)?;
        }
        Message::Joined { peer, version, digest, world } => {
            w.0.push(67);
            w.0.extend_from_slice(&peer.to_le_bytes());
            w.0.extend_from_slice(&version.to_le_bytes());
            w.digest(digest);
            w.params(world);
        }
        Message::Direct { address, version, digest, world } => {
            w.0.push(74);
            w.text(&address.to_string(), 64)?;
            w.0.extend_from_slice(&version.to_le_bytes());
            w.digest(digest);
            w.params(world);
        }
        Message::Steam { identity, version, digest, world } => {
            w.0.push(75);
            w.0.extend_from_slice(&identity.to_le_bytes());
            w.0.extend_from_slice(&version.to_le_bytes());
            w.digest(digest);
            w.params(world);
        }
        Message::PeerJoined { peer } => {
            w.0.push(68);
            w.0.extend_from_slice(&peer.to_le_bytes());
        }
        Message::PeerLeft { peer } => {
            w.0.push(69);
            w.0.extend_from_slice(&peer.to_le_bytes());
        }
        Message::FromPeer { peer, bytes } => {
            w.0.push(70);
            w.0.extend_from_slice(&peer.to_le_bytes());
            w.data(bytes);
        }
        Message::FromHost { bytes } => {
            w.0.push(71);
            w.data(bytes);
        }
        Message::Refused { reason } => {
            w.0.push(72);
            w.text(reason, MAX_REASON)?;
        }
        Message::Closed { reason } => {
            w.0.push(73);
            w.text(reason, MAX_REASON)?;
        }
    }
    if w.0.len() > MAX_BODY {
        return Err(ProtocolError("lobby message exceeds the frame limit"));
    }
    Ok(w.0)
}

/// Decodes one message body. Rejects unknown tags, bad bounds and trailing bytes.
pub fn decode(body: &[u8]) -> Result<Message, ProtocolError> {
    decode_version(body, PROTOCOL)
}

pub fn decode_version(body: &[u8], protocol: u16) -> Result<Message, ProtocolError> {
    check_version(protocol)?;
    let mut r = Reader(body);
    let message = match r.u8()? {
        1 => Message::Hello { protocol: r.u16()? },
        2 => {
            let text = r.text(MAX_QUERY)?;
            let mode = match r.u8()? {
                0 => None,
                code => Some(Mode::from_code(code)?),
            };
            let version = match r.u32()? {
                0 => None,
                version => Some(version),
            };
            let include_full = r.u8()? != 0;
            let sort = Sort::from_code(r.u8()?)?;
            let offset = r.u16()?;
            let limit = r.u8()?;
            if limit > MAX_PAGE {
                return Err(ProtocolError("lobby page too large"));
            }
            Message::Browse(Query { text, mode, version, include_full, sort, offset, limit })
        }
        tag @ (3 | 10 | 12) => {
            let info = HostInfo {
            name: r.text(MAX_NAME)?,
            world: r.text(MAX_NAME)?,
            mode: Mode::from_code(r.u8()?)?,
            version: r.u32()?,
            digest: r.digest()?,
            max_players: {
                let max = r.u8()?;
                if !(1..=MAX_PLAYERS).contains(&max) {
                    return Err(ProtocolError("lobby player limit out of range"));
                }
                max
            },
            unlisted: r.u8()? != 0,
            world_params: r.params()?,
            motd: if protocol >= 3 { r.text(MAX_MOTD)? } else { String::new() },
            };
            if tag == 10 {
                let port = r.u16()?;
                if port == 0 { return Err(ProtocolError("invalid direct UDP port")); }
                Message::HostDirect { info, port }
            } else if tag == 12 {
                let identity = u64::from(r.u32()?) | (u64::from(r.u32()?) << 32);
                if identity == 0 { return Err(ProtocolError("invalid Steam identity")); }
                Message::HostSteam { info, identity }
            } else {
                Message::Host(info)
            }
        }
        4 => Message::Join { code: r.text(MAX_CODE)?, version: r.u32()?, digest: r.digest()? },
        6 => Message::Ping,
        7 => Message::ToPeer { peer: r.u32()?, bytes: r.data()? },
        8 => Message::ToHost { bytes: r.data()? },
        9 => Message::Kick { peer: r.u32()? },
        11 => Message::Presence { players: r.u8()? },
        64 => Message::Ready { protocol: r.u16()? },
        65 => {
            let total = r.u32()?;
            let count = r.u8()?;
            if count > MAX_PAGE {
                return Err(ProtocolError("lobby page too large"));
            }
            let mut entries = Vec::with_capacity(usize::from(count));
            for _ in 0..count {
                entries.push(Entry {
                    code: r.text(MAX_CODE)?,
                    name: r.text(MAX_NAME)?,
                    world: r.text(MAX_NAME)?,
                    mode: Mode::from_code(r.u8()?)?,
                    version: r.u32()?,
                    players: r.u8()?,
                    max_players: r.u8()?,
                    age_secs: r.u32()?,
                    motd: if protocol >= 3 { r.text(MAX_MOTD)? } else { String::new() },
                });
            }
            Message::Listing { total, entries }
        }
        66 => Message::Hosted { code: r.text(MAX_CODE)? },
        67 => Message::Joined { peer: r.u32()?, version: r.u32()?, digest: r.digest()?, world: r.params()? },
        68 => Message::PeerJoined { peer: r.u32()? },
        69 => Message::PeerLeft { peer: r.u32()? },
        70 => Message::FromPeer { peer: r.u32()?, bytes: r.data()? },
        71 => Message::FromHost { bytes: r.data()? },
        72 => Message::Refused { reason: r.text(MAX_REASON)? },
        73 => Message::Closed { reason: r.text(MAX_REASON)? },
        74 => Message::Direct {
            address: r.text(64)?.parse().map_err(|_| ProtocolError("invalid direct UDP address"))?,
            version: r.u32()?, digest: r.digest()?, world: r.params()?,
        },
        75 => Message::Steam {
            identity: u64::from(r.u32()?) | (u64::from(r.u32()?) << 32),
            version: r.u32()?, digest: r.digest()?, world: r.params()?,
        },
        _ => return Err(ProtocolError("unknown lobby message tag")),
    };
    r.finish()?;
    Ok(message)
}

/// Writes one length-prefixed frame. The caller flushes.
pub fn write_message(out: &mut impl Write, message: &Message) -> io::Result<()> {
    write_message_version(out, message, PROTOCOL)
}

pub fn write_message_version(out: &mut impl Write, message: &Message, protocol: u16) -> io::Result<()> {
    let body = encode_version(message, protocol).map_err(invalid)?;
    out.write_all(&(body.len() as u32).to_le_bytes())?;
    out.write_all(&body)
}

/// Reads one frame. A length above `MAX_BODY` is refused before the body is allocated.
pub fn read_message(input: &mut impl Read) -> io::Result<Message> {
    read_message_version(input, PROTOCOL)
}

pub fn read_message_version(input: &mut impl Read, protocol: u16) -> io::Result<Message> {
    check_version(protocol).map_err(invalid)?;
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_BODY {
        return Err(invalid(ProtocolError("lobby frame exceeds the limit")));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    decode_version(&body, protocol).map_err(invalid)
}

fn check_version(protocol: u16) -> Result<(), ProtocolError> {
    if matches!(protocol, LEGACY_PROTOCOL | PROTOCOL) { Ok(()) }
    else { Err(ProtocolError("unsupported lobby protocol")) }
}

fn invalid(error: ProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> Digest {
        [seed; 32]
    }

    fn samples() -> Vec<Message> {
        let entry = Entry {
            code: "ABCD-EFGH".into(),
            name: "Riverside".into(),
            world: "Hills 0.5 m".into(),
            mode: Mode::Creative,
            version: 7,
            players: 3,
            max_players: 8,
            age_secs: 120,
            motd: "quiet valley".into(),
        };
        vec![
            Message::Hello { protocol: PROTOCOL },
            Message::Ready { protocol: PROTOCOL },
            Message::Browse(Query {
                text: "river hills".into(),
                mode: Some(Mode::Survival),
                version: Some(7),
                include_full: false,
                sort: Sort::Newest,
                offset: 64,
                limit: 20,
            }),
            Message::Browse(Query {
                text: String::new(),
                mode: None,
                version: None,
                include_full: true,
                sort: Sort::Players,
                offset: 0,
                limit: 0,
            }),
            Message::Listing { total: 1, entries: vec![entry] },
            Message::Listing { total: 0, entries: vec![] },
            Message::Host(HostInfo {
                name: "Ana's build".into(),
                world: "Hills".into(),
                mode: Mode::Survival,
                version: 7,
                digest: digest(9),
                max_players: 16,
                unlisted: true,
                world_params: WorldParams { seed: 74, kind: 1, scale: 1 },
                motd: "bring timber".into(),
            }),
            Message::Hosted { code: "ABCD-EFGH".into() },
            Message::Join { code: "ABCD-EFGH".into(), version: 7, digest: digest(9) },
            Message::Joined { peer: 4, version: 7, digest: digest(9), world: WorldParams { seed: 74, kind: 1, scale: 1 } },
            Message::Ping,
            Message::ToPeer { peer: 4, bytes: vec![1, 2, 3] },
            Message::FromPeer { peer: 4, bytes: vec![] },
            Message::PeerJoined { peer: 4 },
            Message::PeerLeft { peer: 4 },
            Message::ToHost { bytes: vec![0xff; 1000] },
            Message::Kick { peer: 4 },
            Message::FromHost { bytes: vec![7] },
            Message::Refused { reason: "session is full".into() },
            Message::Closed { reason: "host left".into() },
        ]
    }

    #[test]
    fn every_message_round_trips() {
        for message in samples() {
            let body = encode(&message).expect("encode");
            assert_eq!(decode(&body), Ok(message.clone()), "{message:?}");
        }
    }

    #[test]
    fn frames_round_trip_through_a_stream() {
        let mut wire = Vec::new();
        for message in samples() {
            write_message(&mut wire, &message).expect("write");
        }
        let mut input = wire.as_slice();
        for message in samples() {
            assert_eq!(read_message(&mut input).expect("read"), message);
        }
        assert!(input.is_empty());
    }

    #[test]
    fn unknown_tags_and_trailing_bytes_are_refused() {
        assert!(decode(&[200]).is_err());
        let mut body = encode(&Message::Ping).expect("encode");
        body.push(0);
        assert_eq!(decode(&body), Err(ProtocolError("trailing bytes in lobby message")));
    }

    #[test]
    fn truncated_bodies_are_refused() {
        let body = encode(&samples()[6]).expect("encode");
        for cut in 1..body.len() {
            assert!(decode(&body[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn control_characters_and_long_names_are_refused_both_ways() {
        let bad = Message::Host(HostInfo {
            name: "bad\nname".into(),
            world: "w".into(),
            mode: Mode::Survival,
            version: 1,
            digest: digest(0),
            max_players: 4,
            unlisted: false,
            world_params: WorldParams { seed: 1, kind: 0, scale: 0 },
            motd: "fine".into(),
        });
        assert!(encode(&bad).is_err());
        let long = Message::Join { code: "x".repeat(MAX_CODE + 1), version: 1, digest: digest(0) };
        assert!(encode(&long).is_err());
        let mut body = encode(&Message::Join { code: "ABCD".into(), version: 1, digest: digest(0) }).expect("encode");
        body[1..3].copy_from_slice(&200u16.to_le_bytes());
        assert_eq!(decode(&body), Err(ProtocolError("lobby text too long")));
    }

    #[test]
    fn host_limits_and_page_sizes_are_enforced() {
        let mut info = match samples()[6].clone() {
            Message::Host(info) => info,
            _ => unreachable!(),
        };
        info.max_players = 0;
        let body = encode(&Message::Host(info.clone())).expect("encode");
        assert_eq!(decode(&body), Err(ProtocolError("lobby player limit out of range")));
        info.max_players = 4;
        let mut body = encode(&Message::Host(info)).expect("encode");
        let mode_at = 1 + 2 + "Ana's build".len() + 2 + "Hills".len();
        body[mode_at] = 9;
        assert_eq!(decode(&body), Err(ProtocolError("unknown lobby mode")));
        let page = |limit| {
            Message::Browse(Query {
                text: String::new(),
                mode: None,
                version: None,
                include_full: false,
                sort: Sort::Players,
                offset: 0,
                limit,
            })
        };
        assert_eq!(encode(&page(MAX_PAGE + 1)), Err(ProtocolError("lobby page too large")));
        let mut body = encode(&page(MAX_PAGE)).expect("encode");
        *body.last_mut().expect("limit byte") = MAX_PAGE + 1;
        assert_eq!(decode(&body), Err(ProtocolError("lobby page too large")));
    }

    #[test]
    fn oversized_frames_are_refused_before_allocation() {
        let wire = ((MAX_BODY + 1) as u32).to_le_bytes();
        let error = read_message(&mut wire.as_slice()).expect_err("oversized");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
