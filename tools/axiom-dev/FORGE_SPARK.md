# Forge work card · Spark projectile speed

The first Forge proof takes the Spark row (spell ID 0) from the existing RON table through a typed edit and the actual native wand evaluator. The attached *AXIOM Forge — Dot Implementation Plan*, section 24, defines the proof. Attachment SHA-256 is `e6e68f3d2d957c91952a7861e75bb33ec2d6d2d5cb41234e62a7cde3c2338344`. The starting checkout was `51a0e11a` with local changes.

| Boundary | Owner |
| --- | --- |
| Canonical source | `assets/spells.ron`, read by `crates/app/src/world/adventure/spell_defs.rs` |
| Typed edit | `forge-spark.mjs` and the Spells inspector in `app.mjs` |
| Revision and undo | Existing Axiom Dev draft history and revision-checked save in `workspace.mjs` |
| Disposable launch | `server.mjs` and `game.mjs` |
| Solo commands | `crates/app/src/dev_panel.rs` and the existing solo-world gate in `main.rs` |
| Cast and emission | `crates/app/src/world/adventure/wands.rs` |

Only Spark's base projectile speed is editable through this adapter, with a fixture range of 1–120 m/s. It patches the existing RON token. The current source value at implementation was 26 m/s. The fixture launches a new process and passes the chosen speed to a single admitted Spark cast. Normal input continues to use the embedded definition. The same cast evaluator checks wand layout, cooldown, mana, projectile admission and antimagic, and emits the projectile into the real world. Decision 0039 retains host authority for projectiles. The bridge accepts fixture casts only in its existing solo-world scope.

## Review procedure

1. In Axiom Dev, open **Playtest → Build game**. In **Workspace → Spells**, select **Launch saved speed**.
2. Create a solo world in the fresh fixture folder. Record its seed, world type, voxel scale, settings and firing position. In Playtest, click **Add fixture wand**, select the reported hotbar slot in the game, and aim into open space. Click **Cast Spark**, then resume the game if it is paused.
3. Record the projectile and compare the **emitted** entries in `forge-spark.jsonl` under the Playtest save folder. The trace also records the requested base speed, source revision, executable timestamp and mana debit.
4. Close the game. Stage a different value, such as 38 m/s. Use **Launch draft speed** and repeat with the same conditions. For a plain wand, the two launch speeds should differ by 12 m/s and the paid mana should match. Expected values are acceptance criteria.
5. Enter 0 and attempt to stage it. The previous draft should remain. To inspect the stale guard, use a second editor session to save an approved change while the first session holds the earlier revision. The first session should refuse preview or save until it reloads.
6. Use **Review & save** to inspect the exact RON diff before saving an approved value. Reload the file to verify persistence. To revert an unsaved value, use **Restore saved speed** or Undo. To roll back a saved value, restore the prior revision from **File info → Backups**, or stage the previous speed and review the save. Rebuild for the normal game to pick up a saved RON change.

This card covers the first Spark workflow. Typed graph topology, general spell authoring, hot reload, AI proposals and the broader Forge acceptance register are later milestones.
