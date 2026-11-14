//! Pending cell changes for one joiner: oldest first, player edits ahead, one entry per cell.
//! Packed as chunk groups, so a coherent front costs four bytes a cell.
use anyhow::{Result, ensure};
use engine_content::BlockId;
use glam::IVec3;
use std::collections::{HashMap, VecDeque};

/// Message tag for packed cell deltas.
pub const TAG: u8 = 28;
/// Whole message budget, matching the RON replication messages.
const PACKET: usize = 900;
/// Chunk coordinate (3 x i32) and cell count (u16).
const GROUP: usize = 14;
/// Local index (u16) and block (u16).
const CELL: usize = 4;
const SIDE: i32 = 32;
const _: () = assert!(BlockId::COUNT <= u16::MAX as usize + 1);

#[derive(Default)]
pub struct CellQueue {
    /// Newest block per pending cell, and whether the cell already has an urgent entry.
    latest: HashMap<[i32; 3], (u32, bool)>,
    urgent: VecDeque<[i32; 3]>,
    normal: VecDeque<[i32; 3]>,
}

impl CellQueue {
    pub fn len(&self) -> usize {
        self.latest.len()
    }

    pub fn is_empty(&self) -> bool {
        self.latest.is_empty()
    }

    /// Records a cell's newest block. A pending cell keeps its place in line, so a cell
    /// that changes every tick cannot push others back; `urgent` moves it ahead of the line.
    pub fn push(&mut self, cell: [i32; 3], block: u32, urgent: bool) {
        match self.latest.get_mut(&cell) {
            Some((current, promoted)) => {
                *current = block;
                if urgent && !*promoted {
                    *promoted = true;
                    self.urgent.push_back(cell);
                }
            }
            None => {
                self.latest.insert(cell, (block, urgent));
                if urgent { self.urgent.push_back(cell) } else { self.normal.push_back(cell) }
            }
        }
    }

    /// The next cell to send. Entries for cells already sent are dropped on the way.
    fn front(&mut self) -> Option<[i32; 3]> {
        loop {
            let line = if self.urgent.is_empty() { &mut self.normal } else { &mut self.urgent };
            let cell = *line.front()?;
            if self.latest.contains_key(&cell) {
                return Some(cell);
            }
            line.pop_front();
        }
    }

    fn pop(&mut self) -> Option<([i32; 3], u32)> {
        let cell = self.front()?;
        let line = if self.urgent.is_empty() { &mut self.normal } else { &mut self.urgent };
        line.pop_front();
        self.latest.remove(&cell).map(|(block, _)| (cell, block))
    }

    /// Removes the oldest cells that fit one message and packs them.
    pub fn take_packet(&mut self) -> Option<(Vec<u8>, Vec<([i32; 3], u32)>)> {
        let mut groups: Vec<([i32; 3], Vec<(u16, u16)>)> = Vec::new();
        let mut taken = Vec::new();
        let mut size = 1;
        while let Some(cell) = self.front() {
            let (chunk, local) = split(cell);
            let group = groups.iter().position(|(c, cells)| *c == chunk && cells.len() < usize::from(u16::MAX));
            let cost = CELL + if group.is_some() { 0 } else { GROUP };
            if size + cost > PACKET {
                break;
            }
            let Some((cell, block)) = self.pop() else { break };
            size += cost;
            let entry = (local, block as u16);
            match group {
                Some(i) => groups[i].1.push(entry),
                None => groups.push((chunk, vec![entry])),
            }
            taken.push((cell, block));
        }
        if taken.is_empty() {
            return None;
        }
        let mut bytes = Vec::with_capacity(size);
        bytes.push(TAG);
        for (chunk, cells) in &groups {
            for v in chunk {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            bytes.extend_from_slice(&(cells.len() as u16).to_le_bytes());
            for (local, block) in cells {
                bytes.extend_from_slice(&local.to_le_bytes());
                bytes.extend_from_slice(&block.to_le_bytes());
            }
        }
        Some((bytes, taken))
    }

    /// Puts back cells a refused message held, ahead of everything else. A cell that
    /// changed again in the meantime keeps its newer block.
    pub fn restore(&mut self, cells: Vec<([i32; 3], u32)>) {
        for (cell, block) in cells.into_iter().rev() {
            if !self.latest.contains_key(&cell) {
                self.latest.insert(cell, (block, true));
                self.urgent.push_front(cell);
            }
        }
    }
}

fn split(cell: [i32; 3]) -> ([i32; 3], u16) {
    let p = IVec3::from_array(cell);
    let chunk = p.div_euclid(IVec3::splat(SIDE));
    let l = p.rem_euclid(IVec3::splat(SIDE));
    // Matches ChunkData::local_index: x + z*32 + y*32*32.
    (chunk.to_array(), (l.x + l.z * SIDE + l.y * SIDE * SIDE) as u16)
}

/// Unpacks a message body (tag removed) into cells and blocks; blocks are checked by the caller.
pub fn decode(mut bytes: &[u8]) -> Result<Vec<([i32; 3], u32)>> {
    ensure!(bytes.len() < PACKET, "oversized delta message");
    let mut cells = Vec::new();
    let take = |n: usize, bytes: &mut &[u8]| -> Result<Vec<u8>> {
        ensure!(bytes.len() >= n, "truncated delta message");
        let (head, rest) = bytes.split_at(n);
        *bytes = rest;
        Ok(head.to_vec())
    };
    while !bytes.is_empty() {
        let head = take(GROUP, &mut bytes)?;
        let word = |i: usize| i32::from_le_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
        let chunk = IVec3::new(word(0), word(4), word(8));
        ensure!(chunk.abs().max_element() <= 31_249, "delta chunk outside world range");
        let count = usize::from(u16::from_le_bytes([head[12], head[13]]));
        ensure!(count > 0, "empty delta group");
        let body = take(count * CELL, &mut bytes)?;
        for entry in body.chunks_exact(CELL) {
            let local = i32::from(u16::from_le_bytes([entry[0], entry[1]]));
            ensure!(local < SIDE * SIDE * SIDE, "delta cell outside its chunk");
            let p = chunk * SIDE + IVec3::new(local % SIDE, local / (SIDE * SIDE), (local / SIDE) % SIDE);
            cells.push((p.to_array(), u32::from(u16::from_le_bytes([entry[2], entry[3]]))));
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(queue: &mut CellQueue) -> Vec<([i32; 3], u32)> {
        let mut out = Vec::new();
        while let Some((bytes, taken)) = queue.take_packet() {
            assert!(bytes.len() <= PACKET);
            let decoded = decode(&bytes[1..]).expect("decodes");
            let mut sorted = taken.clone();
            sorted.sort();
            let mut round = decoded;
            round.sort();
            assert_eq!(round, sorted);
            out.extend(taken);
        }
        out
    }

    #[test]
    fn churning_cells_do_not_starve_older_cells() {
        let mut queue = CellQueue::default();
        // A high coordinate waits first; a low-coordinate front churns behind it.
        queue.push([900, 10, 900], 3, false);
        for tick in 0..50u32 {
            for x in 0..400 {
                queue.push([x, 5, 0], tick % 4, false);
            }
        }
        assert_eq!(queue.len(), 401);
        let (_, first) = queue.take_packet().expect("packet");
        assert_eq!(first[0], ([900, 10, 900], 3));
        // Churned cells went out once each, with their newest block.
        let rest = drain(&mut queue);
        assert_eq!(rest.len() + first.len(), 401);
        assert!(rest.iter().chain(&first).filter(|(c, _)| c[1] == 5).all(|&(_, b)| b == 49 % 4));
    }

    #[test]
    fn player_edits_go_ahead_of_a_backlog() {
        let mut queue = CellQueue::default();
        for x in 0..10_000 {
            queue.push([x, 0, 0], 1, false);
        }
        queue.push([5, 70, 5], 2, true);
        // A queued ambient cell the player then edits is promoted, not duplicated.
        queue.push([9_999, 0, 0], 4, true);
        let (_, first) = queue.take_packet().expect("packet");
        assert_eq!(&first[..2], &[([5, 70, 5], 2), ([9_999, 0, 0], 4)]);
        let rest = drain(&mut queue);
        assert_eq!(rest.len() + first.len(), 10_000 + 1);
        assert!(!rest.iter().any(|(c, _)| *c == [9_999, 0, 0]));
    }

    #[test]
    fn a_coherent_front_packs_many_cells_and_restores_in_order() {
        let mut queue = CellQueue::default();
        for x in -40..40 {
            for z in 0..8 {
                queue.push([x, -3, z], 7, false);
            }
        }
        let (bytes, taken) = queue.take_packet().expect("packet");
        assert!(taken.len() > 150, "{} cells in {} bytes", taken.len(), bytes.len());
        let before = queue.len();
        queue.restore(taken.clone());
        assert_eq!(queue.len(), before + taken.len());
        let (_, again) = queue.take_packet().expect("packet");
        assert_eq!(again[..taken.len()], taken[..]);
    }

    #[test]
    fn malformed_messages_are_refused() {
        assert!(decode(&[1, 2, 3]).is_err());
        let mut group = Vec::new();
        for v in [0i32, 0, 0] {
            group.extend_from_slice(&v.to_le_bytes());
        }
        group.extend_from_slice(&2u16.to_le_bytes());
        group.extend_from_slice(&[0, 0, 1, 0]);
        assert!(decode(&group).is_err(), "count claims two cells, one present");
        group.extend_from_slice(&[0xFF, 0xFF, 1, 0]);
        assert!(decode(&group).is_err(), "local index past the chunk");
    }
}
