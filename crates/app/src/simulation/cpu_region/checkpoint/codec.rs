use engine_sim::Cell;
use glam::IVec3;

pub(in crate::simulation::cpu_region) fn number(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn position(out: &mut Vec<u8>, at: IVec3) {
    for value in at.to_array() {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

pub(in crate::simulation::cpu_region) fn cell(out: &mut Vec<u8>, value: Cell) {
    out.extend_from_slice(&[value.material, value.volume]);
    out.extend_from_slice(&value.temperature.to_le_bytes());
    out.extend_from_slice(&value.pressure.to_le_bytes());
    out.extend_from_slice(&value.charge.to_le_bytes());
}

pub(super) fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

pub(crate) struct Reader<'a>(pub &'a [u8]);

impl Reader<'_> {
    pub fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let bytes = self.0.get(..N).ok_or("truncated CPU checkpoint")?;
        let mut value = [0; N];
        value.copy_from_slice(bytes);
        self.0 = &self.0[N..];
        Ok(value)
    }

    pub fn number(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }

    pub fn position(&mut self) -> Result<IVec3, String> {
        Ok(IVec3::new(
            i32::from_le_bytes(self.bytes()?),
            i32::from_le_bytes(self.bytes()?),
            i32::from_le_bytes(self.bytes()?),
        ))
    }

    pub fn cell(&mut self) -> Result<Cell, String> {
        let [material, volume] = self.bytes()?;
        Ok(Cell {
            material,
            volume,
            temperature: u16::from_le_bytes(self.bytes()?),
            pressure: i16::from_le_bytes(self.bytes()?),
            charge: i16::from_le_bytes(self.bytes()?),
        })
    }

    pub fn count(&mut self, maximum: usize) -> Result<usize, String> {
        let value = self.number()?;
        if value > maximum as u64 {
            return Err("CPU checkpoint count exceeds capacity".into());
        }
        Ok(value as usize)
    }
}
