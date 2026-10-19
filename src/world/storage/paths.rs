//! Stable user-owned storage; legacy worlds are imported only when opened.
use super::*;
use std::sync::OnceLock;

pub fn save_directory() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some(dir) = std::env::var_os("VOXEL_SAVE_DIR").filter(|s| !s.is_empty()) {
            return cwd.join(dir);
        }
        let candidates = default_directories(&cwd);
        for (index, dir) in candidates.iter().enumerate() {
            if writable_directory(dir) {
                if index > 0 {
                    eprintln!(
                        "[storage] Preferred save directory is unavailable; using {}",
                        dir.display()
                    );
                }
                return dir.clone();
            }
        }
        // Keep the normal error path if none are writable. Never claim that a
        // save succeeded, or silently redirect an explicit VOXEL_SAVE_DIR.
        candidates
            .into_iter()
            .next()
            .unwrap_or_else(|| cwd.join("saves"))
    })
    .clone()
}

fn default_directories(cwd: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for key in ["LOCALAPPDATA", "APPDATA", "USERPROFILE", "HOME"] {
        if let Some(dir) = std::env::var_os(key).filter(|s| !s.is_empty()) {
            let path = cwd.join(dir).join("VoxelWorkshop").join("saves");
            if !roots.contains(&path) {
                roots.push(path);
            }
        }
    }
    roots.push(cwd.join("saves"));
    roots
}

fn writable_directory(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| t.as_nanos());
    let probe = dir.join(format!(".write-probe-{}-{stamp}", std::process::id()));
    let Ok(file) = OpenOptions::new().write(true).create_new(true).open(&probe) else {
        return false;
    };
    let synced = file.sync_all().is_ok();
    drop(file);
    let removed = fs::remove_file(&probe).is_ok();
    synced && removed
}

pub(super) fn library_directories() -> Vec<PathBuf> {
    let mut roots = vec![save_directory()];
    let mut add = |dir: PathBuf| {
        if dir.is_dir() && !roots.contains(&dir) {
            roots.push(dir);
        }
    };
    if let Ok(cwd) = std::env::current_dir() {
        // Retain discovery of worlds at the preferred path after a permission
        // fallback, so opening a legacy world can import it into writable storage.
        for dir in default_directories(&cwd) {
            add(dir);
        }
    }
    // A loose candidate lives under target/<review>/; find the repository's
    // legacy saves even when Explorer launches it from a different directory.
    if let Ok(exe) = std::env::current_exe() {
        for dir in exe.ancestors().skip(1).take(5) {
            if dir.parent().is_some() {
                add(dir.join("saves"));
            }
        }
    }
    roots
}

pub(super) fn existing_world(id: &str) -> PathBuf {
    library_directories()
        .into_iter()
        .map(|r| r.join("regions").join(id))
        .find(|r| r.is_dir())
        .unwrap_or_else(|| save_directory().join("regions").join(id))
}

/// Runs on the library worker. Publish a complete copy atomically and retain
/// every original file. New writes never need permission in an installation.
pub(super) fn import_world(id: &str) -> Result<()> {
    ensure!(
        (1..=96).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid world identity"
    );
    let source = existing_world(id);
    let target = save_directory().join("regions").join(id);
    if source == target || !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(target.parent().context("Missing region parent")?).with_context(|| {
        format!(
            "Cannot create save directory {}",
            save_directory().display()
        )
    })?;
    let temporary = target.with_file_name(format!(
        ".import-{}-{}",
        id,
        super::super::regions::checkpoint_id()
    ));
    fs::create_dir(&temporary)?;
    let copy = || -> Result<()> {
        for entry in fs::read_dir(&source)? {
            let entry = entry?;
            let name = entry.file_name();
            let p = entry.path();
            if entry.file_type()?.is_file()
                && (p.extension().is_some_and(|e| e == "vxr") || name == "world.meta")
            {
                copy_writable(&p, &temporary.join(&name))?;
            } else if name == "checkpoints" && entry.file_type()?.is_dir() {
                fs::create_dir(temporary.join("checkpoints"))?;
                for commit in fs::read_dir(p)? {
                    let commit = commit?;
                    if commit.file_type()?.is_file()
                        && commit.path().extension().is_some_and(|e| e == "vxw")
                    {
                        copy_writable(
                            &commit.path(),
                            &temporary.join("checkpoints").join(commit.file_name()),
                        )?;
                    }
                }
            }
        }
        fs::rename(&temporary, &target)?;
        Ok(())
    };
    let result = copy().with_context(|| format!("Cannot import world into {}", target.display()));
    if result.is_err() {
        let _ = fs::remove_dir_all(&temporary);
    }
    result
}

fn copy_writable(source: &Path, target: &Path) -> Result<()> {
    fs::copy(source, target)
        .with_context(|| format!("Cannot copy {} to {}", source.display(), target.display()))?;
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(target)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(target, permissions)?;
    }
    OpenOptions::new().write(true).open(target)?.sync_all()?;
    Ok(())
}

pub(crate) fn preference_read_path() -> PathBuf {
    library_directories()
        .into_iter()
        .map(|r| r.join("preferences.vxp"))
        .find(|p| p.is_file() || p.with_extension("prev.vxp").is_file())
        .unwrap_or_else(|| save_directory().join("preferences.vxp"))
}
