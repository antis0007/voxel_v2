# Adaptive cloud latency repair

The cloud governor could raise its resolution divisor without changing the
actual image: the normal 640x360 image floor cancelled adaptation at common
window sizes. Its request key also caused full renderer resizes for ineffective
steps, and effective cloud changes recreated the swapchain, scene depth,
atmosphere and postprocessing allocations.

The requested quality keeps the existing 640x360 floor. Only sustained GPU
overload enables a temporary 426x240 floor, with at most two divisor increments
from the effective requested setting. High resolution windows may use divisors
up to six. The existing asymmetric overload/headroom windows still control
recovery. Terrain resolution, geometry and cloud density/lighting are unchanged.

Target identity now follows the effective cloud divisor and scene scale.
Ineffective steps do not recreate resources. Cloud-only adjustments rebuild
only the cloud output/current/history textures and their dependent bindings;
window and scene-scale changes retain the full resize path.

The egui end-frame path also avoids an empty queue submission when callbacks
produce no separate command buffers. wgpu defers `write_buffer` transfers until
the next submission, so the enclosing frame's normal submission already uploads
the data before rendering. Real callback buffers retain their earlier ordering.
This removes one redundant submission across gameplay, menus and loading.
Reference: https://docs.rs/wgpu/27.0.1/wgpu/struct.Queue.html#method.write_buffer

`--benchmark-adaptive-clouds` opts a performance capture into the production
cloud controller. Other benchmark runs continue to disable adaptation and world
quality changes. `tools/Measure-CloudPerformance.ps1 -AdaptiveClouds` records
the option in the capture manifest. Compare fixed and adaptive runs using one
frozen executable and otherwise identical settings. Any gain trades cloud
resolution for frame time; it is not an identical-quality shader optimization.

Build log: `target/cloud-adaptive-lag-build.log`. Frozen candidate, hashes,
native frame captures and GPU timings: `target/adaptive-cloud-review-20261024/`.
No tests or hosted CI were added or run.

## Native measurements

Final same-binary captures (`final-fixed`, `final-adaptive`): RTX 4090/Vulkan,
1920x1080, partly cloudy, fixed seed/camera/clock, 16 chunk view, 10 second
warmup and 8 second measurement. Both drew 431 terrain batches and 1,789,125
triangles; no configuration crossovers or peer-renderer overlap were recorded.

| Mode | Cloud divisor | Average FPS | p95 frame ms | Worst frame ms | Cloud GPU ms | Total GPU ms |
| --- | --- | --- | --- | --- | --- | --- |
| Fixed quality | 3 | 63.06 | 16.84 | 18.26 | 9.48 | 14.79 |
| Adaptive | 4 | 74.06 | 14.58 | 15.46 | 7.08 | 12.38 |

The adaptive controller logged its effective divisor change during warmup.
Terrain GPU time remained 5.10 ms in both runs. Adaptation reduced cloud image
size from 640x360 to 480x270, with scene resolution remaining 1920x1080. This
is a bounded visual tradeoff, not an identical-quality shader optimization.

Earlier captures (`fixed-clean`, `adaptive-clean`) preceded removal of the
empty UI queue submission and had variable terrain timings and major CPU/
surface-acquisition stalls. Do not attribute their entire difference to one
change. Those stalls did not recur in the final eight-second samples; this
does not establish their elimination in all gameplay. Interrupted/overlapped
captures are invalid for timing comparisons.

Inspected production-renderer world capture
`adaptive-visual/camera.png` and menu export `menu-art/title.png` after the UI
submission change. The separate visual run logged divisor 4 without renderer
validation errors. Capture timing is excluded from the comparison above.
