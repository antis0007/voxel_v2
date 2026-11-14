use super::*;
use std::io::Cursor;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("axiom-cache-storage-test-{}-{}",
            std::process::id(), NONCE.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> Store { Store::new(self.0.join("geology")) }
}
impl Drop for Temp {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn envelope(key: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new(); write_entry(&mut bytes, key, payload).unwrap(); bytes
}

#[test]
fn storage_cold_roundtrip_and_immutable_retry() {
    let temp = Temp::new(); let store = temp.store();
    assert!(store.read(b"key").unwrap().is_none());
    assert!(!store.root.exists());
    store.write(b"key", b"world").unwrap();
    assert_eq!(store.read(b"key").unwrap(), Some(b"world".to_vec()));
    store.write(b"key", b"different").unwrap();
    assert_eq!(store.read(b"key").unwrap(), Some(b"world".to_vec()));
    assert_eq!(store.inventory().unwrap().len(), 1);
}

#[test]
fn storage_wrong_key_corruption_and_repair() {
    let temp = Temp::new(); let store = temp.store();
    store.write(b"one", b"world").unwrap();
    let path = store.root.join(filename(b"one"));
    fs::copy(&path, store.root.join(filename(b"two"))).unwrap();
    assert!(store.read(b"two").unwrap().is_none());
    let mut bytes = fs::read(&path).unwrap(); *bytes.last_mut().unwrap() ^= 1;
    fs::write(&path, bytes).unwrap();
    assert!(store.read(b"one").unwrap().is_none());
    store.write(b"one", b"restored").unwrap();
    assert_eq!(store.read(b"one").unwrap(), Some(b"restored".to_vec()));
    assert!(store.read(b"two").unwrap().is_none());
}

#[test]
fn storage_repairs_truncated_and_trailing_payloads_with_valid_ownership_header() {
    let temp = Temp::new(); let store = temp.store();
    let key = b"repair-length";
    store.write(key, b"original body").unwrap();
    let path = store.root.join(filename(key));
    let complete = envelope(key, b"original body");
    // Retain the complete bounded header and key, but truncate all or part of the body.
    for end in [format::HEADER as usize + key.len(), complete.len() - 1] {
        fs::write(&path, &complete[..end]).unwrap();
        assert!(store.read(key).unwrap().is_none());
        assert!(store.inventory().unwrap().iter().any(|e| e.path == path && e.deletable));
        store.write(key, b"restored").unwrap();
        assert_eq!(store.read(key).unwrap(), Some(b"restored".to_vec()));
    }
    let mut trailing = complete; trailing.extend_from_slice(b"unexpected suffix");
    fs::write(&path, trailing).unwrap();
    assert!(store.read(key).unwrap().is_none());
    store.write(key, b"restored again").unwrap();
    assert_eq!(store.read(key).unwrap(), Some(b"restored again".to_vec()));
    assert_eq!(store.inventory().unwrap().len(), 1);
}

#[test]
fn storage_parser_rejects_truncated_trailing_and_oversize_data() {
    let original = envelope(b"k", b"data");
    for end in 0..original.len() {
        assert!(read_entry(&mut Cursor::new(&original[..end]), end as u64, b"k").is_err());
    }
    let mut trailing = original.clone(); trailing.push(0);
    assert!(read_entry(&mut Cursor::new(&trailing), trailing.len() as u64, b"k").is_err());
    for (offset, replacement) in [(20, u32::MAX.to_le_bytes().to_vec()),
        (24, (format::MAX_PAYLOAD + 1).to_le_bytes().to_vec())] {
        let mut bytes = original.clone(); bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        assert!(read_entry(&mut Cursor::new(&bytes), bytes.len() as u64, b"k").is_err());
    }
    for index in [0, 16, 32, 64, 65] {
        let mut bytes = original.clone(); bytes[index] ^= 1;
        assert!(read_entry(&mut Cursor::new(&bytes), bytes.len() as u64, b"k").is_err());
    }
    assert!(format::key_valid(&[]).is_err());
    assert!(format::key_valid(&vec![0; format::MAX_KEY + 1]).is_err());
}

#[test]
fn storage_partial_temporary_is_never_a_hit() {
    let temp = Temp::new(); let store = temp.store();
    fs::create_dir(&store.root).unwrap();
    let stem = format!("{}{}", format::PREFIX, digest_name(b"k"));
    let partial = store.root.join(format!("{stem}.1.0000000000000001.tmp"));
    fs::write(&partial, b"AX").unwrap();
    assert!(store.read(b"k").unwrap().is_none());
    // A complete ownership header can be cleaned even after a partial body write.
    let recognized = store.root.join(format!("{stem}.1.0000000000000002.tmp"));
    let bytes = envelope(b"k", b"body");
    fs::write(&recognized, &bytes[..format::HEADER as usize + 1]).unwrap();
    store.write(b"k", b"complete").unwrap();
    assert!(partial.exists()); // Unrecognizable bytes are not deleted.
    assert!(!recognized.exists());
    assert_eq!(store.read(b"k").unwrap(), Some(b"complete".to_vec()));
}

#[test]
fn storage_lock_contention_is_nonblocking_and_read_still_works() {
    let temp = Temp::new(); let store = temp.store();
    store.write(b"a", b"stable").unwrap();
    let lock = OpenOptions::new().read(true).write(true).open(store.root.join(LOCK)).unwrap();
    lock.try_lock().unwrap();
    assert_eq!(store.write(b"b", b"new").unwrap_err().kind(), ErrorKind::WouldBlock);
    assert_eq!(store.read(b"a").unwrap(), Some(b"stable".to_vec()));
    drop(lock);
    store.write(b"b", b"new").unwrap();
}

#[test]
fn storage_evicts_only_recognized_entries_and_caps_committed_count() {
    let temp = Temp::new(); let store = temp.store();
    store.write(b"a", b"a").unwrap();
    let personal = store.root.join("notes.txt"); fs::write(&personal, b"keep").unwrap();
    for key in [b"b", b"c", b"d"] { store.write(key, key).unwrap(); }
    let inventory = store.inventory().unwrap();
    assert_eq!(inventory.iter().filter(|e| !e.temporary).count(), 2);
    assert!(inventory.iter().map(|e| e.length).sum::<u64>() <= MAX_BYTES);
    assert_eq!(store.read(b"d").unwrap(), Some(b"d".to_vec()));
    assert_eq!(fs::read(personal).unwrap(), b"keep");
}

#[test]
fn storage_unrecognized_cache_named_files_are_preserved() {
    let temp = Temp::new(); let store = temp.store(); fs::create_dir(&store.root).unwrap();
    let unknown = store.root.join(filename(b"unknown")); fs::write(&unknown, b"do not delete").unwrap();
    store.write(b"a", b"a").unwrap(); store.write(b"b", b"b").unwrap();
    assert_eq!(fs::read(&unknown).unwrap(), b"do not delete");
    assert!(store.write(b"unknown", b"replacement").is_err());
    assert_eq!(fs::read(unknown).unwrap(), b"do not delete");
}

#[test]
fn storage_invalid_roots_and_lock_files_fail_without_mutation() {
    let temp = Temp::new();
    assert!(Store::new(PathBuf::from("relative-cache")).write(b"a", b"b").is_err());
    let file = temp.0.join("not-directory"); fs::write(&file, b"keep").unwrap();
    assert!(Store::new(file.clone()).read(b"a").is_err());
    assert!(Store::new(file.clone()).write(b"a", b"b").is_err());
    assert_eq!(fs::read(file).unwrap(), b"keep");
    let store = temp.store(); fs::create_dir(&store.root).unwrap();
    fs::write(store.root.join(LOCK), b"not our empty lock").unwrap();
    assert!(store.write(b"a", b"b").is_err());
    assert_eq!(fs::read(store.root.join(LOCK)).unwrap(), b"not our empty lock");
}

#[test]
fn storage_name_grammar_excludes_unrelated_and_traversal_names() {
    for name in ["../file.bin", "axiom-geology-v1-ab.bin", "notes.tmp", "axiom-geology-v1.lock"] {
        assert!(owned_name(name).is_none());
    }
    assert!(owned_name(&filename(b"key")).is_some());
    assert!(2 * (format::HEADER + format::MAX_KEY as u64 + format::MAX_PAYLOAD)
        + format::HEADER + format::MAX_KEY as u64 + format::MAX_PAYLOAD < MAX_BYTES);
}

#[cfg(unix)]
#[test]
fn storage_rejects_symlink_root_and_entry() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new(); let store = temp.store(); store.write(b"a", b"a").unwrap();
    let link = temp.0.join("link"); symlink(&store.root, &link).unwrap();
    assert!(Store::new(link).read(b"a").is_err());
    let outside = temp.0.join("outside"); fs::write(&outside, b"safe").unwrap();
    symlink(&outside, store.root.join(filename(b"b"))).unwrap();
    assert!(store.read(b"b").is_err()); assert!(store.write(b"b", b"new").is_err());
    assert_eq!(fs::read(outside).unwrap(), b"safe");
}

#[cfg(windows)]
#[test]
fn storage_read_only_lock_does_not_block_valid_reads_or_damage_entries() {
    let temp = Temp::new(); let store = temp.store(); store.write(b"a", b"stable").unwrap();
    let path = store.root.join(LOCK);
    let original = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone(); readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let write_result = store.write(b"b", b"new");
    let read_result = store.read(b"a");
    fs::set_permissions(&path, original).unwrap();
    assert!(write_result.is_err());
    assert_eq!(read_result.unwrap(), Some(b"stable".to_vec()));
    assert!(store.read(b"b").unwrap().is_none());
}
