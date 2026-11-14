use super::*;
use std::fs as disk;
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

static TEST_ID: AtomicU64 = AtomicU64::new(0);
fn root() -> PathBuf {
    let base = std::env::temp_dir().join("axiom-stage-storage-tests");
    disk::create_dir_all(&base).unwrap();
    loop {
        let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = base.join(format!("{}-{stamp}-{id}", std::process::id()));
        match disk::create_dir(&path) {
            Ok(()) => return path,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => panic!("private test directory: {e}"),
        }
    }
}
fn limited(root: PathBuf, bytes: u64, entries: usize) -> Store {
    Store { root, state: Mutex::new(State::Uninitialized), limits: Limits { bytes, entries } }
}
fn put(path: &std::path::Path, key: &[u8], payload: &[u8]) {
    let mut f = disk::File::create(path).unwrap();
    format::write_entry(&mut f, key, payload).unwrap();
}
fn assert_stored(store: &Store, key: &[u8], payload: &[u8]) {
    assert_eq!(store.try_write(key, payload, false).unwrap(), WriteOutcome::Stored);
}

#[test]
fn stage_storage_roundtrip_immutable_and_semantic_repair() {
    let dir = root(); let store = Store::new(dir.clone());
    assert_eq!(store.read(b"one").unwrap(), None);
    assert_stored(&store, b"one", b"payload");
    assert_eq!(store.read(b"one").unwrap(), Some(b"payload".to_vec()));
    assert_eq!(store.read(b"other").unwrap(), None);
    assert_eq!(store.try_write(b"one", b"different", false).unwrap(), WriteOutcome::AlreadyPresent);
    assert_eq!(store.try_write(b"one", b"repaired", true).unwrap(), WriteOutcome::Stored);
    assert_eq!(store.read(b"one").unwrap(), Some(b"repaired".to_vec()));
}

#[test]
fn stage_storage_corrupt_truncated_and_trailing_entries_repair() {
    for kind in 0..3 {
        let dir = root(); let key = b"repair"; let path = dir.join(format::filename(key));
        put(&path, key, b"initial payload");
        let mut bytes = disk::read(&path).unwrap();
        match kind { 0 => *bytes.last_mut().unwrap() ^= 1, 1 => bytes.truncate(bytes.len()-3), _ => bytes.extend(b"extra") }
        disk::write(&path, bytes).unwrap();
        let store = Store::new(dir);
        assert!(store.read(key).unwrap().is_none());
        assert_stored(&store, key, b"replacement");
        assert_eq!(store.read(key).unwrap(), Some(b"replacement".to_vec()));
    }
}

#[test]
fn stage_storage_unknown_headers_keys_and_foreign_files_protected() {
    let dir = root(); let key = b"wanted";
    let target = dir.join(format::filename(key));
    put(&target, b"different-key", b"do not replace");
    let original = disk::read(&target).unwrap();
    disk::write(dir.join("user-notes.txt"), b"precious").unwrap();
    let store = Store::new(dir.clone());
    assert!(store.read(key).unwrap().is_none());
    assert_eq!(store.try_write(key, b"new", true).unwrap(), WriteOutcome::ProtectedSlot);
    assert_eq!(disk::read(target).unwrap(), original);
    assert_stored(&store, b"other", b"safe");
    assert_eq!(disk::read(dir.join("user-notes.txt")).unwrap(), b"precious");
}

#[test]
fn stage_storage_stale_temporary_ownership() {
    let dir = root(); let hash = format::digest_name(b"temp");
    let stale = dir.join(format!("{}{hash}.abc.0000000000000001.tmp", format::PREFIX));
    put(&stale, b"temp", b"payload");
    let unknown = dir.join(format!("{}{hash}.abc.0000000000000002.tmp", format::PREFIX));
    disk::write(&unknown, b"partial").unwrap();
    let store = Store::new(dir.clone());
    assert_stored(&store, b"one", b"new");
    assert!(!stale.exists());
    assert_eq!(disk::read(unknown).unwrap(), b"partial");
}

#[test]
fn stage_storage_byte_and_entry_quota_include_temporary_peak() {
    let dir = root(); let store = limited(dir.clone(), 512, 8192);
    for key in [b"a", b"b", b"c", b"d"] { assert_stored(&store, key, &[7;150]); }
    let bytes: u64 = disk::read_dir(&dir).unwrap().map(|x| x.unwrap().metadata().unwrap().len()).sum();
    assert!(bytes <= 512);
    let present = [b"a", b"b", b"c", b"d"].into_iter().filter(|key| store.read(*key).unwrap().is_some()).count();
    assert_eq!(present,2);
    let dir = root(); let store = limited(dir.clone(), 10000, 4);
    for key in [b"a", b"b", b"c", b"d", b"e"] { assert_stored(&store, key, b"x"); }
    assert!(disk::read_dir(dir).unwrap().count() <= 4);
    let store = limited(root(),300,8192);
    assert_stored(&store,b"a",&[1;150]);
    assert_eq!(store.try_write(b"a",&[2;150],true).unwrap(),WriteOutcome::QuotaBlocked);
    assert_eq!(store.read(b"a").unwrap(),Some(vec![1;150]));
}

#[test]
fn stage_storage_foreign_quota_blocks_without_deletion() {
    let dir=root(); disk::write(dir.join("foreign"),[9;250]).unwrap();
    let store=limited(dir.clone(),300,8192);
    assert_eq!(store.try_write(b"a",b"payload",false).unwrap(),WriteOutcome::QuotaBlocked);
    assert_eq!(disk::read(dir.join("foreign")).unwrap(),vec![9;250]);
}

#[test]
fn stage_storage_os_owner_and_mutex_contention_are_nonblocking() {
    let dir=root();let owner=Store::new(dir.clone());assert_stored(&owner,b"a",b"old");
    let reader=Store::new(dir.clone());
    assert_eq!(reader.try_write(b"b",b"new",false).unwrap(),WriteOutcome::ReadOnly);
    assert_eq!(reader.read(b"a").unwrap(),Some(b"old".to_vec()));
    {
        let _guard=owner.state.lock().unwrap();
        assert_eq!(owner.try_write(b"b",b"new",false).unwrap(),WriteOutcome::Busy);
        assert_eq!(owner.read(b"a").unwrap(),Some(b"old".to_vec()));
    }
    drop(owner);
    assert_eq!(reader.try_write(b"b",b"new",false).unwrap(),WriteOutcome::ReadOnly);
    let next=Store::new(dir);assert_stored(&next,b"b",b"new");
}

#[test]
fn stage_storage_concurrent_reads_see_complete_old_or_new() {
    let store=Arc::new(Store::new(root()));assert_stored(&store,b"a",&[1;256]);
    std::thread::scope(|scope| {
        for _ in 0..3 {
            let store=Arc::clone(&store);
            scope.spawn(move || for _ in 0..40 {
                let value=store.read(b"a").unwrap().expect("atomic replacement has no missing interval");
                assert!(value==vec![1;256] || value==vec![2;256]);
            });
        }
        for index in 0..30 {
            assert_eq!(store.try_write(b"a",&[if index%2==0 {2} else {1};256],true).unwrap(),WriteOutcome::Stored);
        }
    });
}

#[test]
fn stage_storage_failure_disables_writer_but_not_reads() {
    let dir=root();let store=Store::new(dir.clone());assert_stored(&store,b"good",b"data");
    disk::create_dir(dir.join(format::filename(b"blocked"))).unwrap();
    assert!(store.try_write(b"blocked",b"x",false).is_err());
    assert_eq!(store.try_write(b"later",b"x",false).unwrap(),WriteOutcome::Disabled);
    assert_eq!(store.read(b"good").unwrap(),Some(b"data".to_vec()));
    assert!(dir.join(format::filename(b"blocked")).is_dir());
}

#[test]
fn stage_storage_bounds_and_parser_reject_invalid_frames() {
    let store=Store::new(root());
    assert!(store.try_write(b"",b"",false).is_err());
    assert!(store.try_write(&vec![0;4097],b"",false).is_err());
    assert!(store.try_write(b"a",&vec![0;format::MAX_FILE as usize],false).is_err());
    assert_stored(&store,b"a",b"ok");
    let mut frame=Vec::new();format::write_entry(&mut frame,b"key",b"value").unwrap();
    for kind in 0..6 {
        let mut bad=frame.clone();
        match kind {
            0=>bad[0]^=1,1=>bad[16..20].copy_from_slice(&2_u32.to_le_bytes()),
            2=>bad[20..24].copy_from_slice(&4097_u32.to_le_bytes()),
            3=>bad[24..32].copy_from_slice(&u64::MAX.to_le_bytes()),
            4=>{bad.pop();},_=>bad.push(0),
        }
        assert!(format::read_entry(&mut &bad[..],bad.len() as u64,b"key").is_err());
    }
    assert!(format::read_entry(&mut &frame[..],frame.len() as u64,b"other").is_err());
}

#[test]
fn stage_storage_reparse_entry_is_not_followed() {
    let dir=root();let target=dir.join("foreign");disk::write(&target,b"precious").unwrap();
    let link=dir.join(format::filename(b"key"));
    #[cfg(windows)] let result=std::os::windows::fs::symlink_file(&target,&link);
    #[cfg(unix)] let result=std::os::unix::fs::symlink(&target,&link);
    #[cfg(not(any(windows,unix)))] return;
    #[cfg(any(windows,unix))] {
        if let Err(e)=result { assert_eq!(e.kind(),io::ErrorKind::PermissionDenied); eprintln!("symlink fixture unavailable: {e}");return; }
        let store=Store::new(dir);
        assert!(store.read(b"key").is_err());
        assert!(store.try_write(b"key",b"new",true).is_err());
        assert_eq!(disk::read(target).unwrap(),b"precious");
    }
}

#[test]
fn stage_storage_external_metadata_change_disables_session() {
    let dir=root();let store=Store::new(dir.clone());assert_stored(&store,b"a",b"data");
    let path=dir.join(format::filename(b"a"));let mut file=disk::OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::End(0)).unwrap();file.write_all(b"unexpected").unwrap();drop(file);
    assert!(store.read(b"a").unwrap().is_none());
    assert!(store.try_write(b"a",b"repair",true).is_err());
    assert_eq!(store.try_write(b"next",b"x",false).unwrap(),WriteOutcome::Disabled);
}

#[test]
fn stage_storage_cross_process_owner_is_read_only() {
    let dir=root();let owner=Store::new(dir.clone());assert_stored(&owner,b"a",b"owned");
    let test_name=format!("{}::stage_storage_child_reader",module_path!().split_once("::").unwrap().1);
    let mut command=std::process::Command::new(std::env::current_exe().unwrap());
    command.args([&test_name,"--exact","--ignored","--nocapture","--test-threads=1"])
        .env("AXIOM_STAGE_STORAGE_CHILD_ROOT",&dir);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output=command.output().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

#[test]
#[ignore = "invoked only by cross-process storage fixture"]
fn stage_storage_child_reader() {
    let dir=std::env::var_os("AXIOM_STAGE_STORAGE_CHILD_ROOT").expect("parent-owned private test root");
    let store=Store::new(dir.into());
    assert_eq!(store.try_write(b"b",b"child",false).unwrap(),WriteOutcome::ReadOnly);
    assert_eq!(store.read(b"a").unwrap(),Some(b"owned".to_vec()));
}

#[test]
#[cfg(windows)]
fn stage_storage_failed_replace_preserves_valid_destination() {
    let dir=root();let store=Store::new(dir.clone());assert_stored(&store,b"a",b"keep");
    let path=dir.join(format::filename(b"a"));
    let mut permissions=disk::metadata(&path).unwrap().permissions();permissions.set_readonly(true);
    disk::set_permissions(&path,permissions).unwrap();
    assert!(store.try_write(b"a",b"replacement",true).is_err());
    assert_eq!(store.read(b"a").unwrap(),Some(b"keep".to_vec()));
    assert_eq!(store.try_write(b"b",b"x",false).unwrap(),WriteOutcome::Disabled);
    let mut permissions=disk::metadata(&path).unwrap().permissions();permissions.set_readonly(false);
    disk::set_permissions(&path,permissions).unwrap();
}
