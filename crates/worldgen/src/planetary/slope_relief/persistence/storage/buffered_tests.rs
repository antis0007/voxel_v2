use super::*;

#[derive(Default)]
struct Sink { bytes: Vec<u8>, writes: usize, flushes: usize, fail_write: bool, fail_flush: bool }
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_write { return Err(io::Error::other("injected write failure")); }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush { return Err(io::Error::other("injected flush failure")); }
        Ok(())
    }
}

#[test]
fn buffered_frame_preserves_bytes_and_coalesces_writes() {
    let key = b"independent-stage-key";
    for size in [0, 1, 8192, MAX_FILE as usize - HEADER as usize - key.len()] {
        let payload: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let mut reference = Sink::default();
        write_entry(&mut reference, key, &payload).unwrap();
        let mut actual = Sink::default();
        write_entry_buffered(&mut actual, key, &payload).unwrap();
        assert_eq!(actual.bytes, reference.bytes);
        assert_eq!(actual.writes, 1);
        assert_eq!(actual.flushes, 1);
        let len = actual.bytes.len() as u64;
        assert_eq!(read_entry(&mut actual.bytes.as_slice(), len, key).unwrap(), payload);
    }
}

#[test]
fn buffered_frame_reports_write_and_flush_errors_without_drop_retry() {
    let mut sink = Sink { fail_write: true, ..Sink::default() };
    let error = write_entry_buffered(&mut sink, b"key", b"payload").unwrap_err();
    assert!(error.to_string().contains("injected write"));
    assert_eq!(sink.writes, 1);
    assert_eq!(sink.flushes, 0);
    let mut sink = Sink { fail_flush: true, ..Sink::default() };
    let error = write_entry_buffered(&mut sink, b"key", b"payload").unwrap_err();
    assert!(error.to_string().contains("injected flush"));
    assert_eq!(sink.writes, 1);
    assert_eq!(sink.flushes, 1);
}

#[test]
fn buffered_frame_rejects_invalid_bounds_before_io() {
    let mut sink = Sink::default();
    assert!(write_entry_buffered(&mut sink, b"", b"x").is_err());
    assert!(write_entry_buffered(&mut sink, b"key", &vec![0; MAX_FILE as usize]).is_err());
    assert_eq!((sink.writes, sink.flushes), (0, 0));
}
