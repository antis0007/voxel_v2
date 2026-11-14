Upstream: game-networking-sockets-sys 0.3.0, https://github.com/hussein-aitlahcen/gns-rs
License: MIT OR Apache-2.0, as declared in Cargo.toml. The bundled Valve source
retains its BSD license in thirdparty/GameNetworkingSockets/LICENSE.

Local change: build.rs accepts GNS_PREBUILT_ROOT, an existing x64 Windows build
output containing bindings.rs, lib/GameNetworkingSockets_s.lib and
vcpkg/installed/x64-windows-static-md-release/lib/*.lib. It copies the generated
bindings and links the actual native libraries and dependencies. No DOCS_RS
substitution is used. Without that environment variable the upstream build runs.

Use only a trusted build of the pinned 0.3.0 package and its bundled Valve source.
Native outputs remain outside the source tree and are not distributed by this patch.

The unused staticlib crate type is omitted: this workspace consumes the Rust
rlib, which already carries the native link inputs. Emitting a second standalone
C archive duplicates those libraries and substantially increases build disk use.
