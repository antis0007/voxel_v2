//! Survival worlds lock every creative entry point; only the hand stays.
use super::{CreativeState, ToolKind};

impl CreativeState {
    /// Follows the open world's game mode. Locking drops a held creative tool,
    /// its queued edits and any creative window already open.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        if !locked {
            return;
        }
        if self.tool != ToolKind::Hand {
            self.tool = ToolKind::Hand;
            self.cancel_edits();
        }
        self.show_panel = false;
        self.show_quick_select = false;
        self.show_material_editor = false;
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Selects `tool` unless it is a creative tool in a locked world.
    pub fn choose_tool(&mut self, tool: ToolKind) -> bool {
        if self.locked && tool != ToolKind::Hand {
            return false;
        }
        self.tool = tool;
        true
    }

    /// Opens or closes the creative panel; a locked world keeps it shut.
    pub fn toggle_creative_panel(&mut self) {
        let open = !self.show_panel && !self.locked;
        self.close_panels();
        self.show_panel = open;
    }

    /// Opens or closes the tool wheel; a locked world keeps it shut.
    pub fn toggle_quick_select(&mut self) {
        let open = !self.show_quick_select && !self.locked;
        self.close_panels();
        self.show_quick_select = open;
    }

    /// The item catalogue page and its grants.
    pub fn catalogue_open(&self) -> bool {
        !self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survival_gating_blocks_each_creative_entry_point() {
        let mut c = CreativeState::default();
        c.set_locked(true);
        for tool in ToolKind::ALL {
            assert_eq!(c.choose_tool(tool), tool == ToolKind::Hand, "{tool:?}");
            assert_eq!(c.tool, ToolKind::Hand);
        }
        c.toggle_creative_panel();
        assert!(!c.show_panel);
        c.toggle_quick_select();
        assert!(!c.show_quick_select);
        assert!(!c.catalogue_open());
    }

    #[test]
    fn locking_drops_a_held_creative_tool_and_open_windows() {
        let mut c = CreativeState::default();
        assert!(c.choose_tool(ToolKind::ExplosiveWand));
        c.toggle_creative_panel();
        assert!(c.show_panel);
        c.set_locked(true);
        assert_eq!(c.tool, ToolKind::Hand);
        assert!(!c.show_panel && !c.show_quick_select && !c.show_material_editor);
    }

    #[test]
    fn creative_worlds_keep_every_tool() {
        let mut c = CreativeState::default();
        c.set_locked(false);
        for tool in ToolKind::ALL {
            assert!(c.choose_tool(tool));
            assert_eq!(c.tool, tool);
        }
        c.toggle_quick_select();
        assert!(c.show_quick_select);
        c.toggle_creative_panel();
        assert!(c.show_panel && !c.show_quick_select);
        assert!(c.catalogue_open());
    }
}
