# Anchor Studio

Double-click **Open-Anchor-Studio.cmd** in the project’s tools folder, or run `./tools/Open-Anchor-Studio.ps1` in PowerShell. The launcher builds the local compiler and opens the editor at http://127.0.0.1:4377. It uses the project's Rust toolchain and Node. The first launch can take a few minutes. Close the launcher window to stop Studio.

## Design a building

1. Pick a building type and click **Load template**. Apartments, clinics, offices, workshops and relays each start with a different layout.
2. Drag with **Room** to redraw part of the floor plan. Use **Door** on shared walls and **Entrance** on outside walls. **Layout tools → Connect rooms** places interior doors automatically. For multiple floors, give the stair a 2 × 2 area within one room or choose **Find stair space**.
3. Pick a roof, windows, and materials. Drag in the 3D viewport to orbit, scroll to zoom, and use **Cut height** to see the interior. **Edit in 3D** applies the same plan tools in the cutaway. Floors repeat the plan around a common stairwell.
4. **Save** writes an editable design to `assets/anchor-studio/designs`, where it can be version-controlled. Use **Open** to continue it. **Duplicate** starts a separate draft. **Make a variation** also changes the material scheme and weathering seed. **Export → Editable design** downloads a portable `.vxplan` file.
5. **Review in city** samples your design in a real Anchor parcel on Hills seed `424242` at 1 m per block. The city still enforces street and megastructure clearances. The building preview and blueprint export use the block size you choose in the editor.

The room compiler reports disconnected rooms, uncovered bays, and invalid stair footprints above the 3D viewport. Drafts can be saved while you fix them.

## Place it in-game

Choose the same block size as the destination world and export **Game blueprint**. Copy the `.vxbp` into the game’s `blueprints` folder under its save directory, then load it from the creative blueprint shelf and place it. The file contains the sampled building blocks. Keep the `.vxplan` for further room and facade editing.

## Review in the game renderer

Build the game from this checkout, then run the place review with the exported design. Use `anchor:studio-port` for a Workshop and `anchor:studio-office` for the other building types.

```powershell
./tools/Review-Places.ps1 -Executable ./target/release-fast/minecraft_style_voxel_prototype.exe -Label my-workshop -Mode anchor:studio-port -Generator 229 -Design ./My-Workshop.vxplan -SkipHorizon
```

The preview design replaces that building type where it fits in a fresh review district. Studio ships the design as a blueprint or a review asset. Adding it to the default world generator is a separate release change.
