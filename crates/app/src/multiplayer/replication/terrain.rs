//! Terrain changes frozen with one authoritative gameplay image.
use super::*;
use engine_netcode::wire::put_i64;

pub const MAX_CHUNKS: usize = 512;
pub const MAX_STAGED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
pub struct Frame {
    pub cells: Vec<([i32; 3], u32)>,
    pub chunk: Option<Chunk>,
}
impl Frame {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.chunk.is_none()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.cells.len() <= super::super::MAX_EDITS,
            "terrain frame cell budget exceeded"
        );
        let mut seen = HashSet::new();
        let mut chunks = HashSet::new();
        for &(p, block) in &self.cells {
            validate_cell(p, block)?;
            ensure!(seen.insert(p), "duplicate terrain frame cell");
            chunks.insert(world::chunk_coord_from_world_block(IVec3::from_array(p)));
            ensure!(
                chunks.len() <= MAX_CHUNKS,
                "terrain frame chunk budget exceeded"
            );
        }
        if let Some(chunk) = &self.chunk {
            validate_chunk(chunk)?;
            chunks.insert(IVec3::from_array(chunk.coord));
        }
        ensure!(
            chunks.len() <= MAX_CHUNKS,
            "terrain frame chunk budget exceeded"
        );
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut out = Vec::new();
        put_u64(&mut out, self.cells.len() as u64);
        for &(p, block) in &self.cells {
            for v in p {
                put_i64(&mut out, i64::from(v));
            }
            put_u64(&mut out, u64::from(block));
        }
        out.push(u8::from(self.chunk.is_some()));
        if let Some(chunk) = &self.chunk {
            for v in chunk.coord {
                put_i64(&mut out, i64::from(v));
            }
            put_u64(&mut out, chunk.runs.len() as u64);
            for &(count, block) in &chunk.runs {
                put_u64(&mut out, u64::from(count));
                put_u64(&mut out, u64::from(block));
            }
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let count = usize::try_from(wire(r.u64())?)?;
        ensure!(
            count <= super::super::MAX_EDITS && count <= r.remaining() / 4,
            "terrain frame cell budget exceeded"
        );
        let mut cells = Vec::with_capacity(count);
        for _ in 0..count {
            let p = [wire(r.i32())?, wire(r.i32())?, wire(r.i32())?];
            cells.push((p, wire(r.u32())?));
        }
        let chunk = match wire(r.u8())? {
            0 => None,
            1 => {
                let coord = [wire(r.i32())?, wire(r.i32())?, wire(r.i32())?];
                let count = usize::try_from(wire(r.u64())?)?;
                ensure!(
                    count <= 32768 && count <= r.remaining() / 2,
                    "terrain frame run budget exceeded"
                );
                let mut runs = Vec::with_capacity(count);
                for _ in 0..count {
                    runs.push((u16::try_from(wire(r.u64())?)?, wire(r.u32())?));
                }
                Some(Chunk { coord, runs })
            }
            _ => anyhow::bail!("invalid terrain frame chunk flag"),
        };
        ensure!(r.done(), "trailing terrain frame data");
        let frame = Self { cells, chunk };
        frame.validate()?;
        Ok(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terrain_frame_round_trip_and_sparse_bound() {
        let cells: Vec<_> = (0..MAX_CHUNKS)
            .map(|i| ([i as i32 * 32 - 8192, 128, -1], BlockId::Stone.as_u32()))
            .collect();
        let mut frame = Frame { cells, chunk: None };
        let decoded = Frame::decode(&frame.encode().unwrap()).unwrap();
        assert_eq!(decoded.cells, frame.cells);
        frame
            .cells
            .push(([20000, 128, -1], BlockId::Stone.as_u32()));
        assert!(frame.encode().is_err());
    }
    #[test]
    fn terrain_frame_rejects_invalid_data_before_publication() {
        let mut frame = Frame {
            cells: vec![([1, 128, -1], BlockId::Air.as_u32())],
            chunk: None,
        };
        frame.cells.push(frame.cells[0]);
        assert!(frame.encode().is_err());
        frame.cells.pop();
        frame.cells[0].1 = BlockId::COUNT as u32;
        assert!(frame.encode().is_err());
        assert!(Frame::decode(&[255; 20]).is_err());
        assert!(Frame::decode(&[0, 2]).is_err());
        frame = Frame {
            cells: Vec::new(),
            chunk: Some(Chunk {
                coord: [-1, 4, 0],
                runs: vec![(32768, BlockId::Air.as_u32())],
            }),
        };
        let mut bytes = frame.encode().unwrap();
        assert!(Frame::decode(&bytes).is_ok());
        bytes.push(0);
        assert!(Frame::decode(&bytes).is_err());
        frame.chunk.as_mut().unwrap().runs[0].0 = 32767;
        assert!(frame.encode().is_err());
    }
}
