//! Multiplayer array codecs. Width compatibility and per-value admission live
//! here; the disk codec keeps its separate historical limits in persistence.rs.
use super::super::KindCounts;
use super::{
    INGOT_RESOURCES, Kind, LEGACY_GAS_SPECIES, PLACER_RESOURCES, PRE_PLACER_RESOURCES,
    RESOURCE_COUNT, SPECIES,
};

pub(crate) fn read_resources<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<[u32; RESOURCE_COUNT], D::Error> {
    read_counts(
        d,
        &[PRE_PLACER_RESOURCES, PLACER_RESOURCES, INGOT_RESOURCES],
    )
}

pub(in crate::world::factory) fn read_gas_counts<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<[u32; SPECIES], D::Error> {
    read_counts(d, &[LEGACY_GAS_SPECIES])
}

pub(in crate::world::factory) fn read_counts<'de, D: serde::Deserializer<'de>, const N: usize>(
    d: D,
    previous: &[usize],
) -> Result<[u32; N], D::Error> {
    d.deserialize_tuple(
        N,
        Counts {
            previous,
            maximum: None,
            expected: "counts or a supported legacy array",
        },
    )
}

// One reader enforces exact or explicitly supported historical widths, zero
// fills an accepted legacy tail, and rejects surplus elements. Value limits
// are independent: resource/gas/block arrays retain the full u32 range, while
// machine kit arrays retain their network-specific limit.
struct Counts<'a, const N: usize> {
    previous: &'a [usize],
    maximum: Option<(u32, &'static str)>,
    expected: &'static str,
}

impl<'de, const N: usize> serde::de::Visitor<'de> for Counts<'_, N> {
    type Value = [u32; N];

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{N} {}", self.expected)
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut counts = [0; N];
        for (index, count) in counts.iter_mut().enumerate() {
            match seq.next_element()? {
                Some(value) => {
                    if let Some((limit, message)) = self.maximum
                        && value > limit
                    {
                        return Err(serde::de::Error::custom(message));
                    }
                    *count = value;
                }
                None if self.previous.contains(&index) => return Ok(counts),
                None => return Err(serde::de::Error::invalid_length(index, &self)),
            }
        }
        if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(serde::de::Error::invalid_length(N + 1, &self));
        }
        Ok(counts)
    }
}

impl serde::Serialize for KindCounts {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut tuple = s.serialize_tuple(Kind::COUNT)?;
        for value in &self.0 {
            tuple.serialize_element(value)?;
        }
        tuple.end()
    }
}

impl<'de> serde::Deserialize<'de> for KindCounts {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_tuple(
            Kind::COUNT,
            Counts {
                previous: &[],
                maximum: Some((4096, "machine count exceeds capacity")),
                expected: "machine counts",
            },
        )
        .map(Self)
    }
}
