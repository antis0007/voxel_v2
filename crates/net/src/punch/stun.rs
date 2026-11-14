//! Minimal RFC 8489 IPv4 Binding discovery on the socket that carries play.
use std::{io, net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket},
    time::{Duration, Instant}};

const COOKIE: [u8; 4] = [0x21, 0x12, 0xa4, 0x42];

pub(super) struct Candidates {
    pub addresses: Vec<SocketAddrV4>,
    pub server: Option<SocketAddrV4>,
}

fn request() -> Option<[u8; 20]> {
    let mut bytes = [0; 20];
    bytes[1] = 1;
    bytes[4..8].copy_from_slice(&COOKIE);
    getrandom::fill(&mut bytes[8..]).ok()?;
    Some(bytes)
}

fn mapped(bytes: &[u8], request: &[u8; 20]) -> Option<SocketAddrV4> {
    if bytes.len() < 20 || bytes[..2] != [1, 1] || bytes[4..20] != request[4..] {
        return None;
    }
    let length = usize::from(u16::from_be_bytes(bytes[2..4].try_into().ok()?));
    if length % 4 != 0 { return None; }
    let mut attributes = bytes.get(20..20 + length)?;
    while attributes.len() >= 4 {
        let tag = u16::from_be_bytes(attributes[..2].try_into().ok()?);
        let size = usize::from(u16::from_be_bytes(attributes[2..4].try_into().ok()?));
        let value = attributes.get(4..4 + size)?;
        if tag == 0x0020 && size == 8 && value[..2] == [0, 1] {
            let port = u16::from_be_bytes([value[2] ^ COOKIE[0], value[3] ^ COOKIE[1]]);
            let octets: [u8; 4] = std::array::from_fn(|i| value[4 + i] ^ COOKIE[i]);
            let ip = Ipv4Addr::from(octets);
            if port == 0 || ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() {
                return None;
            }
            return Some(SocketAddrV4::new(ip, port));
        }
        attributes = attributes.get(4 + size.next_multiple_of(4)..)?;
    }
    None
}

fn binding(socket: &UdpSocket, server: SocketAddrV4) -> Option<SocketAddrV4> {
    let request = request()?;
    let mut response = [0; 1024];
    socket.set_read_timeout(Some(Duration::from_millis(120))).ok()?;
    for _ in 0..3 {
        socket.send_to(&request, server).ok()?;
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            if let Ok((count, from)) = socket.recv_from(&mut response)
                && from == server.into()
                && let Some(address) = mapped(&response[..count], &request)
            {
                return Some(address);
            }
        }
    }
    None
}

pub(super) fn keepalive(socket: &UdpSocket, server: SocketAddrV4) {
    if let Some(request) = request() { let _ = socket.send_to(&request, server); }
}

pub(super) fn discover(socket: &UdpSocket) -> io::Result<Candidates> {
    let port = socket.local_addr()?.port();
    let mut addresses = Vec::with_capacity(3);
    let settings = std::env::var("VOXEL_STUN").unwrap_or_else(|_| "stun.cloudflare.com:3478".into());
    let server = if settings.eq_ignore_ascii_case("off") {
        None
    } else {
        settings.to_socket_addrs().ok().and_then(|mut addresses| addresses.find_map(|address| {
            if let SocketAddr::V4(address) = address { Some(address) } else { None }
        }))
    };
    let reflexive = server.and_then(|server| binding(socket, server));
    let probe = server.unwrap_or_else(|| SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 9));
    // UDP connect here only selects a route. This temporary socket sends no packets.
    if let Ok(route) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        && route.connect(probe).is_ok()
        && let Ok(SocketAddr::V4(local)) = route.local_addr()
        && !local.ip().is_unspecified() && !local.ip().is_loopback()
    {
        addresses.push(SocketAddrV4::new(*local.ip(), port));
    }
    if let Some(address) = reflexive && !addresses.contains(&address) { addresses.push(address); }
    addresses.push(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    Ok(Candidates { addresses, server: server.filter(|_| reflexive.is_some()) })
}
