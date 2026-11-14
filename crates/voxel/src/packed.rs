//! Compact block arrays for regenerable caches: one value, a palette, or dense.
//!
//! Palette order is first appearance, so equal inputs pack to equal bytes.
use engine_content::BlockId;

/// Widest palette stored as indices; a wider one costs more than plain ids.
pub const MAX_PALETTE: usize = 16;

/// Block array held as one value, palette indices or plain ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackedBlocks {
    Uniform { block: BlockId, len: usize },
    Indexed { palette: Box<[BlockId]>, bits: u8, words: Box<[u64]>, len: usize },
    Dense(Box<[BlockId]>),
}

fn index_bits(distinct: usize) -> u8 {
    match distinct {
        0..=2 => 1,
        3..=4 => 2,
        _ => 4,
    }
}

impl PackedBlocks {
    /// Packs `blocks` into the smallest of the three forms.
    pub fn pack(blocks: &[BlockId]) -> Self {
        let mut slot = [u8::MAX; BlockId::COUNT];
        let mut palette = Vec::with_capacity(MAX_PALETTE);
        for &block in blocks {
            let entry = &mut slot[block as usize];
            if *entry == u8::MAX {
                if palette.len() == MAX_PALETTE {
                    return Self::Dense(blocks.into());
                }
                *entry = palette.len() as u8;
                palette.push(block);
            }
        }
        match palette.as_slice() {
            [] => Self::Dense(Box::default()),
            &[block] => Self::Uniform { block, len: blocks.len() },
            _ => {
                let bits = index_bits(palette.len());
                let per_word = 64 / usize::from(bits);
                let words = blocks
                    .chunks(per_word)
                    .map(|cells| {
                        cells.iter().enumerate().fold(0u64, |word, (k, &block)| {
                            word | u64::from(slot[block as usize]) << (k * usize::from(bits))
                        })
                    })
                    .collect();
                Self::Indexed { palette: palette.into(), bits, words, len: blocks.len() }
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Uniform { len, .. } | Self::Indexed { len, .. } => *len,
            Self::Dense(blocks) => blocks.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The single block when every cell holds the same one.
    pub fn uniform(&self) -> Option<BlockId> {
        match self {
            Self::Uniform { block, .. } => Some(*block),
            _ => None,
        }
    }

    /// Block at `index`, or `None` past the end.
    pub fn get(&self, index: usize) -> Option<BlockId> {
        if index >= self.len() {
            return None;
        }
        match self {
            Self::Uniform { block, .. } => Some(*block),
            Self::Dense(blocks) => blocks.get(index).copied(),
            Self::Indexed { palette, bits, words, .. } => {
                let bits = usize::from(*bits);
                let per_word = 64 / bits;
                let word = words.get(index / per_word)?;
                let entry = (word >> ((index % per_word) * bits)) & ((1 << bits) - 1);
                palette.get(entry as usize).copied()
            }
        }
    }

    /// Replaces `out` with the plain ids, reusing its allocation.
    pub fn unpack_into(&self, out: &mut Vec<BlockId>) {
        out.clear();
        match self {
            Self::Uniform { block, len } => out.resize(*len, *block),
            Self::Dense(blocks) => out.extend_from_slice(blocks),
            Self::Indexed { palette, bits, words, len } => {
                let Some(&first) = palette.first() else { return };
                // A full table lets the masked lookup skip its bounds check.
                let mut table = [first; MAX_PALETTE];
                for (entry, &block) in table.iter_mut().zip(palette.iter()) {
                    *entry = block;
                }
                let bits = usize::from(*bits);
                let per_word = 64 / bits;
                let mask = (1u64 << bits) - 1;
                out.reserve_exact(*len);
                for (&word, start) in words.iter().zip((0..*len).step_by(per_word)) {
                    let cells = per_word.min(*len - start);
                    out.extend((0..cells).map(|k| {
                        table[((word >> (k * bits)) & mask) as usize % MAX_PALETTE]
                    }));
                }
            }
        }
    }

    pub fn unpack(&self) -> Vec<BlockId> {
        let mut out = Vec::new();
        self.unpack_into(&mut out);
        out
    }

    /// Heap bytes this value owns.
    pub fn allocation_bytes(&self) -> usize {
        match self {
            Self::Uniform { .. } => 0,
            Self::Indexed { palette, words, .. } => palette.len() + words.len() * 8,
            Self::Dense(blocks) => blocks.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_content::ALL_BLOCK_IDS;
    use proptest::prelude::*;

    const PADDED: usize = 34 * 34 * 34;

    fn striped(distinct: usize, len: usize) -> Vec<BlockId> {
        (0..len).map(|i| ALL_BLOCK_IDS[(i * 7 + i / 5) % distinct]).collect()
    }

    fn round_trips(blocks: &[BlockId]) -> PackedBlocks {
        let packed = PackedBlocks::pack(blocks);
        assert_eq!(packed.len(), blocks.len());
        assert_eq!(packed.unpack(), blocks);
        for index in (0..blocks.len()).step_by(97).chain(blocks.len().saturating_sub(70)..blocks.len()) {
            assert_eq!(packed.get(index), Some(blocks[index]), "cell {index}");
        }
        assert_eq!(packed.get(blocks.len()), None);
        packed
    }

    #[test]
    fn one_value_source_owns_no_heap() {
        let packed = round_trips(&vec![BlockId::Stone; PADDED]);
        assert_eq!(packed.uniform(), Some(BlockId::Stone));
        assert_eq!(packed.allocation_bytes(), 0);
    }

    #[test]
    fn palette_widths_follow_the_distinct_count() {
        for (distinct, bits) in [(2, 1), (3, 2), (4, 2), (5, 4), (16, 4)] {
            let packed = round_trips(&striped(distinct, PADDED));
            let PackedBlocks::Indexed { bits: got, words, .. } = &packed else {
                panic!("{distinct} ids should be indexed, got {packed:?}");
            };
            assert_eq!(*got, bits, "{distinct} ids");
            assert_eq!(words.len(), (PADDED * usize::from(bits)).div_ceil(64));
        }
    }

    #[test]
    fn seventeen_ids_stay_dense() {
        let packed = round_trips(&striped(17, PADDED));
        assert!(matches!(packed, PackedBlocks::Dense(_)));
        assert_eq!(packed.allocation_bytes(), PADDED);
    }

    #[test]
    fn empty_and_ragged_lengths_round_trip() {
        round_trips(&[]);
        for len in [1, 15, 16, 17, 63, 64, 65, 129] {
            for distinct in [1, 2, 3, 9] {
                round_trips(&striped(distinct, len));
            }
        }
    }

    #[test]
    fn equal_inputs_pack_to_equal_values() {
        let blocks = striped(6, 4096);
        assert_eq!(PackedBlocks::pack(&blocks), PackedBlocks::pack(&blocks));
    }

    proptest! {
        #[test]
        fn any_block_array_round_trips(
            distinct in 1usize..40,
            picks in proptest::collection::vec(any::<u16>(), 0..600),
        ) {
            let distinct = distinct.min(ALL_BLOCK_IDS.len());
            let blocks: Vec<BlockId> =
                picks.iter().map(|&p| ALL_BLOCK_IDS[usize::from(p) % distinct]).collect();
            let packed = PackedBlocks::pack(&blocks);
            prop_assert_eq!(packed.unpack(), blocks);
        }
    }
}
