//! Compatibility contract shared by the before/after network codec implementations.
use super::*;

#[derive(serde::Deserialize)]
struct Resources(#[serde(deserialize_with = "read_resources")] [u32; RESOURCE_COUNT]);
#[derive(serde::Deserialize)]
struct Gases(#[serde(deserialize_with = "read_gas_counts")] [u32; SPECIES]);

#[test]
fn machine_counts_keep_tuple_shape_and_round_trip_every_slot() {
    let counts = KindCounts(std::array::from_fn(|i| i as u32 * 101));
    let values: Vec<_> = counts.iter().copied().collect();
    let json = serde_json::to_string(&counts).unwrap();
    assert_eq!(json, serde_json::to_string(&values).unwrap());
    assert_eq!(serde_json::from_str::<KindCounts>(&json).unwrap(), counts);
    let tuple = format!(
        "({})",
        values
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(ron::to_string(&counts).unwrap(), tuple);
    assert_eq!(ron::from_str::<KindCounts>(&tuple).unwrap(), counts);
}

#[test]
fn machine_counts_require_exact_width_and_bound_every_slot() {
    assert_eq!(Kind::COUNT, 40);
    for len in 0..=41 {
        let json = serde_json::to_string(&vec![4096_u32; len]).unwrap();
        assert_eq!(
            serde_json::from_str::<KindCounts>(&json).is_ok(),
            len == 40,
            "width {len}"
        );
    }
    for slot in 0..40 {
        for value in [4097, 65536, u32::MAX] {
            let mut values = vec![0_u32; 40];
            values[slot] = value;
            let error =
                serde_json::from_str::<KindCounts>(&serde_json::to_string(&values).unwrap())
                    .unwrap_err();
            assert!(
                error.to_string().contains("machine count exceeds capacity"),
                "slot {slot}: {error}"
            );
        }
    }
}

#[test]
fn other_network_arrays_keep_legacy_widths_and_full_u32_values() {
    assert_eq!(
        (RESOURCE_COUNT, SPECIES, blocks::Material::COUNT),
        (25, 6, 35)
    );
    for len in 0..=26 {
        let json = serde_json::to_string(&vec![u32::MAX; len]).unwrap();
        let result = serde_json::from_str::<Resources>(&json);
        if [22, 23, 24, 25].contains(&len) {
            let Resources(values) = result.unwrap();
            assert!(values[..len].iter().all(|&count| count == u32::MAX));
            assert!(values[len..].iter().all(|&count| count == 0));
        } else {
            assert!(result.is_err(), "resource width {len}");
        }
    }
    for len in 0..=7 {
        let json = serde_json::to_string(&vec![u32::MAX; len]).unwrap();
        let result = serde_json::from_str::<Gases>(&json);
        if [5, 6].contains(&len) {
            let Gases(values) = result.unwrap();
            assert!(values[..len].iter().all(|&count| count == u32::MAX));
            assert!(values[len..].iter().all(|&count| count == 0));
        } else {
            assert!(result.is_err(), "gas width {len}");
        }
    }
    for len in 0..=36 {
        let counts = vec![u32::MAX; len];
        let json = serde_json::json!({"input": counts, "output": counts}).to_string();
        let result = serde_json::from_str::<blocks::Buffers>(&json);
        if [7, 27, 30, 35].contains(&len) {
            let buffers = result.unwrap();
            for values in [&buffers.input, &buffers.output] {
                assert!(values[..len].iter().all(|&count| count == u32::MAX));
                assert!(values[len..].iter().all(|&count| count == 0));
            }
        } else {
            assert!(result.is_err(), "block width {len}");
        }
    }
}

#[test]
fn count_arrays_reject_malformed_values_at_both_ends() {
    for invalid in ["null", "-1", "1.5", "\"4\"", "4294967296"] {
        for len in [6, 25, 35, 40] {
            for slot in [0, len - 1] {
                let mut values = vec!["0"; len];
                values[slot] = invalid;
                let json = format!("[{}]", values.join(","));
                let accepted = match len {
                    6 => serde_json::from_str::<Gases>(&json).is_ok(),
                    25 => serde_json::from_str::<Resources>(&json).is_ok(),
                    35 => serde_json::from_str::<blocks::Buffers>(&format!(
                        "{{\"input\":{json},\"output\":{json}}}"
                    ))
                    .is_ok(),
                    40 => serde_json::from_str::<KindCounts>(&json).is_ok(),
                    _ => unreachable!(),
                };
                assert!(!accepted, "width {len}, slot {slot}, value {invalid}");
            }
        }
    }
}

#[test]
fn saved_machine_counts_keep_their_separate_historical_limit() {
    for format in 8..=FACTORY_FORMAT {
        for count in [4096, 4097, 65536, 65537] {
            let mut factory = Factory::default();
            factory.kits[0] = count;
            let mut bytes = Vec::new();
            factory.encode_format(&mut bytes, format);
            let mut words = bytes
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes(word.try_into().unwrap()));
            let restored = Factory::decode(
                &mut || {
                    words
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("truncated fixture"))
                },
                1,
                format,
            );
            if count <= 65536 {
                assert_eq!(restored.unwrap().kits[0], count, "format {format}");
                assert!(words.next().is_none());
            } else {
                assert_eq!(
                    restored.err().unwrap().to_string(),
                    "Invalid factory kit count"
                );
            }
        }
    }
}

#[test]
fn ron_wire_arrays_preserve_legacy_and_malformed_input_contracts() {
    #[derive(serde::Deserialize)]
    struct ResourceField {
        #[serde(rename = "values", deserialize_with = "read_resources")]
        _values: [u32; RESOURCE_COUNT],
    }
    #[derive(serde::Deserialize)]
    struct GasField {
        #[serde(rename = "values", deserialize_with = "read_gas_counts")]
        _values: [u32; SPECIES],
    }
    fn accepts(tuple: &str, array: usize) -> bool {
        let field = format!("(values:{tuple})");
        match array {
            0 => ron::from_str::<ResourceField>(&field).is_ok(),
            1 => ron::from_str::<GasField>(&field).is_ok(),
            2 => {
                ron::from_str::<blocks::Buffers>(&format!("(input:{tuple},output:{tuple})")).is_ok()
            }
            3 => ron::from_str::<KindCounts>(tuple).is_ok(),
            _ => unreachable!(),
        }
    }
    for (array, width, previous) in [
        (0, 25, &[22, 23, 24][..]),
        (1, 6, &[5][..]),
        (2, 35, &[7, 27, 30][..]),
        (3, 40, &[][..]),
    ] {
        for len in 0..=width + 1 {
            let tuple = format!("({})", "4096,".repeat(len));
            assert_eq!(
                accepts(&tuple, array),
                len == width || previous.contains(&len),
                "array {array}, width {len}"
            );
        }
        for invalid in ["None", "-1", "1.5", "\"4\"", "4294967296"] {
            for slot in [0, width - 1] {
                let mut values = vec!["0"; width];
                values[slot] = invalid;
                assert!(
                    !accepts(&format!("({},)", values.join(",")), array),
                    "array {array}, slot {slot}, value {invalid}"
                );
            }
        }
    }
    let too_many = format!("({})", "4097,".repeat(40));
    assert!(
        ron::from_str::<KindCounts>(&too_many)
            .unwrap_err()
            .to_string()
            .contains("machine count exceeds capacity")
    );
}
