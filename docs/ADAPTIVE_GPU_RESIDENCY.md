# Automatic GPU terrain residency

The previous 512 MiB default permanently deferred terrain chunks, even on cards with ample VRAM. The budget now defaults to **Automatic**. It is an allocation ceiling: unused allowance does not allocate GPU memory.

For Vulkan, the game queries `VK_EXT_memory_budget` on the physical adapter selected by wgpu. Device-local heaps are counted once, so mapped BAR memory types do not double-count VRAM. This directly matches the rendering GPU, including multi-GPU systems. On Windows other backends can query DXGI local-memory budget and process usage by adapter vendor/device IDs; ambiguous identical matches use a conservative fallback. Runtime inspection exposed unavailable DXGI information in the managed launch and prompted the direct Vulkan path.

The terrain ceiling subtracts non-terrain process allocations and reserves 20% of the OS budget (at least 512 MiB). Queries run on a background thread every two seconds. The streamer reads atomics; smaller limits take effect promptly, and upward changes use hysteresis to avoid repeated eviction/regeneration. Existing meshes remain drawable through replacement uploads.

Manual limits remain available and are capped by the detected safe allowance under VRAM pressure. Settings store the policy rather than persisting the current automatic allowance. Preferences v5 represent automatic mode as zero; older preferences containing the former 512 MiB default migrate to automatic. Other legacy manual values are retained; v5 manual 512 MiB remains manual.

Without a usable live query, physical local-memory estimates retain extra headroom. Completely unavailable adapter memory information uses a conservative device-type fallback; manual control remains available. Advanced rendering settings show the effective allowance, detected local memory/OS budget, and chunks deferred by a memory limit. An impossible render-distance/memory combination is not reported as fully loaded.

The byte limit, source and worker budgets remain enforced. Automatic mode does not silently reduce the selected render radius. Enough VRAM is still necessary to retain all exact geometry at extreme settings.

## Build and runtime evidence

Frozen source: `target/residency-review/source`; source hashes: `source-manifest.json`. Optimized offline/locked final build log: `target/residency-review/vulkan-build.log`. Executable/hash receipt: `build-receipt.json`. Launch with `run-auto-residency.cmd` (the earlier `run-voxel-gameplay.cmd` also selects this build).

No tests or CI were added or run. Actual runtime comparisons and renderer captures are recorded separately below.

### Actual rendering and residency measurements

RTX 4090, Vulkan, 1600 × 900, Hills seed 2738958700, one-metre voxels, partly cloudy noon, fixed elevated camera. Physics disabled to isolate rendering/residency. All three final runs used the same frozen executable, SHA-256 `2FF3688A33D8F2D738E68501A94C0DACE272C4F0378D69BBECF9BBB50B2D97CA`.

| Policy / selected chunk radius | Requested / resident | Missing / deferred at end | Terrain GPU memory | First full coverage | Average FPS | P99 frame |
| --- | --- | --- | --- | --- | --- | --- |
| Manual 512 MiB / 16 | 3,190 / 1,550 | 1,640 / 1,640 | 496.00 MiB | Never in 45 s | 213.34 | 6.194 ms |
| Automatic / 16 | 3,190 / 3,190 | 0 / 0 | 1,170.18 MiB | 7 s | 194.17 | 6.920 ms |
| Automatic / 32 | 13,101 / 13,101 | 0 / 0 | 4,613.29 MiB | 27 s | 158.02 | 8.651 ms |

The higher FPS with 512 MiB omits over half the requested chunks. Automatic 16 rendered 2.99 million terrain triangles versus 1.51 million in the capped run; this change restores coverage rather than claiming an isolated FPS improvement. Both 16-radius FPS intervals overlapped another chat's CPU compilation, so their FPS comparison is approximate. The final 32-radius run had no other review game or compiler active. Warmup / measurement: 30 / 15 s at 16, 45 / 15 s at 32.

The driver reported 24,356 MiB of local memory and a live budget initially around 23,551 MiB. The terrain ceiling settled at 17,920 MiB for the 16-radius run and 16,320 MiB for the 32-radius run as process allocations changed. Actual retained terrain used only the values in the table. The 32-radius run retained all terrain despite 5,292 CPU source-cache evictions; source residency stayed at 252.53 MiB of its 256 MiB allowance. Both automatic runs had zero GPU evictions.

Final profile logs and actual renderer PNGs:

- `target/residency-review/fixed-512-vulkan/run.log` and `captures`.
- `target/residency-review/auto-16/run.log` and `captures`.
- `target/residency-review/auto-32/run.log` and `captures`.

All three error logs were empty. Settled 16- and 32-radius renderer captures were inspected; the 32-radius image shows exact terrain replacing the earlier simplified distant hills without budget-created gaps. The initial DXGI-only attempt is retained in `fixed-512` and `voxel-dxgi-control.exe` for provenance, and is not the launcher's final build. The source manifest was rechecked with zero mismatches.

These runs verify this NVIDIA/Vulkan hardware and the two selected distances. Other cards, severe external VRAM pressure, unsupported query backends, and extreme render distances still need their own runtime verification. If exact requested geometry cannot fit safely, the settings expose the memory limitation; the implementation does not promise unlimited distance on insufficient VRAM.
