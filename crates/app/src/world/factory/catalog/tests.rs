use super::*;

/// Serialising the loaded table reproduces the file (minus its comment
/// header), so the file is the single source of truth.
fn assert_round_trip<T: serde::Serialize>(rows: &T, file: &str) {
    let text = ron::ser::to_string_pretty(rows, ron::ser::PrettyConfig::new())
        .unwrap()
        .replace('\r', "");
    let file = file.replace('\r', "");
    let body = match file.find("\n[\n") {
        Some(i) => &file[i + 1..],
        None => file.as_str(),
    };
    let (a, b) = (text.trim_end(), body.trim_end());
    if a != b {
        let i = a
            .bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        let lo = i.saturating_sub(60);
        panic!(
            "table file differs from the loaded rows at byte {i}; rerun the ignored dump test to rewrite it.\n table: {:?}\n  file: {:?}",
            &a[lo..(i + 60).min(a.len())],
            &b[lo..(i + 60).min(b.len())]
        );
    }
}

#[test]
fn machines_ron_round_trips() {
    assert_round_trip(&*MACHINES, MACHINES_RON);
}

/// Rewrites assets/machines.ron from the loaded table (keeps the header).
#[test]
#[ignore = "writes assets/machines.ron"]
fn dump_machines_ron() {
    let header = MACHINES_RON.split("[").next().unwrap_or("");
    let text = ron::ser::to_string_pretty(&*MACHINES, ron::ser::PrettyConfig::new()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/machines.ron");
    std::fs::write(path, format!("{header}{text}")).unwrap();
}

// Deliberately independent of the declaration: these existing identities are a
// save/network compatibility contract, not a second production catalogue.
const EXISTING_IDENTITIES: [(&str, u32); 40] = [
    ("Generator", 0),
    ("Crusher", 1),
    ("Furnace", 2),
    ("Feeder", 3),
    ("Storage", 4),
    ("Pipe", 5),
    ("Cable", 6),
    ("Bioreactor", 7),
    ("AlloyKiln", 8),
    ("Fabricator", 9),
    ("Cultivator", 10),
    ("MiningDrill", 11),
    ("Pump", 12),
    ("FluidPipe", 13),
    ("Tank", 14),
    ("Washer", 15),
    ("Battery", 16),
    ("CokeOven", 17),
    ("BlastFurnace", 18),
    ("CokeBrick", 19),
    ("BlastBrick", 20),
    ("InductionCasing", 21),
    ("InductionPort", 22),
    ("InductionCell", 23),
    ("InductionProvider", 24),
    ("Transformer", 25),
    ("Assembler", 26),
    ("SearedBrick", 27),
    ("SmelteryController", 28),
    ("SmelteryDrain", 29),
    ("SmelteryFaucet", 30),
    ("CastingTable", 31),
    ("CastingBasin", 32),
    ("ExhaustDuct", 33),
    ("Chimney", 34),
    ("DustFilter", 35),
    ("WetScrubber", 36),
    ("GasTank", 37),
    ("GasGenerator", 38),
    ("Quarry", 39),
];

#[test]
fn saved_ids_and_serialized_names_remain_stable() {
    assert_eq!(std::mem::size_of::<Kind>(), 4);
    assert_eq!(Kind::COUNT, 40);
    assert_eq!(Kind::LEGACY_COUNT, 16);
    for (name, id) in EXISTING_IDENTITIES {
        let kind: Kind = ron::from_str(name).unwrap();
        assert_eq!(kind as u32, id, "{name}");
        assert_eq!(Kind::ALL[id as usize], kind, "{name}");
        assert_eq!(ron::to_string(&kind).unwrap(), name);
        let json = format!("\"{name}\"");
        assert_eq!(serde_json::to_string(&kind).unwrap(), json);
        assert_eq!(serde_json::from_str::<Kind>(&json).unwrap(), kind);
    }
}

#[test]
fn authored_machine_rows_have_unique_reversible_block_identity() {
    let mut blocks = std::collections::HashSet::new();
    for kind in Kind::ALL {
        assert_eq!(MACHINES[kind as usize].kind, kind);
        assert!(
            blocks.insert(kind.block()),
            "duplicate machine block for {kind:?}"
        );
        assert_eq!(Kind::from_block(kind.block()), Some(kind));
    }
}

#[test]
fn every_historical_kind_prefix_preserves_kits_and_node_identity() {
    use super::super::{FACTORY_FORMAT, Factory, Node};
    use glam::IVec3;

    for format in 8..=FACTORY_FORMAT {
        let count = Kind::count_in_format(format);
        let mut factory = Factory::default();
        for kind in Kind::ALL.into_iter().take(count) {
            let id = kind as usize;
            factory.kits[id] = id as u32 + 1;
            factory
                .nodes
                .insert(IVec3::new(id as i32 * 2, 64, 0), Node::new(kind));
        }
        let mut bytes = Vec::new();
        factory.encode_format(&mut bytes, format);
        let words: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect();
        assert_eq!(&words[..count], &factory.kits[..count], "format {format}");
        let mut cursor = words.into_iter();
        let restored = Factory::decode(
            &mut || {
                cursor
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("truncated fixture"))
            },
            1,
            format,
        )
        .unwrap_or_else(|error| panic!("format {format}: {error}"));
        assert!(cursor.next().is_none(), "format {format}");
        assert_eq!(restored.kits, factory.kits, "format {format}");
        assert_eq!(restored.nodes.len(), count, "format {format}");
        for (position, node) in &factory.nodes {
            assert_eq!(restored.nodes[position].kind, node.kind, "format {format}");
        }
        let mut reencoded = Vec::new();
        restored.encode_format(&mut reencoded, format);
        assert_eq!(reencoded, bytes, "format {format}");
    }
}
