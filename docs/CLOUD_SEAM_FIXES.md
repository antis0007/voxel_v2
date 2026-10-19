# Cloud continuity and cost pass

- Disabled feeder arms subtract a signed-field penalty instead of multiplying the field by zero, which created phantom flat surfaces.
- Fair-weather cumulus no longer includes an unconditional secondary deck.
- Shared and local middle-layer bounds move together above the conservative low-layer envelope. Whole-layer depth sorting cannot correctly compose intersecting volumes.
- Upper weather amounts taper under strong low cover; middle cloud types combine with maximum density instead of addition.
- View march caps are 128/48/24 steps for low/middle/high clouds, previously 160/72/72. This reduces the maximum from 304 to 200, not a measured frame-rate improvement.
- Macro coverage uses the existing continuous weather-bank function instead of eight procedural-noise evaluations per density sample.

Local `cargo check --locked --offline --target-dir target` passed, including composed WGSL validation. No tests or GPU review ran in this chat. Visual seam removal and frame-time improvement remain unverified. The separate `run-cloud-seam-fixes.cmd` launcher preserves the previous visual-review build.

Release packaging succeeded with cargo rustc and an explicit output path: target/cloud-seam-fixes/voxel-clouds.exe. The default release executable was locked by a running process and was not replaced.
