//! Exact completed Stage values. This is not a resumable solve or save format.
use super::super::{Grid, Stage};

pub(super) type Result<T> = std::result::Result<T, String>;
const MAGIC: &[u8; 8] = b"AXSTG001";
const MAX_NODES: usize = 176 * 176;
pub(super) const MAX_BYTES: usize = 400 * 1024;

fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.to_owned()) }
}

fn grid_len(grid: &Grid) -> Result<usize> {
    let Grid { n, cell_m, min_m } = *grid;
    require([(22, 32.0_f64), (44, 16.0), (88, 8.0), (176, 4.0)].iter()
        .any(|&(side, spacing)| n == side && cell_m.to_bits() == spacing.to_bits()),
        "unsupported stage grid")?;
    require(min_m.iter().all(|x| x.is_finite()), "nonfinite stage origin")?;
    let len = n.checked_mul(n).ok_or("stage size overflow")?;
    require(len <= MAX_NODES, "stage too large")?;
    Ok(len)
}

pub(super) fn same_grid(a: &Grid, b: &Grid) -> bool {
    let Grid { n: an, cell_m: ac, min_m: am } = *a;
    let Grid { n: bn, cell_m: bc, min_m: bm } = *b;
    an == bn && ac.to_bits() == bc.to_bits()
        && am.map(f64::to_bits) == bm.map(f64::to_bits)
}

pub(super) fn encode(stage: &Stage) -> Result<Vec<u8>> {
    // Exhaustive guards force a review when either authoritative layout grows.
    let Stage { grid, rock_delta, soil, base } = stage;
    let Grid { n, cell_m, min_m } = *grid;
    let len = grid_len(grid)?;
    require(rock_delta.len() == len && soil.len() == len
        && base.as_ref().is_none_or(|b| b.len() == len), "stage field length")?;
    let size = 8 + 4 + 24 + 1 + (2 + usize::from(base.is_some())) * (4 + 4 * len);
    require(size <= MAX_BYTES, "stage payload too large")?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|_| "stage allocation failed")?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(n as u32).to_le_bytes());
    for v in [cell_m, min_m[0], min_m[1]] { bytes.extend_from_slice(&v.to_bits().to_le_bytes()); }
    bytes.push(u8::from(base.is_some()));
    for values in [Some(rock_delta), Some(soil), base.as_ref()].into_iter().flatten() {
        bytes.extend_from_slice(&(len as u32).to_le_bytes());
        for &v in values {
            require(v.is_finite(), "nonfinite stage field")?;
            bytes.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }
    Ok(bytes)
}

struct Reader<'a> { bytes: &'a [u8], at: usize }
impl Reader<'_> {
    fn raw<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or("stage offset overflow")?;
        let source = self.bytes.get(self.at..end).ok_or("truncated stage")?;
        let mut result = [0; N];
        result.copy_from_slice(source);
        self.at = end;
        Ok(result)
    }
    fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.raw()?)) }
    fn f64(&mut self) -> Result<f64> {
        let value = f64::from_bits(u64::from_le_bytes(self.raw()?));
        require(value.is_finite(), "nonfinite stage grid")?;
        Ok(value)
    }
    fn field(&mut self, len: usize) -> Result<Vec<f32>> {
        require(self.u32()? as usize == len, "stage field length")?;
        require(self.bytes.len() - self.at >= len * 4, "truncated stage field")?;
        let mut values = Vec::new();
        values.try_reserve_exact(len).map_err(|_| "stage allocation failed")?;
        for _ in 0..len {
            let value = f32::from_bits(self.u32()?);
            require(value.is_finite(), "nonfinite stage field")?;
            values.push(value);
        }
        Ok(values)
    }
}

pub(super) fn decode(bytes: &[u8], expected: &Grid, expected_base: bool) -> Result<Stage> {
    require(bytes.len() <= MAX_BYTES, "stage payload too large")?;
    let len = grid_len(expected)?;
    let mut reader = Reader { bytes, at: 0 };
    require(&reader.raw::<8>()? == MAGIC, "stage schema")?;
    let grid = Grid { n: reader.u32()? as usize, cell_m: reader.f64()?,
        min_m: [reader.f64()?, reader.f64()?] };
    require(same_grid(&grid, expected), "stage grid identity")?;
    let flag = reader.raw::<1>()?[0];
    require(flag <= 1 && (flag == 1) == expected_base, "stage base presence")?;
    let rock_delta = reader.field(len)?;
    let soil = reader.field(len)?;
    let base = if flag == 1 { Some(reader.field(len)?) } else { None };
    require(reader.at == bytes.len(), "trailing stage bytes")?;
    Ok(Stage { grid, rock_delta, soil, base })
}
