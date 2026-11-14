use super::*;
use super::review_fixture as fixture;
use crate::{player::Player, simulation::{STEP_SECONDS, residency::Residency},
    world::{World, factory, inventory::Item}};
use engine_core::{Budget, VoxelScalePreset};
use engine_voxel::WorldType;
use glam::IVec3;
use std::collections::HashSet;

const POS: IVec3 = IVec3::new(-16, 64, -16);

fn coil() -> Node {
    let mut n = Node::new(Kind::Assembler);
    n.target = all_recipes().iter().position(|r| r.name == "Hand-wound induction coil").unwrap() as u32 + 1;
    n.mode = SINGLE_BATCH;
    n.energy = 64;
    n.input[6] = 256;
    n.input[4] = 128;
    n
}
fn roundtrip(f: &Factory) -> Factory {
    let mut bytes = Vec::new();
    f.encode(&mut bytes);
    let mut words = bytes.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    Factory::decode(&mut || words.next().ok_or_else(|| anyhow::anyhow!("eof")), 1, factory::FACTORY_FORMAT).unwrap()
}
fn run(w: &mut World, p: &Player, ticks: usize) {
    let (mut sources, mut cursor, mut blocks, mut chunks) = (Residency::default(), 0, HashSet::new(), HashSet::new());
    let size = factory::step(w);
    for _ in 0..ticks {
        w.factory.tick(STEP_SECONDS, size);
        factory::tick_world(w, p, STEP_SECONDS, &mut sources, &mut cursor,
            Budget::UNBOUNDED, &mut blocks, &mut chunks);
    }
}

#[test]
fn assembler_batch_mode_switches_preserve_paid_progress() {
    let mut w = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    w.factory.nodes.insert(POS, coil());
    for _ in 0..3 { w.factory.run_assemblers(); }
    let before = w.factory.nodes[&POS].clone();
    for mode in [SINGLE_BATCH, SINGLE_BATCH, 0, SINGLE_BATCH, 2] {
        let previous_mode = w.factory.nodes[&POS].mode;
        factory::action(&mut w, POS.as_vec3(), factory::Action::SetRecipe(POS, mode));
        let n = &w.factory.nodes[&POS];
        assert_eq!((n.input, n.output, n.energy, n.progress), (before.input, before.output, before.energy, before.progress));
        assert_eq!(n.mode, if mode < 2 { mode } else { previous_mode });
    }
    w.factory = roundtrip(&w.factory);
    assert_eq!(w.factory.nodes[&POS].progress, 3);
    for _ in 3..CRAFT_TICKS { w.factory.run_assemblers(); }
    let n = &w.factory.nodes[&POS];
    assert!(!n.enabled);
    assert_eq!((n.output[16], n.input[6], n.input[4], n.energy), (64, 128, 64, 24));
    w.factory = roundtrip(&w.factory);
    for _ in 0..CRAFT_TICKS * 2 { w.factory.run_assemblers(); }
    assert_eq!(w.factory.nodes[&POS].output[16], 64);
    factory::action(&mut w, POS.as_vec3(), factory::Action::Toggle(POS));
    w.factory.nodes.get_mut(&POS).unwrap().energy = 64;
    for _ in 0..CRAFT_TICKS * 2 { w.factory.run_assemblers(); }
    assert_eq!(w.factory.nodes[&POS].output[16], 128);
    assert!(!w.factory.nodes[&POS].enabled);
}

#[test]
fn assembler_batch_waits_for_power_and_output_room() {
    let mut f = Factory::default();
    let mut n = coil(); n.energy = 0;
    f.nodes.insert(POS, n);
    f.run_assemblers();
    assert!(f.nodes[&POS].enabled);
    assert_eq!(f.nodes[&POS].progress, 0);
    f.nodes.get_mut(&POS).unwrap().energy = 64;
    for _ in 0..3 { f.run_assemblers(); }
    f.nodes.get_mut(&POS).unwrap().output[16] = CAPACITY;
    for _ in 0..CRAFT_TICKS { f.run_assemblers(); }
    assert!(f.nodes[&POS].enabled);
    assert_eq!((f.nodes[&POS].progress, f.nodes[&POS].energy), (3, 49));
    f.nodes.get_mut(&POS).unwrap().output[16] -= 64;
    for _ in 3..CRAFT_TICKS { f.run_assemblers(); }
    assert!(!f.nodes[&POS].enabled);
    assert_eq!((f.nodes[&POS].output[16], f.nodes[&POS].input[6]), (CAPACITY, 128));
}

#[test]
fn assembler_batch_stops_chest_intake_but_delivers_each_requested_craft() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let mut w = World::with_seed(WorldType::Superflat, scale, 617);
        let mut p = Player::new();
        fixture::install(&mut w, &mut p, 3).unwrap();
        let at = fixture::machine(&w);
        factory::action(&mut w, p.pos, factory::Action::SetRecipe(at, SINGLE_BATCH));
        run(&mut w, &p, 960);
        let first = fixture::Ledger::capture(&w).unwrap();
        assert!(first.conserved(3));
        assert_eq!((first.destination_coils, first.source_copper, first.source_fiber), (1, 4, 2));
        assert_eq!((first.input_copper, first.input_fiber, first.output_units), (0, 0, 0));
        assert!(!first.enabled);
        run(&mut w, &p, 480);
        assert_eq!(fixture::Ledger::capture(&w), Some(first));
        factory::action(&mut w, p.pos, factory::Action::Toggle(at));
        run(&mut w, &p, 960);
        let second = fixture::Ledger::capture(&w).unwrap();
        assert!(second.conserved(3));
        assert_eq!((second.destination_coils, second.source_copper, second.source_fiber), (2, 2, 1));
        assert!(!second.enabled);
    }
}

fn output_line(kind: Kind) -> Factory {
    let mut f = Factory::default();
    let mut n = if kind == Kind::Assembler { coil() } else { Node::new(kind) };
    n.enabled = false; n.input.fill(0); n.output[16] = 64;
    f.nodes.insert(POS, n);
    f.nodes.insert(POS + IVec3::X, Node::new(Kind::Pipe));
    f.nodes.insert(POS + IVec3::X * 2, Node::new(Kind::Storage));
    f
}

#[test]
fn assembler_batch_paused_output_drains_fractional_units_through_transfer_pipes() {
    let mut f = output_line(Kind::Assembler);
    let dest = POS + IVec3::X * 2;
    f.nodes.get_mut(&dest).unwrap().input[16] = CAPACITY - 32;
    for _ in 0..40 { f.tick(0.25, 1); }
    assert_eq!(f.nodes[&dest].input[16], CAPACITY);
    assert_eq!(f.nodes[&POS].output[16], 32);
    assert!(!f.nodes[&POS].enabled);
    // Collection frees room without restarting the finished assembler.
    f.nodes.get_mut(&dest).unwrap().input[16] = 0;
    for _ in 0..40 { f.tick(0.25, 1); }
    assert_eq!(f.nodes[&dest].input[16], 32);
    assert_eq!(f.nodes[&POS].output[16], 0);
    let mut other = output_line(Kind::Fabricator);
    for _ in 0..40 { other.tick(0.25, 1); }
    assert_eq!(other.nodes[&POS].output[16], 64, "other paused machines retain their existing behavior");
}

#[test]
fn assembler_batch_nonresource_output_preserves_identity_and_legacy_mode() {
    let index = all_recipes().iter().position(|r| r.output.item == Item::Block(Kind::Generator.block())).unwrap();
    let mut n = Node::new(Kind::Assembler);
    n.target = index as u32 + 1; n.mode = SINGLE_BATCH; n.energy = 64;
    n.input[6] = 256; n.input[8] = 256;
    let mut f = Factory::default(); f.nodes.insert(POS, n);
    for _ in 0..CRAFT_TICKS * 2 { f.run_assemblers(); }
    f = roundtrip(&f);
    assert!(!f.nodes[&POS].enabled);
    assert_eq!(f.nodes[&POS].stock, 1);
    assert_eq!(stock_item(&f.nodes[&POS]), Some(Item::Block(Kind::Generator.block())));
    f.nodes.get_mut(&POS).unwrap().mode = 0;
    f.nodes.get_mut(&POS).unwrap().enabled = true;
    f = roundtrip(&f);
    assert_eq!(f.nodes[&POS].mode, 0);
    assert!(f.nodes[&POS].enabled);
}
