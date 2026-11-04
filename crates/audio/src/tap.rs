//! Opt-in recording of the final mix: `VOXEL_AUDIO_TAP=<file.wav>`.
//!
//! The file holds exactly what reaches the device, so a bad sound in it is a
//! synthesis or mix fault, and a clean file with audible garble points at
//! device starvation, which the underrun count in the log confirms.
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// Three minutes of 48 kHz stereo; later frames are dropped.
const MAX_SAMPLES: usize = 48_000 * 2 * 180;
/// Rewrite the file this often, so a crash or kill still leaves a recording.
const FLUSH_SAMPLES: usize = 48_000 * 2 * 5;

/// Collects frames on the audio thread and hands full blocks to a writer
/// thread. Writing the file on the mixer thread stalled it past the queued
/// output and caused the very underruns the tap is meant to count.
pub(crate) struct Tap {
    block: Vec<i16>,
    tx: Option<mpsc::Sender<Vec<i16>>>,
    sent: usize,
}

impl Tap {
    pub(crate) fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("VOXEL_AUDIO_TAP")?);
        let (tx, rx) = mpsc::channel::<Vec<i16>>();
        std::thread::Builder::new()
            .name("audio-tap".into())
            .spawn(move || {
                let mut samples = Vec::new();
                for block in rx {
                    samples.extend_from_slice(&block);
                    save(&path, &samples);
                }
            })
            .ok()?;
        Some(Self { block: Vec::with_capacity(FLUSH_SAMPLES), tx: Some(tx), sent: 0 })
    }

    pub(crate) fn push(&mut self, frame: [i16; 2]) {
        if self.sent >= MAX_SAMPLES {
            return;
        }
        self.block.extend_from_slice(&frame);
        if self.block.len() >= FLUSH_SAMPLES {
            self.flush();
        }
    }

    pub(crate) fn flush(&mut self) {
        let block = std::mem::replace(&mut self.block, Vec::with_capacity(FLUSH_SAMPLES));
        self.sent += block.len();
        if let Some(tx) = &self.tx
            && tx.send(block).is_err()
        {
            self.tx = None;
        }
    }
}

fn save(path: &Path, samples: &[i16]) {
    let seconds = samples.len() as f32 / (48_000.0 * 2.0);
    let underruns = crate::underruns();
    let gap = crate::LONGEST_GAP_US.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    let mix = crate::LONGEST_MIX_US.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
    match write_wav(path, 2, 48_000, samples) {
        Ok(()) => eprintln!(
            "[audio-tap] {seconds:.0} s, {underruns} underruns, longest wake gap {gap:.1} ms, longest mix {mix:.1} ms -> {}",
            path.display()
        ),
        Err(error) => eprintln!("[audio-tap] {}: {error}", path.display()),
    }
}

/// Writes 16-bit PCM.
pub(crate) fn write_wav(path: &Path, channels: u16, rate: u32, samples: &[i16]) -> std::io::Result<()> {
    let data_len = samples.len() as u32 * 2;
    let align = channels * 2;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
    bytes.extend_from_slice(&align.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes)
}
