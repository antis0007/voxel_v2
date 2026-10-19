# Optional GPU profiler failure handling

Confirmed evidence: `target/stability-review/final-day/errors.log` reports chunk
allocation retries followed by a fatal `Buffer::get_mapped_range` on destroyed
`gpu timestamp readback 4`. The mapping completion result alone does not prove
the buffer remains valid when the rendering thread later consumes it.

The profiler now:

- Checks scoped allocation/validation errors before exposing its query sets and
  buffers. Allocation failure leaves GPU profiling unavailable.
- Observes device loss through an atomic callback signal and stops admission,
  resolving and mapping once loss is known.
- Treats failed device polling or mapping as a reason to retire the profiler.
  Failed mappings are not blindly unmapped or recycled.
- Contains map-request and mapped-view acquisition failures at the optional
  profiler boundary. wgpu 27.0.1 has no fallible mapped-view getter; its backend
  can panic if a successfully mapped buffer is subsequently destroyed. This
  narrow containment covers that remaining race. The normal panic hook may
  still print a caught diagnostic, followed by the profiling-disabled message.
- Drops the entire profiler on the next collection boundary, clearing timing
  availability rather than presenting stale GPU measurements. Submission IDs
  continue to reject stale completion callbacks.

Mapped timestamp words are copied through byte decoding rather than assuming
the host mapping is suitably aligned for a typed `u64` slice.

This change isolates optional timing readback failures. It does not implement
full graphics-device recreation, guarantee that unrelated allocations survive
device loss, or establish that the game can continue rendering on a lost device.
No tests, CI, game launch or injected device-loss/memory-pressure replay was run.
Local verification uses Cargo compilation and an optimized executable build.

Optimized candidate: `target/profiler-safety-review/voxel-workshop-profiler-safe.exe`.
SHA256: `FFC25E66483A4C87D8DA9871C156C242B1E8433E91BC367125866DFCE0E03483`.
Integration build passed (1m 26s). All 96 recorded source files remained
unchanged across compilation; receipt: `target/profiler-safety-review/source-before.json`.
