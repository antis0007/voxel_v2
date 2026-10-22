# Native encounter gameplay review — 2026-10-21

Built and ran `target/release-fast/minecraft_style_voxel_prototype.exe` in disposable native game worlds. Inspected native windows and saved frames. Scripted scenarios feed ordinary movement inputs into `update_player` and use the production wand deck, mana, collision, enemy AI, damage, death, rewards and save service. Manual checks used normal R key taps. These were arena fixtures, not a campaign playthrough or purely simulated/headless assertions.

Final executable SHA256: `BF18723A8DCAEDA72BD1FD35E616C60F02AEA88C1B78B7E3085B4205D08B0D3D`.

## Fixes found through the runs

- Concealed mimics previously announced their species and health in the targeting HUD. Concealed creatures now skip that focus readout. Their physical seam/glint and their vulnerability to attacks remain.
- A quick R press/release could disappear before the next gameplay frame sampled it. `Input::cast_pressed` retains the press until it reaches the existing adventure cast queue. A native manual tap subsequently fired Spark, spent mana, and reduced a hidden mimic from 72 to 50 health. The later manual frost shot missed the moving mimic; disappearance of its focus label was not a kill. The idle player was eventually killed by that mimic.
- Boss aiming previously predicted only 0.25 seconds despite a longer windup and projectile travel. Prediction now includes the windup and bounded travel compensation, with a five-metre displacement cap. Aim remains committed; changing direction can evade it.
- Castellan and Keeper observe an active visible ward when selecting their next attack, favoring warned ground pressure and alternating crossfire over repeatedly feeding the ward the same volley. This does not bypass the ward's existing damage rules or retarget an attack already in progress.
- An opaque ten-orb muzzle effect half a metre in front of the camera repeatedly covered the boss. The wand already had a muzzle flash. Replaced the duplicate effect with four small sparks beside the wand. Additional visual refinements bound friendly projectile size near the camera and reduce opaque explosion cores/debris. Incoming enemy darts retain their full visual size.

The last visual retest shows the boss body and tell through the casting effect:

- [Before muzzle correction](C:/dev/voxel_v2_backup/target/gameplay-review/final-projectile-visibility/bench-001.png)
- [After muzzle correction](C:/dev/voxel_v2_backup/target/gameplay-review/final-clear-muzzle/bench-001.png)

## Observed results

| Scenario | Result | Evidence directory under `target/gameplay-review/` |
| --- | --- | --- |
| Stationary Castellan baseline | Warned attacks dealt damage; idle player died and respawned at about 21 simulation seconds | `castellan-baseline` |
| Moving tier-2 wand duels | Castellan and Keeper passed through both phases and were defeated through ordinary casts; this defensive deck/movement route retained full health | `final-castellan`, `final-keeper` |
| Enraged Keeper, movement without attacks/ward | Patterns 21/22/23 occurred; moving player reached 68.9 minimum health and survived the run | `final-keeper-enraged` |
| Rubble ambush | Remained disguised while distant. Emergence was observed at 5.25 seconds, first player damage at 7.75 seconds. The player later died after stopping nearby | `final-rubble` |
| Obstacle route | A skitterer moved around the wall, reached the player's side and attacked; it did not remain walking into the obstruction | `final-obstacle` |
| Trapper/skitterer/stalker pack, player not casting | Spore fields reduced the skitterer from 35 to 20.2 health and stalker from 58 to 15.6. Trapper remained alive; the pack killed the moving player around 14 seconds | `final-pack` |
| Cache room progression | Ordinary wand casts killed the enemy; record 257, material payouts and new spell bits appeared. Mastery increased, naturally raising/refilling health to 120 and mana to 115 | `final-progression`, `final-disk-progression`, `final-save-recheck` |
| Real save and reload | Version-24 quicksave reloaded with identical clear records, spell bits and material counts. A second clear after reload awarded no resources | `final-save-recheck` |
| Corrected cast visibility | Final executable ran both boss phases while the centre of the casting view stayed readable in inspected frames | `final-clear-muzzle` |

The gameplay suite used build `D155E89F...`; a subsequent visual-only build used `141B5770...`. The final muzzle correction produced `BF18723A...` and was rerun for boss visuals and full on-disk progression. Per-run manifests record full executable hashes. No AI, damage, progression or input rules changed between the gameplay suite and these visual retests.

All completed scripted review commands exited successfully and produced native benchmark completion records. Logs and PNGs remain beside their manifests. FPS samples are diagnostics only: other native rendering reviews were active during parts of this work, so these are not isolated performance measurements.

## Reproduce

```powershell
cargo build --offline --profile release-fast --target-dir target
./tools/Review-Encounters.ps1 -Kind 6 -Mode duel -Seconds 30 -Label my-castellan
./tools/Review-Encounters.ps1 -Kind 7 -Mode dodge -Enraged -Seconds 20 -Label my-enraged-keeper
./tools/Review-Encounters.ps1 -Kind 3 -Mode dodge -Scene ambush -Seconds 20 -Label my-ambush
./tools/Review-Encounters.ps1 -Kind 12 -Mode dodge -Scene pack -Seconds 28 -Label my-pack
./tools/Review-Encounters.ps1 -Kind 10 -Mode progression -Scene cache -Seconds 24 -Label my-progression
```

Labels must be unused to preserve evidence. Scripts restore environment variables and isolate saves beneath each evidence directory. Automated runs do not refill health or mana through review code, force kills or replace enemy action timers; ordinary progression and death still perform their normal refills. `-Mode manual` permits the normal native game controls in the disposable arena. Avoid injecting absolute mouse clicks while the first-person camera owns the cursor; that can produce an automation-induced camera movement.

## Remaining coverage

Generated dungeon layouts, long campaign balance, multiplayer, gamepad play and other machines remain unverified. The dedicated Hart and Tollkeeper fights were not replayed in this pass. Local navigation still has its documented bounded search range. Legacy stranded-clear reconciliation is implemented, but this review's disk checkpoint verifies a newly earned clear rather than an imported historical broken-save fixture. Successful arena reviews do not establish that every generated encounter is fair or that the game has final balance.
