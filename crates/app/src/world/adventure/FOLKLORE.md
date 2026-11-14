# Common mob animation and folklore roster

Kinds are append-only. Existing saves retain their identities; night actors do
not write site-clear records. Stats, spawn weights and drops are in
`assets/bestiary.ron`; rigs and attack choices are in `folklore.rs`.

| Kind | Creature | Attack and response |
|---|---|---|
| 18 | Brambleburst | Six rooted legs and a splitting seed pod; evade its fuse or break sight with cover. |
| 19 | Barrow Archer | Articulated skeleton with a drawn bow; daylight burns it. |
| 20 | Satyr | Horn-lowering preparation, committed short rush, retreat during cooldown. |
| 21 | Pit Imp | Paired slow shots; keeps distance, with folded wings and a casting throat. |
| 22 | Redcap | Short cleaver rush, quick footwork, retreat before another attack. |
| 23 | Barrow Draugr | Slow, heavy axe swing; interrupt the preparation or dodge the committed arc. |
| 24 | Boggart | Alternates claw sweeps and a delayed spore patch. |
| 25 | Corpse Light | Hovering skull and trailing rags; marks a small delayed ground attack. |

The MC-RS directory is the packaged 0.6.18 reference, without Rust source.
Reference principles are clear limb movement, immediate impact feedback and a
readable fuse. No reference textures, models, audio or code enter these rigs.

Common ground gait uses travelled metres and a 60% planted stance. New bipeds
and the archer account for strafing and backpedalling. Weapons reach contact
at damage release, then follow through for 0.42 seconds. Directional impacts
settle over 0.46 seconds; stun interrupts ordinary enemies regardless of their
numeric ID. Boss action clocks remain separate. Bodies fall before removal;
night deaths emit their configured drops without completing a site.

`Review-MobArt.ps1 -Scene folklore -Mode poses` shows the roster. `-Kind 20`
selects one creature. Optional `-PoseTime` freezes walking at 0.5, preparation
at 1.7, follow-through at 2.08, or impact at 2.78. These are authored pose
captures, not combat acceptance. Omit `-PoseTime` to view the moving cycle.

Terrain defaults and generator gates are outside this change. No tests or CI
were added or run. Build and capture receipts belong in `target/folklore-review`.
