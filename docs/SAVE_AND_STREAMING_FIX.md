# Save permissions and streaming overhead

The old implementation resolved maps, checkpoint locks and preferences relative
to the launch working directory. Explorer/shortcut launches could therefore use
a different or unwritable folder. New writes use
`%LOCALAPPDATA%\VoxelWorkshop\saves` on Windows. `VOXEL_SAVE_DIR` can explicitly
override the location. Errors include the affected directory or checkpoint.

The browser also discovers legacy `saves` directories in the launch folder and
beside the executable's parent directories, including this repository when a
candidate lives under `target/<review>`. Opening a legacy world copies that
world on the library worker, publishing its destination directory only after
all copies finish. Original files remain untouched. Import staging directories
are excluded from discovery. Copied files have their Windows read-only flag
cleared, so installation permissions do not propagate into future writes.
Legacy preferences remain readable; subsequent changes write to user storage.
Import can take time for large worlds, but does not copy the entire library at
startup or block the rendering thread.

Resident edit allocation accounting now updates at insertion, eviction and
mutation boundaries, including torch-table growth. Repeated per-frame resident
memory queries are constant time. Water allocation accounting remains chunk
partitioned. Dirty-chunk admission uses a deduplicated rotating queue, examining
at most 64 entries and admitting at most four writes per streaming call; it no
longer allocates and sorts the entire dirty set each frame. New/legacy chunks
register at admission rather than requiring a resident-map sweep every frame.
Physics discovery uses contiguous padded rows and direct halo offsets, and only
allocates an activity mask when an unstable cell is found.

Verification is limited to local compilation and an optimized executable build.
No tests, CI, game launch, save round trip or FPS benchmark were run. These
changes remove specific CPU costs and launch-folder write failures; overall FPS
and streaming throughput still require runtime measurement on the user's scene.

Candidate: `target/save-performance-review/voxel-workshop-save-fix.exe`.
SHA256: `368D293A3675D6100454694446288D3213F25E4CFA6D924863CDD3B0E8DF819E`.
Build log: `target/save-performance-release.log` (optimized build succeeded).
