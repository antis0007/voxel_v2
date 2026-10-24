---
paths:
  - "crates/app/src/shaders/**"
  - "crates/app/src/render/**"
  - "crates/app/src/render.rs"
  - "crates/app/src/shader_sources.rs"
  - "crates/app/src/postprocess*"
---
# Render contracts

- A host struct (`#[repr(C)]` uniform, vertex layout, bind group layout, texture format) and its WGSL declaration are one contract. Change both sides in the same patch and check binding indices, sizes, formats and counts. No WGSL file may change while the contract does, so a Rust-only diff can still break a shader.
- Shaders are concatenated in `shader_sources.rs` and validated by `build.rs`; a new `.wgsl` must be listed there. `cargo check` runs that validation, but it does not prove the pipeline or the picture is right.
- Colour ownership (sRGB palette decode before interpolation, HDR `Rgba16Float` until `postprocess.wgsl`) is in `docs/design/VISUAL_DIRECTION_EXECUTION.md`.
- Evidence for a visual change is a before/after capture from the same pose via `tools/Review-VisualDirection.ps1` with a unique `-Label`; inspect the PNGs. Speed gained by missing geometry or detail is not an optimisation.
- Camera-glued geometry is textured in view space; never discard small moving geometry with the world voxel grid (see the render skill).
