# Inscriptions and tactics

Combat now has shorter, animation-synchronised contact times and a 140 ms input buffer near the end of recovery. Normal strikes land after 75 ms and recover in 300 ms; finishers land after 140 ms and recover in 460 ms. Aim, reach, collision and the existing three-strike sequence still matter. A committed strike continues to reserve the attack button, preventing accidental excavation during recovery.

## Guardians

- Melee creatures flank before closing. Ranged guardians retreat at short range, circle at medium range and approach from farther away. Nearby creatures separate instead of stacking on one point.
- Allies at the same site share sightings within 12 m, then investigate the reported location using the existing last-seen memory. Grounded enemies check footing before walking or sidestepping over ledges. Local obstacle probing and hopping remain in place; this is local steering, not a global navigation mesh.
- Sentinels alternate faster aimed bolts and three-bolt spreads. Their aim includes a modest movement prediction, locked when the wind-up begins. Wisps fire paired bolts. Ranged enemies sidestep nearby incoming player projectiles.
- Spitters alternate spreads with marked spore zones. Wardens use five-bolt fans at range and marked area attacks at close range. Orange ground rings show committed targets before damage; jump or move out of the area. Spore zones persist and pulse. Terrain blocks their damage, and Mirror ward can soften it.
- Brute charges retain their committed direction. Staggering a guardian cancels its wind-up; released projectiles and fields persist independently.
- Four deterministic escort compositions vary uncleared encounters without changing encounter counts, persistent defeat masks or the gentle starter encounter.

## Discoveries

The first clear of each ruin or fungal sanctuary type grants a permanent inscription. Its ten-second reward card explains the new power. Open **Inventory → Gear → Inscriptions** to inspect collected histories and hints for missing discoveries. The benefits apply automatically, including to spells unlocked later through wand crafting.

| Site | Inscription | Permanent effect |
| --- | --- | --- |
| Workshop | Artificer's mark | Spark costs 2 less mana. |
| Watchtower | Storm lens | One additional Spark jump, with 5.5 m reach per jump. |
| Waterworks | Tidal seal | Reflecting a projectile restores 6 mana. |
| Foundry | Cinder script | Burst's splash shatters frozen guardians for 12 extra damage. |
| Conservatory | Verdant covenant | Spore fields last 7 seconds and heal 4 health per pulse. |
| Archive | Archivist's cipher | All spell costs fall by 20%, stacking with the Spark discount. |
| Luminous grove | Mycelial memory | A successful Mend also restores 12 hunger. |
| Ember hollow | Ember heart | Guardian defeats restore 4 health while you are alive. |
| Crystal cathedral | Prism facet | Frost lance's slow lasts 6 seconds. |
| Drowned grotto | Undertow knot | Mirror ward lasts 9 seconds; still three charges. |

Every inscription also reveals a short piece of the lost civilisation's history. Repeated site types still provide their normal supplies and mastery progress, while new types provide new powers. Gear tooltips show discounted mana costs to one decimal place.

Discoveries are reconstructed once from saved clear records and the world's own generator version and seed. Existing cleared sites grant their inscriptions on the first gameplay update. This increment introduces no new save layout, inventory IDs or duplicate material grants. Temporary hazards and queued strikes clear on death and are not saved.

## Verification

Local `cargo build --offline --target-dir target` passed, including the existing build-time WGSL validation. Log: `target/inscriptions-build.log`. The combined build required three small inventory integration repairs: the current max-health call signature, an exhaustive Lumen armour description, and equipment-slot decoding without conflicting borrows.

No tests or gameplay session were run. Dodge windows, steering, visual readability and balance still need in-game acceptance. Existing audio and attack models are reused; no external assets or dependencies were added.
