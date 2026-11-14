use super::*;

fn decode_words(words: &[u32], format: u8) -> anyhow::Result<Factory> {
    let mut cursor = words.iter().copied();
    let factory = Factory::decode(
        &mut || {
            cursor
                .next()
                .ok_or_else(|| anyhow::anyhow!("truncated fixture"))
        },
        1,
        format,
    )?;
    anyhow::ensure!(cursor.next().is_none(), "unconsumed factory fixture");
    Ok(factory)
}

fn words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect()
}

#[test]
fn historical_layout_widths_are_pinned() {
    // Expected numbers are deliberately independent of production constants:
    // changing a width must not silently rewrite an old save layout.
    for format in 0..=u8::MAX {
        let resources = match format {
            0..=6 => 14,
            7..=8 => 19,
            9..=16 => 22,
            17 => 23,
            18..=19 => 24,
            _ => 25,
        };
        let kinds = match format {
            0..=6 => 8,
            7 => 11,
            8 => 16,
            9 => 21,
            10..=11 => 27,
            12..=13 => 33,
            14..=15 => 39,
            _ => 40,
        };
        assert_eq!(resources_in_format(format), resources, "format {format}");
        assert_eq!(Kind::count_in_format(format), kinds, "format {format}");
        assert_eq!(
            gas_species_in_format(format),
            if format < 21 { 5 } else { SPECIES }
        );
    }
    for version in 0..=u8::MAX {
        let expected = match version {
            0..=19 => 19,
            20..=60 => 22,
            61..=62 => 23,
            63..=65 => 24,
            _ => 25,
        };
        assert_eq!(resources_in_save(version), expected, "world save {version}");
    }
}

#[test]
fn every_supported_factory_writer_round_trips_without_trailing_words() {
    let position = IVec3::new(1, 64, -2);
    for format in 8..=FACTORY_FORMAT {
        let mut factory = Factory::default();
        factory.kits[Kind::Storage as usize] = 7;
        let mut node = Node::new(Kind::Storage);
        let last = resources_in_format(format) - 1;
        node.input[0] = 128;
        node.filter = Some(last);
        factory.nodes.insert(position, node);
        // Storage admission follows its authored resource list. The fabricator
        // preserves arbitrary buffered resources across catalogue edits, so it
        // can exercise the last serialized slot without invalid machine stock.
        let mut buffer = Node::new(Kind::Fabricator);
        buffer.input[last] = 64;
        factory.nodes.insert(position + IVec3::X, buffer);
        let mut bytes = Vec::new();
        factory.encode_format(&mut bytes, format);
        let restored = decode_words(&words(&bytes), format)
            .unwrap_or_else(|error| panic!("format {format}: {error}"));
        assert_eq!(restored.kits[Kind::Storage as usize], 7, "format {format}");
        let node = &restored.nodes[&position];
        assert_eq!(node.input[0], 128, "format {format}");
        assert_eq!(node.filter, Some(last), "format {format}");
        let buffer = &restored.nodes[&(position + IVec3::X)];
        assert_eq!(buffer.input[last], 64, "format {format}");
        assert!(buffer.input[last + 1..].iter().all(|count| *count == 0));
        let mut encoded_again = Vec::new();
        restored.encode_format(&mut encoded_again, format);
        assert_eq!(encoded_again, bytes, "format {format}");
    }
}

#[test]
fn pre_writer_legacy_fixtures_preserve_stock_and_default_new_fields() {
    // Construct genuine legacy words instead of using today's writer, which
    // only writes the extended fluid layout introduced in format 8.
    for (format, kinds, resources) in [(6, 8, 14), (7, 11, 19)] {
        let mut fixture = vec![0; kinds];
        fixture[4] = 3; // Storage kit identity.
        fixture.extend([1, 0, 32, 0, 4]); // One storage at (0, 32, 0).
        let mut input = vec![0; resources];
        input[resources - 1] = 192;
        fixture.extend(input);
        fixture.extend(vec![0; resources]); // Output stocks.
        fixture.extend([0, 0, 0, 0, 1]); // Energy, fuel, progress, recipe, enabled.
        if format == 7 {
            fixture.extend([0, 0, 1]);
        } // Mode, upgrade, first-resource filter.
        let restored = decode_words(&fixture, format).unwrap();
        let node = &restored.nodes[&IVec3::new(0, 32, 0)];
        assert_eq!(restored.kits[4], 3);
        assert_eq!(node.input[resources - 1], 192);
        assert!(node.input[resources..].iter().all(|count| *count == 0));
        assert_eq!(node.filter, (format == 7).then_some(0));
        assert_eq!((node.water, node.excavation, node.faces), (0, 0, 0));
        assert!(
            node.exhaust
                .iter()
                .chain(&node.residue)
                .all(|count| *count == 0)
        );
    }
}

#[test]
fn persistence_settles_reserved_shipments_once_and_rejects_truncation() {
    let mut factory = Factory::default();
    let mut node = Node::new(Kind::Storage);
    node.input[0] = 64;
    factory.nodes.insert(IVec3::ZERO, node);
    factory.reserved.insert((IVec3::ZERO, 0), 32);
    let mut bytes = Vec::new();
    factory.encode(&mut bytes);
    let encoded = words(&bytes);
    let restored = decode_words(&encoded, FACTORY_FORMAT).unwrap();
    assert_eq!(restored.nodes[&IVec3::ZERO].input[0], 96);
    assert!(restored.reserved.is_empty());
    let mut saved_again = Vec::new();
    restored.encode(&mut saved_again);
    assert_eq!(saved_again, bytes);
    for end in 0..encoded.len() {
        assert!(
            decode_words(&encoded[..end], FACTORY_FORMAT).is_err(),
            "prefix {end}"
        );
    }
}

#[cfg(feature = "multiplayer")]
#[test]
fn network_arrays_accept_only_supported_historical_widths() {
    #[derive(serde::Deserialize)]
    struct Resources(#[serde(deserialize_with = "read_resources")] [u32; RESOURCE_COUNT]);
    #[derive(serde::Deserialize)]
    struct Gases(#[serde(deserialize_with = "read_gas_counts")] [u32; SPECIES]);
    for len in 0..=RESOURCE_COUNT + 1 {
        let encoded = serde_json::to_string(&vec![7_u32; len]).unwrap();
        let result = serde_json::from_str::<Resources>(&encoded);
        if [22, 23, 24, 25].contains(&len) {
            let restored = result.unwrap().0;
            assert_eq!(&restored[..len], vec![7; len]);
            assert!(restored[len..].iter().all(|count| *count == 0));
        } else {
            assert!(result.is_err(), "resource width {len}");
        }
    }
    for len in 0..=SPECIES + 1 {
        let encoded = serde_json::to_string(&vec![9_u32; len]).unwrap();
        let result = serde_json::from_str::<Gases>(&encoded);
        if [5, SPECIES].contains(&len) {
            let restored = result.unwrap().0;
            assert_eq!(&restored[..len], vec![9; len]);
            assert!(restored[len..].iter().all(|count| *count == 0));
        } else {
            assert!(result.is_err(), "gas width {len}");
        }
    }
}
