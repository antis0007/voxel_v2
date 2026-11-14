# 0043: Bounded terrain progress during state replication

Accepted 2026-11-06.

The outbound pacer previously served terrain only after both gameplay images and
cell edits became empty. Continuous edits could prevent a joined player from ever
receiving new terrain. Use weighted byte service: images receive two shares,
cell edits one, and terrain one. Empty or backpressured lanes keep no credit.
Unused shares are available immediately. The existing 256 KiB/s per-peer cap,
quarter-second burst, realtime queue reserve and transfer framing remain intact.
When all lanes contend, images retain half the bytes, while edits and terrain
receive one quarter each. With only images and edits, their split is two to one.

Compute a transfer's FNV checksum once when creating its sender. Rejected header
writes reuse it instead of rescanning up to 16 MiB. Verify received bytes
incrementally, so the final fragment does not cause a full-buffer checksum pass.
This preserves the wire format, size limits, ordered offsets and corruption
checks. The checksum is not authentication; signed identities remain separate.

`engine_lobby` adds `anyhow` as a development dependency, already used by the app,
to compile the production pacer directly in headless integration tests. No runtime
dependency is added. Tests exercise two peers, continuous images and edits,
terrain completion, unequal packet sizes, backpressure, authenticated reconnect,
fresh baselines and host loss. Local relay tests do not establish WAN/SDR behavior,
gameplay save restoration or civilization support in multiplayer.
