//! Panic hook that keeps a record of crashes on disk.
//!
//! The release build runs windowed, so a panic's message goes to a console
//! nobody is watching. The hook appends the message, the thread and a forced
//! backtrace to `saves/crash.log` before the default handler runs.

use std::io::Write;

const CRASH_LOG: &str = "saves/crash.log";

pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let record = format!(
            "----\nunix_time={}\nthread={}\n{info}\n{}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            thread.name().unwrap_or("unnamed"),
            std::backtrace::Backtrace::force_capture(),
        );
        // Logging must never turn one crash into a second one.
        let _ = std::fs::create_dir_all("saves");
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(CRASH_LOG)
        {
            let _ = file.write_all(record.as_bytes());
        }
        previous(info);
    }));
}
