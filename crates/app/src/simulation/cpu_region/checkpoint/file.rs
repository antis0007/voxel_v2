use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_BYTES: usize = 160 * 1024;
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("cannot open CPU checkpoint: {e}"))?;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read CPU checkpoint: {e}"))?;
    if bytes.len() > MAX_BYTES {
        return Err("CPU checkpoint exceeds size limit".into());
    }
    Ok(bytes)
}

pub(super) fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err("CPU checkpoint exceeds size limit".into());
    }
    let name = path.file_name().ok_or("CPU checkpoint needs a file name")?;
    let mut temporary = name.to_os_string();
    temporary.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = path.with_file_name(temporary);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| format!("cannot create CPU checkpoint: {e}"))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = written.and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|e| format!("cannot publish CPU checkpoint: {e}"))
}
