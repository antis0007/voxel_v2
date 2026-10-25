# Prompt: player-text audit

Paste into a fresh session. The rules are `crates/app/src/egui_ui/UI_RULES.md`
and the Writing section of `CLAUDE.md`. Target voice: vanilla Minecraft with
Jade, JEI/EMI and Xaero's. The screen shows state; it never narrates, coaches
or explains itself.

```text
Audit every player-visible string in this repository against
crates/app/src/egui_ui/UI_RULES.md. Player-visible means: egui widgets in
crates/app/src/egui_ui, chat lines passed to say(...) under crates/app/src/world,
machine status strings, item, block and machine names, recipe and mode labels,
tooltips, loading and menu text, and the RON content in assets/ that reaches
the screen. Log lines (println!, eprintln!, log::) and review-capture output
are not player text.

Phase 1, report only. Do not edit anything.
1. Run `just lint-ui` for the mechanical findings, then read each UI file
   yourself; the lint misses prose that uses no banned word.
2. For every finding give file:line, the exact string, the rule it breaks
   and one of:
   - rewrite: the replacement string, terse, Title Case state or noun,
     numbers with units, keys as [Key].
   - delete: the information is already visible as a bar, slot, number or
     recipe, or the line narrates.
   - needs-design: the screen only works because a sentence explains it.
     Name the widget that should replace the sentence (a slot, a bar with a
     marker, a recipe view, a [Shift] detail line, a look-at line). Do not
     just shorten the prose.
3. Flag invented lore: capitalized abstract concepts, new proper nouns,
   factions, or lines that point at no mechanic the player can use or see.
   Mark these delete unless the owner already approved the name.
4. Group the report by screen (HUD, look-at, inventory, machine panels,
   recipe book, map, menus, chat). Put needs-design items first.
5. Check `git status` and list which files other sessions are editing.
   Those findings wait for their owner.

Phase 2, after the owner approves the report.
1. Apply rewrite and delete items in files no other session is editing.
   Touch strings only, never logic. Keep machine and item names identical
   everywhere they appear (UI_RULES 12).
2. Update tests that pin the old strings.
3. Add each cleaned file to CHECKED in crates/app/src/egui_ui/text_rules.rs
   so `just test` keeps it clean.
4. Build needs-design items one screen at a time, each with a before and
   after capture from the same pose.
5. Report: strings changed per file, files added to CHECKED, needs-design
   items done and left, and `just lint-ui` totals before and after.
```
