//! Bounded disposable Stage files with a session-held writer lease.
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, TryLockError};

mod format;
mod fs;
mod writer;
use writer::Writer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WriteOutcome { Stored, AlreadyPresent, Busy, ReadOnly, Disabled, ProtectedSlot, QuotaBlocked }

#[derive(Clone, Copy)]
struct Limits { bytes: u64, entries: usize }
impl Default for Limits {
    fn default() -> Self { Self { bytes: 512 * 1024 * 1024, entries: 8192 } }
}

enum State { Uninitialized, ReadOnly, Disabled, Active(Writer) }

pub(super) struct Store { root: PathBuf, state: Mutex<State>, limits: Limits }
impl Store {
    pub(super) fn new(root: PathBuf) -> Self {
        Self { root, state: Mutex::new(State::Uninitialized), limits: Limits::default() }
    }

    pub(super) fn read(&self, key: &[u8]) -> io::Result<Option<Vec<u8>>> {
        format::key_valid(key)?;
        match fs::check_root(&self.root) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        }
        let mut file = match fs::regular_file(&self.root.join(format::filename(key))) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        };
        let len = file.metadata()?.len();
        if len > format::MAX_FILE { return Ok(None); }
        match format::read_entry(&mut file, len, key) {
            Ok(value) => Ok(Some(value)),
            Err(e) if matches!(e.kind(), io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(super) fn try_write(&self, key: &[u8], payload: &[u8], replace_invalid: bool) -> io::Result<WriteOutcome> {
        format::entry_len(key, payload.len())?;
        let mut state = match self.state.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return Ok(WriteOutcome::Busy),
            Err(TryLockError::Poisoned(_)) => return Ok(WriteOutcome::Disabled),
        };
        if matches!(*state, State::Uninitialized) {
            match Writer::open(&self.root, self.limits) {
                Ok(Some(writer)) => *state = State::Active(writer),
                Ok(None) => *state = State::ReadOnly,
                Err(error) => { *state = State::Disabled; return Err(error); }
            }
        }
        let result = match &mut *state {
            State::Active(writer) => writer.write(&self.root, key, payload, replace_invalid, self.limits),
            State::ReadOnly => return Ok(WriteOutcome::ReadOnly),
            _ => return Ok(WriteOutcome::Disabled),
        };
        if result.is_err() { *state = State::Disabled; }
        result
    }
}

#[cfg(test)]
#[path = "storage/tests.rs"]
mod tests;
