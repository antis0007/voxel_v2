# Navigation, elevator and shared machine workflow — 2026-11-11

## Delivered behavior

- An intact nearby radio dish projects live orbits in the world. Its interaction hint uses the player's key binding. Interact opens the system map; the orbital service menu and flight HUD share its target.
- Map selection outside the local system automatically frames the selected body's parent. Target, Local, System, Fit, Parent, search, rotation, pan, zoom, transfer previews and orbit visibility are available. Transfer preview holds an absolute simulation instant, so an open window stays open while inspecting it.
- Escape and the close button return mouse control to flight. The destination survives closing and reopening.
- Meridian's procedural terminal floor is above both terrain and sea level. The editable tower, fittings and boarding datum share that floor. Freight socket, carriage and crown share the same lateral offset. Attached props use the terrain's planet projection instead of a nearby parabolic approximation that made tall parts lean away from the shaft.
- Planet generators that bypass the legacy basin now generate the terminal footprint too. Previously they drew its fittings over unrelated terrain/ruins and refused boarding because the cabin volume was obstructed.
- Factory connection capabilities come from one cached catalogue contract used by network compilation, conduit arms, gas routing and the inspector. Incompatible power/fluid/gas endpoints no longer appear connected simply because they are machines.
- Processing machines share recipe candidates with chest intake. Alloying, fabrication, cultivation, washing and quartz crushing join the existing assembler/furnace workflow. Face roles, chest filters, reservations, capacity, fractional resources and bounded transfer budgets remain in effect. Grass-processing cultivation also exposes its authored block requirement to this path.

## Native evidence

### Actual mouse and keyboard navigation

Disposable prebuilt dish, seed 1337, Planet generator 272. Automatic navigation replay disabled; normal cursor capture and free camera enabled after fixture placement.

1. Pressed G while inspecting the dish: system map opened.
2. Clicked Giant in the body tree: parent system framed automatically.
3. Clicked Set Target: Giant became the shared target.
4. Clicked Show Window: preview read Window Open.
5. Toggled Orbit Lines off/on: orbital rings disappeared/reappeared.
6. Pressed Escape: returned to world with Giant marker and matching dish projection.
7. Applied mouse motion: camera rotated in flight.
8. Reopened with semicolon: Giant and preview retained. Clicked X: returned to world.

This is real OS input, separate from the earlier six-stage scripted replay. It verifies interaction with a prepared console, not the survival construction/unlock route.

Evidence directory: `target/navigation-review/manual-final/` (native captures and logs). Manual review previously released the mouse every frame and fixed the camera every frame; both overrides are now disabled for manual mode.

### Elevator placement

`target/navigation-review/elevator-close/bench-002.png`: close native Planet view of the dry terminal platform and aligned tower fittings. Camera at 1170, 60, -780 m, seed 1337. This changes the procedural datum; it does not migrate player-authored blocks in existing saves.

`target/navigation-review/elevator-272/`: completed native boarding, ascent, station deck/gallery access, descent and return to the berth. The initial current-generator run in `elevator-ride/` exposed the missing terminal footprint described above; its refused boarding is retained as failure evidence.

`target/navigation-review/elevator-287/`: the repaired current generator passed the same full route. Boarding succeeded, the cabin docked at 18,499 m, the passenger entered the gallery, and descent returned to the 139.121 m berth. Captures include `lift-00-berth.png`, `lift-deck-hub.png`, and `lift-released.png`.

### Industry and shared chest processing

- `target/industrial-review/20261111-145124/1/`: PASS, eight casts. Quarry → sluice → gold furnace → mold; excavated terrain and cast reloaded from disk; full inventory preserved the cast; repeated collection produced zero extra output; all supported metals, basin and part mold exercised.
- `target/industrial-review/20261111-145602/{1,0.5}/`: PASS, four independent chest-fed recipes: steel kiln, coil fabricator, lumen cultivator and quartz crusher. Exact ingredients consumed, exact output deposited at both scales.
- `target/industrial-review/20261111-150228/{1,0.5}/`: PASS, expanded to six recipes, adding ore washing and the grass press. Exact chest consumption/output at both scales.
- `target/navigation-review/assembler-final/`: PASS, eight batches, paused mid-batch, checkpoint reloaded, production resumed, inventory ledger conserved.

The industry fixture now waits for the final washing byproduct before checking completion; the first cast can correctly arrive before the last ore batch.

Reproduce with `tools/Review-Navigation.ps1 -Manual`, `tools/Review-IndustrialLine.ps1 -Mode Processors`, and `tools/Review-Assembler.ps1 -Mode Window`, specifying the desired executable. All use disposable save locations.

Final release-fast build passed (`target/navigation-review/build-planet-shaft.log`). Frozen executable: `target/navigation-review/final/candidate.exe`, SHA-256 `1CDB54E168F999D570D417FE78BF2C200A59EB3205D2E2342B2697823964A931`. Reproduce the current-generator lift route with `tools/Review-ExpressLift.ps1 -World planet -GeneratorVersion 287 -Seed 1337` and a fresh review directory.

## Scope still open

This is a concrete shared-machine foundation, not completion of the entire modpack/civilization vision. General item/material transport beyond resource buffers, broader physical recipe contracts, progression across alternate fantasy/industrial routes, settlement production planning, warfare and strategy integration still need implementation and end-to-end gameplay acceptance. No performance improvement is claimed from timing measurements in this review. Existing authored styling is retained.
