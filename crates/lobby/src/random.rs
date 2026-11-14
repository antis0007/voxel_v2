//! Operating-system randomness for session codes. Fails closed: an error means no code is issued.
//! Built without a crate so the lobby adds no dependency.

use std::io;

#[cfg(unix)]
pub fn fill(buffer: &mut [u8]) -> io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buffer)
}

#[cfg(windows)]
pub fn fill(buffer: &mut [u8]) -> io::Result<()> {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        // RtlGenRandom, exported as SystemFunction036: the system CSPRNG.
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    for chunk in buffer.chunks_mut(u32::MAX as usize) {
        // SAFETY: the pointer and length describe a writable slice owned by this call.
        let ok = unsafe { SystemFunction036(chunk.as_mut_ptr(), chunk.len() as u32) };
        if ok == 0 {
            return Err(io::Error::other("system random source failed"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_the_buffer_with_varying_bytes() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        fill(&mut a).expect("os random");
        fill(&mut b).expect("os random");
        assert_ne!(a, b);
        assert!(a.iter().any(|&byte| byte != 0));
    }
}
