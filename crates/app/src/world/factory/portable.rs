//! Power assemblies carry their own charge/configuration through inventory,
//! drops, death, sorting and saves. They never substitute for ordinary recipes.
use super::{Kind, Node, induction, power};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PowerKit {
    pub kind: Kind,
    pub energy: u32,
    pub upgrade: u8,
    pub mode: u8,
    pub limit: u8,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::{World, WorldType, inventory::{Item, ItemStack, SurvivalInventory}}};
    use glam::IVec3;

    #[test]
    fn portable_codec_rejects_invalid_state_and_never_stacks_charge() {
        let kit = PowerKit { kind: Kind::Battery, energy: 23_456, upgrade: 1, mode: 2, limit: 12 };
        let item = Item::PowerKit(kit);
        let (tag, payload) = item.encode_u32();
        assert_eq!(Item::decode_u32(tag, payload).unwrap(), item);
        assert!(Item::decode_u32(tag, power::battery_capacity(1) + 1).is_err());
        assert!(PowerKit::decode(tag | (1 << 31), payload).is_err());
        let mut inv = SurvivalInventory::empty();
        assert_eq!(inv.try_insert(ItemStack { item, count: 2 }), (2, 0));
        assert_eq!(inv.hotbar.iter().flatten().filter(|s|s.item == item).count(), 2);
        assert!(inv.hotbar.iter().flatten().all(|s| s.count == 1));
    }

    #[test]
    fn charged_pickup_and_replace_preserve_parts_at_every_scale() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let mut w = World::new(WorldType::Superflat, scale);
            let size = super::super::step(&w);
            let p = IVec3::new(4, 40, 4) * size;
            let n = super::super::fixture_place(&mut w, p, Kind::Battery).unwrap();
            n.upgrade = 2; n.energy = 123_456; n.mode = 1; n.target = 17;
            let expected = PowerKit::from_node(n).unwrap();
            let mut blocks = Default::default();
            let mut chunks = Default::default();
            assert!(super::super::dismantle(&mut w, p, &mut blocks, &mut chunks));
            assert!(!w.factory.nodes.contains_key(&p));
            let item = Item::PowerKit(expected);
            assert_eq!(w.inventory.count_item(item), 1);
            w.sync_from_inventory(); w.reconcile_legacy_to_inventory();
            assert_eq!(w.inventory.count_item(Item::Block(Kind::Battery.block())), 0);
            let mut player = crate::player::Player::new();
            player.pos = p.as_vec3() + glam::Vec3::splat(12.0 * size as f32);
            assert!(super::super::place(&mut w, p, Kind::Battery.block(), &player, Some(item), &mut blocks, &mut chunks));
            assert_eq!(PowerKit::from_node(&w.factory.nodes[&p]), Some(expected));
            assert_eq!(w.inventory.count_item(item), 0);
        }
    }

    #[test]
    fn full_pack_leaves_charged_machine_in_world() {
        let mut w = World::new(WorldType::Superflat, VoxelScalePreset::OneMeter);
        for s in w.inventory.hotbar.iter_mut().chain(w.inventory.pack.iter_mut()) {
            *s = Some(ItemStack::new(Item::Block(crate::world::BlockId::Stone), 64));
        }
        let p = IVec3::new(4, 40, 4);
        super::super::fixture_place(&mut w, p, Kind::Battery).unwrap().energy = 1234;
        super::super::dismantle(&mut w, p, &mut Default::default(), &mut Default::default());
        assert_eq!(w.factory.nodes[&p].energy, 1234);
        assert_eq!(w.get(p), Kind::Battery.block());
    }
}

impl PowerKit {
    pub fn supports(kind: Kind) -> bool {
        matches!(kind, Kind::Battery | Kind::Transformer | Kind::InductionPort | Kind::InductionCell)
    }

    pub(super) fn from_node(n: &Node) -> Option<Self> {
        Self::supports(n.kind).then_some(Self {
            kind: n.kind, energy: n.energy, upgrade: n.upgrade as u8,
            mode: n.mode as u8, limit: n.target as u8,
        })
    }

    pub(super) fn node(self) -> Node {
        let mut n = Node::new(self.kind);
        n.energy = self.energy;
        n.upgrade = self.upgrade as u32;
        n.mode = self.mode as u32;
        n.target = self.limit as u32;
        n
    }

    // Low byte is the item tag. Five kind bits, two upgrade bits, two mode
    // bits and seven rate bits follow. The second word is exact integer J/250.
    pub fn encode(self) -> (u32, u32) {
        (7 | (self.kind as u32) << 8 | (self.upgrade as u32) << 13
            | (self.mode as u32) << 15 | (self.limit as u32) << 17, self.energy)
    }

    pub fn decode(header: u32, energy: u32) -> anyhow::Result<Self> {
        use anyhow::{Context, ensure};
        ensure!(header & 0xff == 7 && header >> 24 == 0, "Invalid power kit header");
        let kind = *Kind::ALL.get(((header >> 8) & 31) as usize).context("Unknown power kit")?;
        ensure!(Self::supports(kind), "Assembly cannot carry power");
        let kit = Self { kind, energy, upgrade: ((header >> 13) & 3) as u8,
            mode: ((header >> 15) & 3) as u8, limit: ((header >> 17) & 127) as u8 };
        ensure!(kit.upgrade <= 2 && (kind.upgradable() || kit.upgrade == 0), "Invalid portable upgrade");
        ensure!((kit.mode as usize) < kind.modes().len().max(1), "Invalid portable mode");
        ensure!(kit.limit == 0 || (kind == Kind::Battery && kit.limit as u32 <= power::battery_rate(kit.upgrade as u32)), "Invalid portable I/O limit");
        let cap = match kind {
            Kind::Battery => power::battery_capacity(kit.upgrade as u32),
            Kind::Transformer => power::transformer_rate(kit.upgrade as u32) * 2,
            Kind::InductionCell => induction::CELL_CAPACITY,
            _ => induction::MAX_CAPACITY, // legacy pools may still live on ports
        };
        ensure!(energy <= cap, "Portable charge exceeds capacity");
        Ok(kit)
    }
}
