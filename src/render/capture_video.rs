//! Opt-in native RGBA recording; never runs on the drawing thread.
use super::EncodeJob;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

static CLOSED: AtomicBool = AtomicBool::new(false);
static CONTROL: OnceLock<Arc<Control>> = OnceLock::new();

#[derive(Default)]
struct Receipt {
    width: u32,
    height: u32,
    fps: u32,
    first: Option<u64>,
    last: Option<u64>,
    unique: u64,
    repeated: u64,
    late: u64,
    exit: Option<i32>,
    finalized: bool,
    error: Option<String>,
}
struct Control {
    path: PathBuf,
    child: Mutex<Option<Child>>,
    receipt: Mutex<Receipt>,
    aborted: AtomicBool,
}
fn quoted(value: &str) -> String {
    let mut s = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if c.is_control() => s.push_str(&format!("\\u{:04x}", c as u32)),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
impl Control {
    fn save(&self) {
        let r = self.receipt.lock().unwrap();
        let optional = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "null".into());
        let body = format!(
            "{{\n  \"codec\":\"h264_nvenc\",\"pixel_format\":\"yuv420p\",\"source\":\"native_rgba_readback\",\n  \"width\":{},\"height\":{},\"fps\":{},\n  \"first_index\":{},\"last_index\":{},\"unique_frames\":{},\"repeated_frames\":{},\"late_frames\":{},\n  \"exit_code\":{},\"finalized\":{},\"error\":{}\n}}\n",
            r.width,r.height,r.fps,optional(r.first),optional(r.last),r.unique,r.repeated,r.late,
            r.exit.map(|n| n.to_string()).unwrap_or_else(|| "null".into()),r.finalized,
            r.error.as_deref().map(quoted).unwrap_or_else(|| "null".into()));
        drop(r);
        if let Err(e) = std::fs::write(self.path.parent().unwrap().join("capture-video.json"), body)
        {
            eprintln!("[capture-video] receipt failed: {e}");
        }
    }
    fn fail(&self, error: String) {
        self.aborted.store(true, Ordering::Release);
        {
            let mut r = self.receipt.lock().unwrap();
            if r.error.is_none() {
                r.error = Some(error);
            }
            r.finalized = false;
        }
        let mut child = self.child.lock().unwrap();
        if let Some(child) = child.as_mut() {
            let _ = child.kill(); // Only this recorder's child, never another encoder.
            if let Ok(Some(status)) = child.try_wait() {
                self.receipt.lock().unwrap().exit = status.code();
            }
        }
        drop(child);
        self.save();
    }
}

pub(super) fn abort(error: &str) {
    CLOSED.store(true, Ordering::Release);
    if let Some(control) = CONTROL.get() {
        control.fail(error.into());
    }
}

pub(super) fn bench_index(path: &Path) -> Option<u64> {
    path.file_stem()?
        .to_str()?
        .strip_prefix("bench-")?
        .parse()
        .ok()
}

// No future frame is used to fill an earlier gap. Leading missing indices are
// excluded and first_index records the actual start of the available recording.
fn repeats_before(last: Option<u64>, next: u64) -> Option<u64> {
    match last {
        None => Some(0),
        Some(last) if next > last => Some(next - last - 1),
        _ => None,
    }
}

pub(super) struct VideoWriter {
    control: Arc<Control>,
    stdin: Option<ChildStdin>,
    last: Option<EncodeJob>,
    last_key_bucket: Option<u64>,
    closed: bool,
}
impl VideoWriter {
    pub(super) fn from_env() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os("VOXEL_CAPTURE_VIDEO")?);
        if path.as_os_str().is_empty() {
            return None;
        }
        let fps_text = std::env::var("VOXEL_CAPTURE_VIDEO_FPS").unwrap_or_else(|_| "20".into());
        let fps = fps_text
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=120).contains(n));
        let control = Arc::new(Control {
            path,
            child: Mutex::new(None),
            receipt: Mutex::new(Receipt {
                fps: fps.unwrap_or(20),
                ..Default::default()
            }),
            aborted: AtomicBool::new(false),
        });
        let _ = CONTROL.set(control.clone());
        let error = if !control.path.is_absolute() {
            Some("VOXEL_CAPTURE_VIDEO must be absolute")
        } else if fps.is_none() {
            Some("invalid VOXEL_CAPTURE_VIDEO_FPS")
        } else if CLOSED.load(Ordering::Acquire) {
            Some("recording already flushed")
        } else {
            None
        };
        if let Some(parent) = control.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(error) = error {
            control.fail(error.into());
        }
        Some(Self {
            control,
            stdin: None,
            last: None,
            last_key_bucket: None,
            closed: false,
        })
    }
    fn start(&mut self, job: &EncodeJob) -> std::io::Result<()> {
        {
            let mut receipt = self.control.receipt.lock().unwrap();
            receipt.width = job.width;
            receipt.height = job.height;
        }
        if job.width % 2 != 0 || job.height % 2 != 0 {
            return Err(std::io::Error::other(
                "yuv420p requires even native dimensions",
            ));
        }
        let fps = self.control.receipt.lock().unwrap().fps;
        let stderr = std::fs::File::create(self.control.path.with_extension("ffmpeg.log"))?;
        let mut command = Command::new("ffmpeg");
        command
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-n",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                &format!("{}x{}", job.width, job.height),
                "-framerate",
                &fps.to_string(),
                "-i",
                "pipe:0",
                "-an",
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p4",
                "-tune",
                "ll",
                "-rc",
                "vbr",
                "-b:v",
                "4M",
                "-maxrate",
                "6M",
                "-bufsize",
                "12M",
                "-pix_fmt",
                "yuv420p",
                "-movflags",
                "+faststart",
            ])
            .arg(&self.control.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(stderr);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn()?;
        self.stdin = child.stdin.take();
        {
            let mut slot = self.control.child.lock().unwrap();
            if self.control.aborted.load(Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::other(
                    "recording aborted during encoder launch",
                ));
            }
            *slot = Some(child);
        }
        let mut receipt = self.control.receipt.lock().unwrap();
        receipt.width = job.width;
        receipt.height = job.height;
        Ok(())
    }
    pub(super) fn frame(&mut self, job: EncodeJob) {
        if self.closed {
            eprintln!("[capture-video] ignored late frame after flush");
            return;
        }
        let Some(index) = bench_index(&job.path) else {
            super::write_png(job);
            return;
        };
        if self.control.aborted.load(Ordering::Acquire) {
            return;
        }
        let (last, fps) = {
            let r = self.control.receipt.lock().unwrap();
            (r.last, r.fps)
        };
        let Some(repeats) = repeats_before(last, index) else {
            self.control.receipt.lock().unwrap().late += 1;
            return;
        };
        // Corrupt/unrelated shot counters must not write hours of repeated data.
        if repeats > u64::from(fps) * 30 {
            self.control
                .fail("capture gap exceeds bounded 30-second repeat budget".into());
            return;
        }
        let result = (|| -> std::io::Result<()> {
            if self.stdin.is_none() {
                self.start(&job)?;
            }
            let r = self.control.receipt.lock().unwrap();
            if (job.width, job.height) != (r.width, r.height)
                || job.rgba.len() != job.width as usize * job.height as usize * 4
            {
                return Err(std::io::Error::other(
                    "native frame dimensions changed or invalid RGBA length",
                ));
            }
            drop(r);
            let stdin = self.stdin.as_mut().unwrap();
            if let Some(previous) = &self.last {
                for _ in 0..repeats {
                    if self.control.aborted.load(Ordering::Acquire) {
                        return Err(std::io::Error::other("recording aborted"));
                    }
                    stdin.write_all(&previous.rgba)?;
                    self.control.receipt.lock().unwrap().repeated += 1;
                }
            }
            stdin.write_all(&job.rgba)?;
            let mut r = self.control.receipt.lock().unwrap();
            if r.first.is_none() {
                r.first = Some(index);
            }
            r.last = Some(index);
            r.unique += 1;
            Ok(())
        })();
        if let Err(error) = result {
            self.control.fail(error.to_string());
            self.stdin.take();
            return;
        }
        let bucket = index / (u64::from(fps) * 10);
        if self.last_key_bucket != Some(bucket) {
            super::write_png(EncodeJob {
                path: job.path.clone(),
                rgba: job.rgba.clone(),
                width: job.width,
                height: job.height,
            });
            self.last_key_bucket = Some(bucket);
        }
        self.last = Some(job);
    }
    pub(super) fn finish(&mut self, deadline: Instant) -> bool {
        if self.closed {
            return self.control.receipt.lock().unwrap().finalized;
        }
        self.closed = true;
        CLOSED.store(true, Ordering::Release);
        self.stdin.take(); // EOF lets the MP4 muxer finalize.
        let mut exit = None;
        loop {
            let result = self
                .control
                .child
                .lock()
                .unwrap()
                .as_mut()
                .map(|c| c.try_wait());
            match result {
                Some(Ok(Some(status))) => {
                    exit = Some(status);
                    break;
                }
                Some(Ok(None)) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                None => break,
                Some(Err(error)) => {
                    self.control.fail(error.to_string());
                    break;
                }
                _ => {
                    self.control.fail("FFmpeg finalization timed out".into());
                    break;
                }
            }
        }
        {
            let mut r = self.control.receipt.lock().unwrap();
            if let Some(status) = exit {
                r.exit = status.code();
                if !status.success() && r.error.is_none() {
                    r.error = Some(format!("FFmpeg exited {status}; see ffmpeg.log"));
                }
            }
            if r.unique == 0 && r.error.is_none() {
                r.error = Some("no native video frames received".into());
            }
            r.finalized = r.error.is_none() && exit.is_some_and(|e| e.success());
        }
        if let Some(last) = self.last.take() {
            super::write_png(last);
        }
        self.control.save();
        let r = self.control.receipt.lock().unwrap();
        println!(
            "[capture-video] finalized={} unique={} repeated={} first={:?} last={:?} path={}",
            r.finalized,
            r.unique,
            r.repeated,
            r.first,
            r.last,
            self.control.path.display()
        );
        r.finalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chronology_repeats_only_previous_real_frames_and_never_rewinds() {
        assert_eq!(repeats_before(None, 7), Some(0));
        assert_eq!(repeats_before(Some(7), 8), Some(0));
        assert_eq!(repeats_before(Some(8), 12), Some(3));
        assert_eq!(repeats_before(Some(12), 12), None);
        assert_eq!(repeats_before(Some(12), 11), None);
        assert_eq!(bench_index(Path::new("bench-123.png")), Some(123));
        assert_eq!(bench_index(Path::new("camera.png")), None);
    }
}
