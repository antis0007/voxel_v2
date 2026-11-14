#!/usr/bin/env bash
set -euo pipefail
build_root=${1:?Pass an isolated native Linux target directory}
mkdir -p "$build_root"
# rustc 1.93.1 can panic rendering warning spans on the Windows-mounted tree.
# Suppress warning emission for this build without changing generated game code.
cat > "$build_root/rustc-lint-wrapper.sh" <<'WRAPPER'
#!/bin/sh
exec "$@" --cap-lints allow
WRAPPER
chmod u+x "$build_root/rustc-lint-wrapper.sh"
export RUSTC_WRAPPER="$build_root/rustc-lint-wrapper.sh"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_RELEASE_FAST_DEBUG=0
cargo build --locked --profile release-fast --features multiplayer-sdr \
  -p minecraft_style_voxel_prototype --bin minecraft_style_voxel_prototype \
  --target-dir "$build_root" -j 2
