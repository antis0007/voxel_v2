//! Joiner profiles for a single-player world opened to friends, kept in that world's folder.
//! Bounded on read: a damaged or oversized file is refused, never trusted.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const FILE: &str = "friends.vxp";
const MAGIC: &[u8; 6] = b"VXFRND";
const VERSION: u8 = 1;
/// Matches the host's 128-profile registry limit.
pub const MAX_PROFILES: usize = 128;
const MAX_PROFILE_BYTES: usize = 1024 * 1024;
const MAX_FILE_BYTES: usize = 32 * 1024 * 1024;
const REALM_PREFIX: &[u8] = b"VXREALM\x01";

pub type Profiles = BTreeMap<String, Vec<u8>>;

fn valid_realm(realm: &str) -> bool {
    (1..=96).contains(&realm.len()) && realm.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// An ordinary entry is a compressed player image. A guest away from the host's
/// world carries its realm ID in the same checksummed entry as its player image.
pub(super) fn in_realm(realm: &str, player: &[u8]) -> Result<Vec<u8>> {
    ensure!(valid_realm(realm), "Invalid guest realm identity");
    let mut entry = Vec::with_capacity(REALM_PREFIX.len() + 1 + realm.len() + player.len());
    entry.extend_from_slice(REALM_PREFIX);
    entry.push(realm.len() as u8);
    entry.extend_from_slice(realm.as_bytes());
    entry.extend_from_slice(player);
    Ok(entry)
}

pub(super) fn realm_entry(entry: &[u8]) -> Result<Option<(&str, &[u8])>> {
    let Some(payload) = entry.strip_prefix(REALM_PREFIX) else { return Ok(None); };
    let (&length, payload) = payload.split_first().context("Missing guest realm identity")?;
    let (id, player) = payload.split_at_checked(usize::from(length)).context("Truncated guest realm entry")?;
    let realm = std::str::from_utf8(id)?;
    ensure!(valid_realm(realm) && !player.is_empty(), "Invalid guest realm entry");
    Ok(Some((realm, player)))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body<T = Profiles> {
    checksum: u64,
    profiles: T,
}

/// Where a world keeps its friends' profiles.
pub fn path_for(world_root: &Path) -> PathBuf {
    world_root.join(FILE)
}

fn checksum(profiles: &Profiles) -> Result<u64> {
    Ok(engine_net::transfer::checksum(ron::to_string(profiles)?.as_bytes()))
}

#[cfg(test)]
fn encode(profiles: &Profiles) -> Result<Vec<u8>> {
    encode_checked(profiles, checksum(profiles)?)
}

fn encode_checked(profiles: &Profiles, checksum: u64) -> Result<Vec<u8>> {
    ensure!(profiles.len() <= MAX_PROFILES, "player registry limit");
    ensure!(profiles.iter().all(|(k, v)| valid_key(k) && v.len() <= MAX_PROFILE_BYTES), "invalid player profile entry");
    let body = super::gameplay::pack(&Body { checksum, profiles })?;
    let mut bytes = Vec::with_capacity(body.len() + MAGIC.len() + 1);
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.extend_from_slice(&body);
    ensure!(bytes.len() <= MAX_FILE_BYTES, "player file exceeds size limit");
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Profiles> {
    ensure!(bytes.len() <= MAX_FILE_BYTES, "player file exceeds size limit");
    ensure!(bytes.len() > MAGIC.len() && bytes.starts_with(MAGIC), "not a player file");
    let version = bytes[MAGIC.len()];
    ensure!(version == VERSION, "player file version {version} is not supported");
    let body: Body = super::gameplay::unpack_limit(&bytes[MAGIC.len() + 1..], MAX_FILE_BYTES)?;
    ensure!(body.profiles.len() <= MAX_PROFILES, "player registry limit");
    ensure!(
        body.profiles.iter().all(|(k, v)| valid_key(k) && v.len() <= MAX_PROFILE_BYTES),
        "invalid player profile entry"
    );
    ensure!(checksum(&body.profiles)? == body.checksum, "player file checksum mismatch");
    for entry in body.profiles.values() {
        realm_entry(entry)?;
    }
    Ok(body.profiles)
}

const TOKEN: &str = "token:";

/// Registry keys are key fingerprints or namespaced profile tokens, never free text.
/// A bare token has the shape of a fingerprint, so bare entries only ever match a key.
fn valid_key(key: &str) -> bool {
    engine_net::identity::is_fingerprint(key) || key.strip_prefix(TOKEN).is_some_and(super::gameplay::valid_profile)
}

/// The registry key for a joiner: its proven key, or on a keyless transport its token in
/// a separate namespace, so a token equal to someone's public fingerprint names nobody else.
pub fn registry_key(fingerprint: Option<String>, keyed: bool, token: &str) -> Result<String> {
    match fingerprint {
        Some(fingerprint) => Ok(fingerprint),
        None => {
            ensure!(!keyed, "player sent a profile before proving a key");
            ensure!(super::gameplay::valid_profile(token), "invalid player profile");
            Ok(format!("{TOKEN}{token}"))
        }
    }
}

/// A missing file is an empty registry.
pub fn load(path: &Path) -> Result<Profiles> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Profiles::new()),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    decode(&bytes)
}

#[cfg(test)]
fn save(path: &Path, profiles: &Profiles) -> Result<()> {
    save_if_changed(path, profiles, None).map(|_| ())
}

/// Replaces the file atomically. Supplying the previous mark skips identical writes.
pub(super) fn save_if_changed(path: &Path, profiles: &Profiles, previous: Option<u64>) -> Result<u64> {
    let mark = checksum(profiles)?;
    if previous == Some(mark) {
        return Ok(mark);
    }
    let bytes = encode_checked(profiles, mark)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("vxp.{}.tmp", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    Ok(mark)
}

/// Loads the registry; a damaged file is set aside so the next save cannot overwrite it.
pub fn load_or_set_aside(path: &Path) -> (Profiles, Option<String>) {
    match load(path) {
        Ok(profiles) => (profiles, None),
        Err(error) => {
            let aside = path.with_extension("vxp.damaged");
            let moved = std::fs::rename(path, &aside).is_ok();
            let note = format!("player file {}: {error:#}{}", path.display(), if moved { "; kept as .damaged" } else { "" });
            (Profiles::new(), Some(note))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profiles {
        let mut profiles = Profiles::new();
        profiles.insert("token:0123456789abcdef0123456789abcdef".into(), vec![1, 2, 3, 250]);
        profiles.insert("fedcba9876543210fedcba9876543210".into(), vec![9; 300]);
        profiles
    }

    #[test]
    fn profiles_survive_a_round_trip_on_disk() {
        let dir = std::env::temp_dir().join(format!("voxel-friends-round-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = path_for(&dir);
        assert!(load(&path).expect("missing file").is_empty());
        save(&path, &sample()).expect("save");
        assert_eq!(load(&path).expect("load"), sample());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_player_files_are_rejected() {
        let good = encode(&sample()).expect("encode");
        let mut flipped = good.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x55;
        assert!(decode(&flipped).is_err(), "flipped byte");
        assert!(decode(&good[..good.len() / 2]).is_err(), "truncated");
        let mut future = good.clone();
        future[MAGIC.len()] = VERSION + 1;
        assert!(decode(&future).is_err(), "unknown version");
        assert!(decode(b"VXFRND").is_err(), "header only");
        assert!(decode(b"(profiles:{})").is_err(), "foreign text");
        // A body whose checksum disagrees with its contents.
        let forged = super::super::gameplay::pack(&Body { checksum: 1, profiles: sample() }).expect("pack");
        let mut bytes = MAGIC.to_vec();
        bytes.push(VERSION);
        bytes.extend_from_slice(&forged);
        assert!(decode(&bytes).is_err(), "checksum mismatch");
    }

    #[test]
    fn a_damaged_file_is_set_aside_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("voxel-friends-aside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = path_for(&dir);
        std::fs::write(&path, b"garbage").expect("write");
        let (profiles, note) = load_or_set_aside(&path);
        assert!(profiles.is_empty() && note.is_some());
        assert!(!path.exists() && path.with_extension("vxp.damaged").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_token_never_restores_a_key_profile() {
        let fingerprint = "fedcba9876543210fedcba9876543210".to_owned();
        let mut registry = sample();
        assert!(registry.contains_key(&fingerprint));
        // Keyless transport: the token is the victim's public fingerprint.
        let key = registry_key(None, false, &fingerprint).expect("token key");
        assert_ne!(key, fingerprint);
        assert!(!registry.contains_key(&key), "building-save registry");
        // The same keying against the per-world sidecar after a disk round trip.
        let sidecar = decode(&encode(&registry).expect("encode")).expect("decode");
        assert!(!sidecar.contains_key(&key), "sidecar registry");
        // A keyed transport never falls back to the token.
        assert!(registry_key(None, true, &fingerprint).is_err());
        assert_eq!(registry_key(Some(fingerprint.clone()), true, "x").expect("key"), fingerprint);
        registry.insert(key.clone(), vec![7]);
        assert_eq!(decode(&encode(&registry).expect("encode")).expect("decode").get(&key), Some(&vec![7]));
    }

    #[test]
    fn bare_tokens_and_free_text_are_not_registry_keys() {
        assert!(!valid_key("token:short"));
        assert!(!valid_key("Ana"));
        let mut registry = Profiles::new();
        registry.insert("not a key".into(), vec![1]);
        assert!(encode(&registry).is_err());
    }

    #[test]
    fn oversized_registries_are_refused() {
        let mut profiles = Profiles::new();
        for i in 0..=MAX_PROFILES {
            profiles.insert(format!("token:{i:032x}"), vec![0]);
        }
        assert!(encode(&profiles).is_err());
    }
}
