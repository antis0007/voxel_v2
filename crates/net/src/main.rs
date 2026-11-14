//! Standalone session harness; does not load or mutate game saves.
use engine_net::{
    protocol::{Packet, TICK_HZ, WorldIdentity},
    steam::{NetResult, SteamClient, SteamHost},
};
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

fn main() -> NetResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 || !matches!(args[0].as_str(), "host" | "join") {
        return Err("usage: voxel-net <host|join> <IP:port> <seed> <generator> <cell-mm> <64-hex-content-digest>".into());
    }
    let address: SocketAddr = args[1].parse()?;
    let digest = args[5].as_bytes();
    if digest.len() != 64 || !digest.iter().all(u8::is_ascii_hexdigit) {
        return Err("content digest must be 64 hex digits".into());
    }
    let mut content = [0; 32];
    for (i, value) in content.iter_mut().enumerate() {
        *value = u8::from_str_radix(&args[5][i * 2..i * 2 + 2], 16)?;
    }
    let world = WorldIdentity {
        seed: args[2].parse()?,
        generator: args[3].parse()?,
        cell_mm: args[4].parse()?,
        content,
    };
    if world.cell_mm == 0 || world.generator == 0 {
        return Err("generator and cell size must be nonzero".into());
    }
    if args[0] == "host" {
        let mut host = SteamHost::bind(address, world)?;
        println!("listening on {address}, {TICK_HZ} Hz");
        let step = Duration::from_secs_f64(1.0 / f64::from(TICK_HZ));
        let mut next = Instant::now() + step;
        loop {
            let now = Instant::now();
            host.poll(now)?;
            if now >= next {
                let frame = host.advance()?;
                if let Packet::Frame { tick, players } = frame {
                    if tick % u64::from(TICK_HZ) == 0 {
                        println!("tick={tick} players={}", players.len());
                    }
                }
                // No unbounded catch-up after suspend or a debugger pause.
                next = if now.duration_since(next) > step * 4 {
                    now + step
                } else {
                    next + step
                };
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    } else {
        let mut client = SteamClient::connect(address, world, Instant::now())?;
        loop {
            let frames = client.poll(Instant::now())?;
            if let Some(player) = client.player() {
                client.submit([0; 2], [0; 2], 0)?;
                for frame in frames {
                    if let Packet::Frame { tick, players } = frame {
                        if tick % u64::from(TICK_HZ) == 0 {
                            println!("player={player} tick={tick} roster={}", players.len());
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
