//! Record semantic UI operations, never accept a client inventory as authority.
use crate::world::inventory::{self, SlotAddr, SurvivalInventory};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Serialize, Deserialize)]
pub enum Command {
    Click(SlotAddr), Right(SlotAddr), Shift(SlotAddr), Ctrl(SlotAddr), Middle(SlotAddr),
    Swap(SlotAddr,usize), Gather(SlotAddr), Wheel(SlotAddr,i32), Sort, Stack,
    Distribute(Vec<SlotAddr>), TakeDrop(SlotAddr,bool), Return,
    Craft(bool), Stage(usize),
    Lock(SlotAddr),
    Recover,
    CraftRecipe(usize, u64, bool),
}
#[derive(Default)]
struct Tape { enabled: bool, depth: usize, commands: Vec<Command> }
thread_local! { static TAPE: RefCell<Tape> = RefCell::new(Tape::default()); }
pub fn begin() { TAPE.with(|t| *t.borrow_mut()=Tape { enabled:true,..Default::default() }); }
pub fn finish() -> Vec<Command> { TAPE.with(|t| { let mut t=t.borrow_mut();t.enabled=false;std::mem::take(&mut t.commands) }) }
pub struct Guard(bool);
pub fn record(command: Command) -> Guard {
    Guard(TAPE.with(|t| { let mut t=t.borrow_mut();if !t.enabled {return false;}
        if t.depth==0 && t.commands.len()<64 { t.commands.push(command); } t.depth+=1;true }))
}
impl Drop for Guard { fn drop(&mut self) { if self.0 { TAPE.with(|t|t.borrow_mut().depth-=1); } } }
fn valid(a: SlotAddr) -> bool { match a {
    SlotAddr::Hotbar(i)=>i<inventory::HOTBAR_SLOTS,
    SlotAddr::Pack(i)=>i<inventory::PACK_SLOTS,
    SlotAddr::Equipment(i)=>i<inventory::EQUIP_SLOTS,
    SlotAddr::CraftGrid(i)=>i<inventory::CRAFT_SLOTS,
    SlotAddr::Offhand=>true,
    SlotAddr::Container(i)=>i<crate::world::containers::SLOTS,
} }
impl Command {
    /// Each command reads canonical contents, then commits before the next
    /// player's command. Two open windows never own independent chest contents.
    pub fn apply_world(self, world: &mut crate::world::World, player: &crate::player::Player, bench: bool) -> anyhow::Result<()> {
        if world.inventory.container.is_some() {
            if !crate::world::containers::reachable(world, player) {
                world.inventory.container = None;
            } else {
                crate::world::containers::refresh(world);
            }
        }
        self.apply(&mut world.inventory, bench)?;
        crate::world::containers::commit(world);
        Ok(())
    }

    pub fn apply(self, inv:&mut SurvivalInventory, bench:bool) -> anyhow::Result<()> {
        use anyhow::ensure;
        match self {
            Self::Recover => inv.recover_overflow(),
            Self::Lock(a) => { ensure!(valid(a), "invalid lock slot"); inv.toggle_protected(a); }
            Self::Click(a)|Self::Right(a)|Self::Shift(a)|Self::Ctrl(a)|Self::Middle(a)|Self::Gather(a) => {
                ensure!(valid(a),"invalid inventory slot");
                match self { Self::Click(_)=>inv.click_slot(a),Self::Right(_)=>inv.right_click_slot(a),
                    Self::Shift(_)=>{inv.shift_click_slot(a);},Self::Ctrl(_)=>{inv.ctrl_click_slot(a);},
                    Self::Middle(_)=>inv.middle_click_slot(a),_=>inv.gather_to_cursor(a) }
            }
            Self::Swap(a,i)=>{ensure!(valid(a)&&i<inventory::HOTBAR_SLOTS,"invalid hotbar swap");inv.swap_slot_with_hotbar(a,i);}
            Self::Wheel(a,d)=>{ensure!(valid(a)&&d.unsigned_abs()<=64,"invalid inventory wheel");inv.wheel_slot(a,d);}
            Self::Sort=>inv.sort_pack(), Self::Stack=>inv.quick_stack_to_hotbar(),
            Self::Distribute(slots)=>{
                ensure!(slots.len()<=inventory::PACK_SLOTS+inventory::HOTBAR_SLOTS+inventory::CRAFT_SLOTS+inventory::EQUIP_SLOTS,"oversized drag");
                let mut unique=Vec::new();for a in slots {ensure!(valid(a),"invalid drag slot");if !unique.contains(&a){unique.push(a);}}
                inv.distribute_even_across(inv.cursor,&unique);
            }
            Self::TakeDrop(a,full)=>{ensure!(valid(a)&&inv.cursor.is_none(),"invalid drop source");inv.cursor=inv.take_for_drop(a,full);}
            Self::Return=>inv.return_transient_to_pack(),
            Self::Craft(bulk)=>{inv.craft_from_grid(bulk,bench);}
            Self::CraftRecipe(i, key, bulk)=>{ensure!(inventory::selected_recipe_key(i) == Some(key), "recipe catalog mismatch");let _=inv.craft_selected_recipe(i,bulk,bench);}
            Self::Stage(i)=>{let recipe=inventory::all_recipes().get(i).ok_or_else(||anyhow::anyhow!("invalid recipe"))?;inv.stage_recipe_into_grid(recipe,bench);}
        }
        Ok(())
    }
}
