//! Background thread for shell face bakes; separate from the tile worker so a
//! 262,144-tap bake never preempts tile bakes or their resumption state.

use super::shell::{ShellConfig, bake_shell};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct ShellBake {
    pub face: u8,
    pub resolution: u32,
    pub coverage: Vec<u8>,
}

pub struct ShellWorker {
    requests: Sender<(ShellConfig, f32, u8)>,
    results: Receiver<ShellBake>,
}

impl ShellWorker {
    /// `None` when the thread cannot be spawned; the shell then stays clear.
    pub fn spawn() -> Option<ShellWorker> {
        let (requests, request_rx) = channel::<(ShellConfig, f32, u8)>();
        let (result_tx, results) = channel();
        std::thread::Builder::new()
            .name("globe-shell".into())
            .spawn(move || {
                while let Ok((cfg, time_s, face)) = request_rx.recv() {
                    let coverage = bake_shell(&cfg, time_s, face);
                    if result_tx.send(ShellBake { face, resolution: cfg.resolution, coverage }).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(ShellWorker { requests, results })
    }

    pub fn request(&self, cfg: ShellConfig, time_s: f32, face: u8) {
        let _ = self.requests.send((cfg, time_s, face));
    }

    pub fn drain(&self) -> Vec<ShellBake> {
        self.results.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_requested_face_comes_back_at_its_resolution() {
        let Some(w) = ShellWorker::spawn() else { panic!("spawn failed") };
        let cfg = ShellConfig { resolution: 16, controls: Default::default(), weather_seed: 1 };
        w.request(cfg, 0.0, 3);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut got = Vec::new();
        while got.is_empty() && std::time::Instant::now() < deadline {
            got = w.drain();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].face, got[0].coverage.len()), (3, 256));
    }
}
