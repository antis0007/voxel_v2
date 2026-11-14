# Generator 92: shorter regions and clustered households

Fresh Hills and Ridged worlds use a 6,144 m geology face with 240 m relief,
down from 10,240 m and 360 m. Drainage and climate still use the same solve.

Four adjacent household parcels now place their homes 22–40 m from a shared
lane junction instead of scattering each home independently across a 256 m
cell. Setbacks vary by seed. Doors face inward. The 512 m regional grid is
still an underlying constraint; this is a first settlement distribution pass,
not a simulated civilization or a guarantee of four surviving homes per town.

Each home requires a dry, supported route to its junction. The route surveys
terrain every three metres plus shoulders, rejects steep steps and reserved
landmarks, and becomes a dirt lane only when its household is admitted.
Unsuitable parcels remain empty rather than building over cliffs or water.
Existing household roles, resident IDs, stock and interaction are reused.

The separate cottage/farm/inn planner uses the same district centres: candidate
admission is 80% within 120 m and 4% elsewhere, before terrain and reservation
rejections. Entrances face their district centre. Boss ruin campuses leave a
160 m gap around centres. Homesteads can occupy suitable uplands below 300 m.
These are placement rules, not measured population or route guarantees.

Saved generator versions retain their terrain and placement. Create a fresh
world to see this change after rebuilding the game. Offline `cargo check -p
engine_worldgen` passed using an isolated target directory because the shared
build directory was locked. No tests were added or run, per project instructions.
The game executable was not rebuilt; native pacing and appearance remain unreviewed.
