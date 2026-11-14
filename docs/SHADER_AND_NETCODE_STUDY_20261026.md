# Early-pass shader and netcode study

Two questions drove this work: what an earlier pass of the world shader did that the
current one lost, and what the networking structure is before matchmaking and
netcode change. This records what was extracted, what the structures are, the one
defect that was fixed, and what is still open.

## Extraction

No decompiler was needed. The candidate binaries in `%USERPROFILE%\Downloads` were
checked first. The only .NET assemblies there belong to a Unity game and a RimWorld
mod, neither of which is this engine, and the voxel titles present are Java or native
code. dnSpy and Ghidra are installed on this machine and had nothing relevant to open.
The earlier passes of this engine exist as WGSL sources, so extraction is a file copy.

Extracted to `downloads/study-20261026/`, which `.gitignore` keeps out of the tree:

- `early-pass-shaders/terrain_shader_upgrade/` from `terrain_shader_upgrade.zip`, the
  2026-10-19 pass: revised body, the original body it replaced, the unified diff, the
  integration contract, and its validation fixtures. `CONTENTS.json` holds a SHA-256
  per file; the revised body is `522d91af...c16481`.
- `early-pass-shaders/world_shader_root-before_20261024.wgsl` and
  `world_shader_main-before_20261024.wgsl`, the 2026-10-24 pre-edit snapshots.
- `netcode/net` and `netcode/lobby`, copies of the two networking crates.

## Shader structure

| Body | Lines | Functions |
|---|---|---|
| 2026-10-19 original | 1621 | 32 |
| 2026-10-19 revised | 1797 | 50 |
| 2026-10-24 snapshots | about 3340 | 88 |
| current | 3716 | 88 |

The current body is a superset of the early pass. Twenty-five of its 32 functions are
still there. Of the seven that are gone, `aces_tonemap` moved to `postprocess.wgsl`,
`saturate_color` and `specular_term` were absorbed into the `terrain_specular` and
colour helpers, `torch_flicker` became `torch_light_flicker`, and `weather_clip_region`
and `sample_weather_bilinear` became the `weather_edge_distance` and
`weather_level_weight` pair.

The seventh has no replacement: `far_shade`.

### `far_shade`, the distance-stable relief field

```wgsl
// Block colour is the biome. These two octaves only bend the smooth normal.
// Wavelength snaps to powers of two and crossfades, so the pattern stays put
// in the world and a crest stays a few pixels wide as it recedes.
fn far_shade(n: vec3<f32>, xz: vec2<f32>, footprint_m: f32) -> vec3<f32> {
    let lod = log2(max(footprint_m, 1.5));
    let w0 = exp2(floor(lod)) * 5.0;
    let t = fract(lod);
    let g0 = surface_noise_grad(xz / w0 + vec2<f32>(2.0, -4.0));
    let g1 = surface_noise_grad(xz / (w0 * 2.0) + vec2<f32>(-5.0, 1.0));
    let g = mix(g0.yz, g1.yz, t);
    return normalize(n + vec3<f32>(g.x, 0.0, g.y) * (0.20 + (1.0 - n.y) * 0.10));
}
```

Three properties carry the idea. The wavelength comes from the pixel footprint and
snaps to a power of two, so a feature is about five pixels across at every distance.
Two octaves crossfade on the fractional part of that level, so the pattern never jumps
when the level steps and never swims in screen space. The tilt grows on faces that are
not up-facing, so a crest keeps its silhouette.

The current body replaced this with two fixed wavelengths, 0.58 m and 0.17 m, whose
amplitude fades out through `surface_detail_weight`, plus the horizon detail map
beyond that. A fixed wavelength that fades to nothing cannot hold slope at distance: it
either aliases while it is still visible or leaves a flat normal once it is gone. That
is the trade the early pass avoided, and it is the trade to re-open when canopy relief
next moves.

The same package's `INTEGRATION.txt` lists what the body needs from the renderer, and
is still accurate: shadow bindings, camera frame, atmosphere and cloud layer,
underwater, and the residency/LOD handover. It also states plainly that the package was
never run in the game, which is why the fixtures under `validation/` compared numbers
rather than images.

## Defect fixed: leaf normals at mid distance

`kind == 6` gave leaves a shading-normal tilt from a 4.8 m noise field. Three things
were wrong with it, and together they produce the mottled, alternately lit and unlit
crowns that the canopy captures show.

The gradient came from `surface_noise_grad`, unfiltered, while the amplitude was scaled
by `surface_detail_weight(fp_m / 4.8)`. Filtering and weighting disagreed, so the field
was sampled at full amplitude everywhere the weight was non-zero and then faded.

The amplitude was 0.28 on a quintic gradient that is about 1.3 per cell at a typical
sample and up to 5.6 at the strongest, since the quintic derivative peaks at 1.875 and
the cell corner differences reach 3. That puts a typical side face near 20 degrees of
tilt and the strong samples past 40. Ground relief does the same job with 0.060 at a
0.58 m wavelength, so the leaf path was an order of magnitude stronger than the path
that was tuned against native captures.

Foliage alone is lit by a wrap lobe, `saturate1((n dot sun + 0.08) / 1.08)`, while every
other material also carries `sun_horizon`. A tilt that large crosses the wrap boundary,
so neighbouring pixels on one crown flip between direct sun and no direct sun. That is
the glitch, and it is normal maths rather than a lighting fault.

The fix uses `surface_noise_grad_filtered` so the gradient and the fade agree, and
reduces the amplitude to 0.08, which is about six degrees at a typical sample. Crown
shape continues to come from the clump colour field, where the 2026-10-24 pass put it.

## Networking structure

Four layers sit under the game:

- `crates/net/src/protocol.rs` is the game packet format: `VXMP`, a `u16` version, then
  a one-byte tag. `Input` is 26 bytes, so a packet is 7 bytes of header plus that.
  `MAX_PACKET` is 1024 and an application payload is capped at 900 bytes.
- `crates/net/src/session.rs` is the host session: admission, `WorldIdentity`
  comparison, a 120 messages/second budget per peer, input sequence and tick-lead
  checks, and the once-per-tick `advance` that builds one `Frame` for the roster.
- `crates/net/src/lobby/relay.rs` and `steam.rs` are two transports behind the same
  session rules. The relay treats the lobby as an opaque byte pipe with `ToHost` and
  `ToPeer`.
- `crates/net/src/lobby/wire.rs` is the lobby framing: a `u32` body length, a one-byte
  tag, then fields. `MAX_BODY` is 64 KiB.

`crates/lobby` is the server. `crates/lobby/src/directory.rs` holds sessions in three
hash maps with a partial top-k browse. `crates/lobby/src/server.rs` runs one reader
thread and one writer thread per connection over a single `Mutex`-guarded registry.

The matchmaking path is: `Host` registers a session and receives a 40-bit code from the
CSPRNG, `Browse` returns a filtered page sorted by players, newest or name, `Join`
carries the code plus the client's generator version and digest, and the lobby refuses a
build whose version or digest differs from the host's. `Joined` returns the peer id and
`WorldParams`, so the joiner can build the host's world before any game packet is sent.

Cost of the hot path, read from the encoders: a `Frame` with all 16 players present is
512 bytes, at 30 Hz, once per peer. The relay adds 11 bytes per packet, being 4 for the
length, 1 tag, 4 peer id and 2 inner length. `TCP_NODELAY` is set on the relay socket.

## Proposals, in the order I would take them

1. Split the lock. `browse` holds the registry mutex for a full pass over the session
   map, and relay routing needs that same mutex. Two locks, one for the directory and
   one for routes, takes listing cost out of the relay path. No dependency, no wire
   change.
2. Give `Frame` a presence bitmask and stop sending `sequence` and a 64-bit `tick`.
   Presence is one byte per player today. A mask plus the present 26-byte inputs is 434
   bytes for a full roster; dropping the per-input `sequence` and sending a `u32` tick
   makes each entry 14 bytes and the frame 238. Against 512 today, on the packet that
   leaves 30 times a second for every peer. Wire version bump.
3. Measure lobby round-trip time and sort by it. `Ping` already exists in the lobby
   wire, and `Entry` has no latency field, so listing cannot prefer a nearer host
   today. This is the part of matchmaking a player feels most.
4. Replace thread-per-connection before any public lobby. At the default 512
   connections that is up to 1024 threads sharing one mutex. `std` has no readiness
   API, so this is a dependency decision and belongs in an ADR before code.
5. Coalesce a peer's per-tick sends into one lobby message, so a tick costs one framing
   layer instead of one per packet. Wire version bump, and only worth doing after item
   2 has shrunk the payload.

## Not verified

The leaf change was validated by the build script's WGSL pass, which reported
`Validated 19 composed WGSL programs`. No native capture of it was taken in this
session. A concurrent session is editing the same shader and the same lighting area, so
a before and after pair from this tree would not isolate my edit and is not reported as
one. The `lush-before-fa` and `lush-before-fb` captures under
`target/reviews/forest-repair-20261021/` are the current-eye baseline for whoever takes
the pair.

None of the lobby or relay code was changed, so the validation recorded in
`crates/net/README.md` still describes that transport as it stands. Proposal 2 is
implemented; the protocol section below records it.

## Compiled shader programs recovered

The compressed container turned out to be plain LZ4 blocks over the per-platform
program bytes, not a proprietary format. Decompressing each block exposes a DXBC
container, so the extraction now holds:

- 718 compiled programs from all 725 shaders, in `dxbc/`, indexed by
  `dxbc_index.json`.
- 100 tuned materials whose shaders the game itself authored, with every property
  value in `game_materials.json`.
- 25 texture pages sampled by those materials, in `textures/`.

Disassembly is the one gap. `dxc` rejects these containers because they are SM5 DXBC
rather than DXIL, and `fxc` rejects them too, so they need header repair before a
disassembler will accept them. RenderDoc, 3Dmigoto and similar tools read the
containers directly.

## Netcode: protocol 3

`crates/net/src/protocol.rs` now sends the roster as a presence bitmask plus one
payload per present player, and the frame tick once instead of per player:

| Field | Before | After |
|---|---|---|
| Frame header | 8 byte tick, 1 byte count | 8 byte tick, 1 byte count, 2 byte mask |
| Per present player | 4 id + 1 presence + 26 input | 4 id + 13 input |
| Per absent player | 4 id + 1 presence | 4 id |
| Full 16-player frame | 512 bytes | 290 bytes |

At 30 Hz for 16 peers, host egress falls from about 247 KB/s to 143 KB/s once the
relay's 11 byte wrapper and the 4 byte length prefix are counted. An `Input`
packet also drops from 33 to 28 bytes.

The changes stay inside the wire format: `Packet::Frame` and `Input` keep their Rust
shape, so nothing above the protocol changed. Encoding now refuses a sequence wider
than `u32` and any button bit outside the four reserved ones, and decoding refuses a
presence bit beyond the roster. Five tests in the module pin the round trip, the exact
sizes, and both refusals. `cargo test -p engine_net` reports 20 passed and
`cargo test -p engine_lobby` reports 19 passed.

Not done: the lock split (proposal 1) and the send coalescing (proposal 5). Both
remain as written above.

