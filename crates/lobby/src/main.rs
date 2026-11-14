//! `voxel-lobby`: session directory and relay for online voxel sessions.
use engine_lobby::{directory::Limits, server::{Config, Lobby}};
use std::{net::TcpListener, process::ExitCode};

const USAGE: &str = "usage: voxel-lobby [--bind ADDR] [--max-sessions N] [--max-per-address N] [--max-connections N]";

fn parse(args: impl Iterator<Item = String>) -> Result<(String, Config), String> {
    let mut bind = "0.0.0.0:27021".to_owned();
    let mut config = Config { limits: Limits { max_sessions: 1024, max_per_address: 4 }, max_connections: 512 };
    let mut args = args;
    while let Some(flag) = args.next() {
        let value = |args: &mut dyn Iterator<Item = String>| args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--bind" => bind = value(&mut args)?,
            "--max-sessions" => config.limits.max_sessions = value(&mut args)?.parse().map_err(|_| "invalid --max-sessions")?,
            "--max-per-address" => config.limits.max_per_address = value(&mut args)?.parse().map_err(|_| "invalid --max-per-address")?,
            "--max-connections" => config.max_connections = value(&mut args)?.parse().map_err(|_| "invalid --max-connections")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok((bind, config))
}

fn main() -> ExitCode {
    let (bind, config) = match parse(std::env::args().skip(1)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let listener = match TcpListener::bind(&bind) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("[lobby] cannot bind {bind}: {error}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("[lobby] listening on {bind} sessions<={} per-address<={} connections<={}", config.limits.max_sessions, config.limits.max_per_address, config.max_connections);
    let lobby = Lobby::new(config);
    match lobby.serve(listener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[lobby] stopped: {error}");
            ExitCode::FAILURE
        }
    }
}
