# Spherical watershed review

The locator uses the current Planet generator and the authoritative spherical
receiver graph. It picks the nearest eligible inland confluence on the spawn
face and keeps camera positions in physical metres across voxel sizes.
Run from the repository root:

```powershell
$build = 'target/planet142-build'
$bin = "$build/release-fast"
cargo build --locked --offline --profile release-fast --target-dir $build -p minecraft_style_voxel_prototype
cargo build --locked --offline --profile release-fast --target-dir $build -p engine_worldgen --example watershed_review
& "$bin/examples/watershed_review.exe" 74 target/watershed-height.pgm |
    Set-Content target/watershed-poses.txt
if ($LASTEXITCODE -ne 0) { throw 'Watershed locator failed' }
$version = [uint32]([regex]::Match((Get-Content target/watershed-poses.txt -Raw), 'generator=(\d+)').Groups[1].Value)
& tools/Review-RegionArt.ps1 -Executable "$bin/minecraft_style_voxel_prototype.exe" `
    -LocationsFile target/watershed-poses.txt -OutputRoot target/watershed-native `
    -Generators $version -Scales '1m','0.25m' -Views orbit,high,medium,low,ground
```

The PGM places near (1 m footprint) and coarse (512 m footprint) hillshade
side by side at identical 16 m sampling positions. Each panel covers about 4 km.
This is the height field without water, vegetation, structures or atmosphere.
It isolates sampling changes; it does not measure which terrain the renderer
actually draws between vertices.

The native review freezes the executable and uses disposable saves. It records
the exact camera pose, executable hash, streaming logs, and sampled process
memory for each scene. Capture timings include readback. Eye-level captures
show the view available to a player, but do not establish that the route is
walkable. Traversal, ecological coherence, settlement suitability and streaming
performance while moving require separate evidence.

Seed 74, generator 154: the chosen junction is at approximately
`(-14625, 13)` metres with 3.23 km² of rainfall-weighted upstream area.
The sampled junction bed is 102.80 m at both 0.25 m and 1 m footprints;
at 512 m the narrow bed and water are filtered and the sample is 104.81 m.
The grayscale comparison exposes rectangular slope facets in both panels.
The coarse solve is sampled with bilinear cell weights, a plausible source
of the sharp slope changes at grid boundaries. The artifact is already
visible before chunk meshing or material shading.

The first native runs did not pass acceptance. At 1 m, both the orbital and
eye-level scenes logged repeated presentation timeouts and produced no image.
The eye-level scene recorded 944 MiB sampled working set and had 268 pending
streaming requests at its last sample, with only two measured frames. The
0.25 m eye-level process exited before recording scene metrics (exit
`0xC0000008`). These are failed capture attempts, not comparable performance
measurements.
