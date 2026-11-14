//! One immutable image may await publication per peer.
use super::*;
pub const ACK: u8 = 35;

#[derive(Default)]
pub struct HostGate {
    committed: u64,
    pending: Option<(u64, bool, Instant)>,
}
impl HostGate {
    pub fn can_capture(&self) -> bool {
        self.pending.is_none()
    }
    pub fn next_sequence(&self) -> Result<u64> {
        ensure!(self.can_capture(), "image still awaits publication");
        self.committed
            .checked_add(1)
            .context("image sequence exhausted")
    }
    pub fn begin(&mut self, sequence: u64, terrain: bool, now: Instant) -> Result<()> {
        ensure!(sequence == self.next_sequence()?, "image sequence mismatch");
        self.pending = Some((sequence, terrain, now));
        Ok(())
    }
    pub fn acknowledge(&mut self, sequence: u64, fully_sent: bool) -> Result<bool> {
        ensure!(sequence != 0, "invalid image acknowledgement");
        if sequence <= self.committed {
            return Ok(false);
        }
        let (expected, terrain, _) = self.pending.context("unsolicited image acknowledgement")?;
        ensure!(
            sequence == expected && fully_sent,
            "image acknowledgement out of order"
        );
        self.committed = sequence;
        self.pending = None;
        Ok(terrain)
    }
    pub fn check_timeout(&self, now: Instant) -> Result<()> {
        ensure!(
            self.pending
                .is_none_or(|(_, _, began)| now.saturating_duration_since(began) <= SYNC_TIMEOUT),
            "gameplay image publication timed out"
        );
        Ok(())
    }
}
pub fn ack(sequence: u64) -> Vec<u8> {
    let mut bytes = vec![ACK];
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes
}
pub fn read_ack(bytes: &[u8]) -> Result<u64> {
    ensure!(
        bytes.len() == 9 && bytes[0] == ACK,
        "invalid image acknowledgement"
    );
    Ok(u64::from_le_bytes(bytes[1..].try_into()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_ack_requires_full_send_and_exact_sequence() {
        let now = Instant::now();
        let mut gate = HostGate::default();
        assert!(gate.acknowledge(0, true).is_err());
        assert!(gate.acknowledge(1, true).is_err());
        gate.begin(1, true, now).unwrap();
        assert!(!gate.can_capture());
        assert!(gate.next_sequence().is_err());
        assert!(gate.acknowledge(1, false).is_err());
        assert!(gate.acknowledge(2, true).is_err());
        assert!(gate.acknowledge(1, true).unwrap());
        assert!(!gate.acknowledge(1, true).unwrap());
        gate.begin(2, false, now).unwrap();
        assert!(!gate.acknowledge(1, true).unwrap());
        assert!(!gate.can_capture());
        assert!(!gate.acknowledge(2, true).unwrap());
        assert!(gate.can_capture());
    }
    #[test]
    fn image_ack_bounds_and_stall_timeout() {
        assert_eq!(read_ack(&ack(u64::MAX)).unwrap(), u64::MAX);
        assert!(read_ack(&[ACK]).is_err());
        assert!(read_ack(&[0; 9]).is_err());
        let now = Instant::now();
        let mut gate = HostGate::default();
        gate.begin(1, false, now).unwrap();
        assert!(gate.check_timeout(now + SYNC_TIMEOUT).is_ok());
        assert!(
            gate.check_timeout(now + SYNC_TIMEOUT + Duration::from_millis(1))
                .is_err()
        );
    }
}
