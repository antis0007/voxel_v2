//! Current-thread CPU time is diagnostic-only and excludes waiting.
#[cfg(windows)]
mod platform {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct FileTime { low: u32, high: u32 }
    impl FileTime {
        fn ticks(&self) -> u64 { (u64::from(self.high) << 32) | u64::from(self.low) }
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetThreadTimes(thread: *mut c_void, created: *mut FileTime, exited: *mut FileTime,
            kernel: *mut FileTime, user: *mut FileTime) -> i32;
    }
    pub(super) fn ticks() -> Option<u64> {
        let (mut created, mut exited, mut kernel, mut user) =
            (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
        // The pseudo-handle names this thread; all output pointers remain valid through the call.
        let ok = unsafe { GetThreadTimes(GetCurrentThread(), &mut created, &mut exited, &mut kernel, &mut user) };
        (ok != 0).then(|| kernel.ticks().saturating_add(user.ticks()))
    }
}
pub(super) fn ticks() -> Option<u64> {
    #[cfg(windows)] { platform::ticks() }
    #[cfg(not(windows))] { None }
}
#[cfg(test)]
mod tests {
    #[test]
    fn current_thread_clock_is_monotonic_when_available() {
        let before = super::ticks();
        let mut value = 1u64;
        for n in 0..100_000 { value = std::hint::black_box(value.wrapping_mul(3).wrapping_add(n)); }
        std::hint::black_box(value);
        let after = super::ticks();
        #[cfg(windows)] { assert!(before.is_some() && after.is_some()); }
        if let (Some(a), Some(b)) = (before, after) { assert!(b >= a); }
    }
}
