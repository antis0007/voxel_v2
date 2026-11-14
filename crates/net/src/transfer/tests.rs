use super::*;

fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index.wrapping_mul(31) % 251) as u8).collect()
}

fn deliver(bytes: Vec<u8>, receiver: &mut Receiver) -> Vec<u8> {
    let mut sender = Sender::new(bytes).expect("sender");
    let mut result = None;
    while let Some(packet) = sender.next() {
        // A rejected transport write cannot advance the stream.
        assert_eq!(sender.next(), Some(packet.clone()));
        result = receiver.receive(&packet).expect("receive");
        sender.accepted();
    }
    result.expect("completed")
}

#[test]
fn checksums_and_framing_match_existing_wire_format() {
    assert_eq!(checksum(b"hello"), 0xa430d84680aabd0b);
    let bytes = payload(CHUNK + 17);
    let mut sender = Sender::new(bytes.clone()).unwrap();
    let mut header = vec![1];
    header.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    header.extend_from_slice(&checksum(&bytes).to_le_bytes());
    assert_eq!(sender.next(), Some(header));
    sender.accepted();
    let mut fragment = vec![2, 0, 0, 0, 0];
    fragment.extend_from_slice(&bytes[..CHUNK]);
    assert_eq!(sender.next(), Some(fragment));
}

#[test]
fn reused_receiver_handles_boundaries_and_largest_allowed_transfer() {
    let mut receiver = Receiver::default();
    for size in [1, CHUNK - 1, CHUNK, CHUNK + 1, CHUNK * 3, MAX_TRANSFER] {
        let bytes = payload(size);
        assert_eq!(deliver(bytes.clone(), &mut receiver), bytes);
    }
    assert!(Sender::new(Vec::new()).is_err());
    assert!(Sender::new(vec![0; MAX_TRANSFER + 1]).is_err());
}

#[test]
fn corruption_is_rejected_and_next_transfer_starts_fresh() {
    let mut receiver = Receiver::default();
    let mut sender = Sender::new(payload(CHUNK + 23)).unwrap();
    receiver.receive(&sender.next().unwrap()).unwrap();
    sender.accepted();
    let mut corrupted = sender.next().unwrap();
    corrupted[5] ^= 1;
    assert!(receiver.receive(&corrupted).unwrap().is_none());
    sender.accepted();
    assert_eq!(receiver.receive(&sender.next().unwrap()), Err(ProtocolError("transfer checksum mismatch")));
    assert_eq!(deliver(payload(CHUNK + 1), &mut receiver), payload(CHUNK + 1));
}

#[test]
fn malformed_fragment_does_not_change_the_running_checksum() {
    let mut receiver = Receiver::default();
    let bytes = payload(CHUNK + 7);
    let mut sender = Sender::new(bytes.clone()).unwrap();
    assert!(receiver.receive(&[2, 0, 0, 0, 0, 7]).is_err());
    receiver.receive(&sender.next().unwrap()).unwrap();
    sender.accepted();
    let first = sender.next().unwrap();
    let mut bad_offset = first.clone();
    bad_offset[1] = 1;
    assert!(receiver.receive(&bad_offset).is_err());
    assert!(receiver.receive(&first).unwrap().is_none());
    assert!(receiver.receive(&first).is_err(), "duplicate data refused");
    sender.accepted();
    assert_eq!(receiver.receive(&sender.next().unwrap()).unwrap(), Some(bytes));
}

#[test]
fn untrusted_header_does_not_allocate_the_advertised_size() {
    let mut receiver = Receiver::default();
    let mut header = vec![1];
    header.extend_from_slice(&(MAX_TRANSFER as u32).to_le_bytes());
    header.extend_from_slice(&0_u64.to_le_bytes());
    receiver.receive(&header).unwrap();
    assert_eq!(receiver.bytes.capacity(), 0);
    assert!(receiver.receive(&header).is_err(), "nested transfer refused");
}
