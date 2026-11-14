//! Native fixed-step acceptance of the shared recipe/chest contract.
use super::{World, Player, B, Item, ItemStack, IVec3, Vec3, f, world};

#[derive(Default)]
pub struct Review {
    cases: Vec<(IVec3, usize, u32)>,
    pub done: bool,
    capture: bool,
}
impl Review {
    pub fn install(&mut self, w: &mut World, p: &mut Player) {
        world::storage::set_autosave_enabled(false);
        let s = f::step(w);
        let origin = f::anchor(p.pos.floor().as_ivec3(), s) + IVec3::Y * 3 * s;
        let cases = [(f::Kind::AlloyKiln, 1), (f::Kind::Fabricator, 0),
            (f::Kind::Cultivator, 1), (f::Kind::Crusher, f::recipes::QUARTZ_RECIPE),
            (f::Kind::Washer, 0), (f::Kind::Cultivator, 3)];
        for (i, (kind, mode)) in cases.into_iter().enumerate() {
            let at = origin + IVec3::X * i as i32 * 4 * s;
            for x in -s..2*s { for z in -2*s..s { for y in 0..s {
                w.set(at + IVec3::new(x,y,z), B::Air);
            }}}
            let node = f::fixture_place(w, at, kind).unwrap();
            node.mode = mode as u32;
            node.water = 8192;
            let process = f::recipes::process(kind, mode).unwrap_or(f::recipes::Process {
                inputs: [(0,0);3], block_input: None, output: (13,64), byproduct: None, ticks:16, power:8,
            });
            f::fixture_place(w, at - IVec3::Z * s, f::Kind::Cable).unwrap();
            let battery = f::fixture_place(w, at - IVec3::Z * 2*s, f::Kind::Battery).unwrap();
            battery.upgrade = 2;
            battery.energy = f::power::battery_capacity(2);
            let input = at - IVec3::X;
            let output = at + IVec3::X * s;
            for pos in [input,output] {
                w.set(pos,B::Chest);
                w.block_data.set_chest(pos,vec![None;B::Chest.chest_slots()]);
            }
            let mut stock = w.block_data.chest(input);
            let mut slot = 0;
            for (resource,units) in process.inputs.into_iter().filter(|(_,n)| *n>0) {
                assert_eq!(units % 64,0);
                stock[slot]=Some(ItemStack::new(Item::Resource(resource as u8), units/64));
                slot+=1;
            }
            if let Some((material,count))=process.block_cost(s) {
                stock[slot]=Some(ItemStack::new(material.item(),count));
            }
            if (kind,mode)==(f::Kind::Cultivator,3) {
                stock[slot]=Some(ItemStack::new(f::blocks::Material::Grass.item(),2));
            }
            w.block_data.set_chest(input,stock);
            let operator=at.as_vec3()+Vec3::Y;
            for (face,role) in [(0,2),(1,1),(2,3),(3,3),(4,3),(5,3)] {
                f::action(w,operator,f::Action::SetFace(at,face,role));
            }
            self.cases.push((at,process.output.0,process.output.1));
        }
        p.pos=origin.as_vec3()+Vec3::new(6.0,3.0,8.0)*s as f32;
        p.precise_pos=p.pos.as_dvec3(); p.vel=Vec3::ZERO; p.flying=true;
        let direction=origin.as_vec3()+Vec3::X*6.0*s as f32-p.eye(w.voxel_scale);
        p.yaw=direction.x.atan2(-direction.z); p.pitch=(direction.y/direction.length()).asin();
        eprintln!("[processors-review] installed kiln, fabricator, cultivator, quartz crusher, washer and grass press; voxel_m={}",w.voxel_scale.meters());
    }
    pub fn frame(&mut self,w:&World) {
        if self.done {return;}
        let mut complete=true;
        for &(at,resource,units) in &self.cases {
            let n=&w.factory.nodes[&at];
            let output=at+IVec3::X*f::step(w);
            let delivered=w.block_data.chest(output).iter().flatten()
                .filter(|s|s.item==Item::Resource(resource as u8)).map(|s|s.count).sum::<u32>()*64;
            assert!(delivered+n.output[resource]<=units,"processor duplicated output");
            complete &= delivered==units && n.input.iter().all(|&n|n==0)
                && n.blocks.input.iter().all(|&n|n==0)
                && w.block_data.chest(at-IVec3::X).iter().all(Option::is_none);
        }
        if complete {
            self.done=true; self.capture=true;
            eprintln!("[processors-review] PASS six recipes consumed exact chest stock and delivered exact output");
        }
    }
    pub fn capture(&mut self)->Option<std::path::PathBuf> {
        if !std::mem::take(&mut self.capture) {return None;}
        Some(std::path::PathBuf::from(std::env::var_os("VOXEL_CAPTURE_DIR")?).join("processors-complete.png"))
    }
}
