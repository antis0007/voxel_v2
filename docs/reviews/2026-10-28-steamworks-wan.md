# Steamworks SDR WAN acceptance — 2026-10-28

Real gameplay passed between a native Linux host on the Toronto VPS
`172.81.178.223` and Windows clients on the development machine. Both ends used
`VOXEL_STEAM_FORCE_SDR=1`, disabling direct ICE routes. Clients joined by Steam
identity. No port forwarding or firewall changes were made. Voice chat is excluded.

## Gameplay evidence

- `target/wan-sdr-pvp`: host and two clients exited successfully. Host-authoritative
  damage reached zero health, followed by death and respawn to 100 health. Public
  chat, private messages, operator commands and denied nonoperator commands passed.
  The receiving client dispatched combat sounds on an initialized audio device.
- `target/wan-sdr-building`: host, two scripted clients and a rendered third client
  exited successfully. Mining produced Dirt inventory; four Torch placements
  consumed inventory and appeared in the remote world. Crafting was attempted and
  host command acknowledgements advanced; the crafted output was not visually
  inspected. Terrain synchronization reached 27/27 chunks and the horizon completed.
- `target/wan-sdr-building/captures/bench-001.png` was inspected: remote player,
  placed torches, landscape and synchronized player roster are visible.
- PvP-disabled attack handling was checked separately in the preceding local
  forced-SDR acceptance run, `target/sdr-pvp-disabled-gameplay`.
- The preceding networking library run passed 74 tests:
  `target/steamworks-core-tests.log`.

These are automated gameplay runs with renderer inspection. The Windows peers
shared one machine and one signed-in Steam account, using separate signed game
profiles. The Linux host was a separate machine across the public internet.
This does not establish two-account friend-invite behavior, human combat feel,
subjective sound quality, long-session reliability or comparative Minecraft performance.

## Remote resource measurements

Measured on the actual VPS with `/usr/bin/time -v`; values include startup,
tracing and shutdown in a small review arena.

| Run | Wall time | CPU time, user + system | Average CPU | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| PvP, two clients | 43.95 s | 13.10 s | 29% of one core | 83.6 MiB |
| Building, three clients | 48.05 s | 15.13 s | 31% of one core | 76.6 MiB |

Both runs reported zero swaps and exit status 0. These are short acceptance
measurements, not full-world capacity or bandwidth benchmarks. Logs and raw
measurements are in each run's `remote-metrics.txt`. The harness labels SSH
client measurements separately; those are not server CPU measurements.

Steam handles SDR traffic; the project's lobby relay does not carry gameplay for
this transport. Normal operation allows direct-or-SDR routing; forcing SDR was
only necessary to prove relay traversal here. Dedicated simulation pauses when idle.

## Runtime identity

- Windows executable: `target/multiplayer-sdr.exe`
- Windows SHA-256: `5D0EF9ECD92292BD5EFF69F4063024AA4DA1922AADB34CF73A2BB4FF759170F0`
- Runtime archive: `D:\CodexBuilds\voxel-sdr-wan-20261028\voxel-sdr-wan-runtime.tar.gz`
- Archive size: 39,850,630 bytes
- Archive SHA-256, verified after upload:
  `CC335AD85E800F8148761C0947B1B08D5B4778EFF25F235D8CB90A1949B40041`
- Shared compatibility ID:
  `16f5f7f50d9df4b060a6ed8705284fb9f7d71b3a32161d68bbc3a46f5533ad88`
- Development App ID: 480. This is development acceptance, not approval to ship
  under Valve's sample App ID; release needs the game's own Steamworks configuration.

The approved package contains runtime binaries, runtime assets, required native
libraries and the launcher. It excludes source, existing saves, player keys,
credentials and SDK caches/logs. No existing service was replaced.

## Cleanup and reproduction

Both temporary hosts closed cleanly. A subsequent remote process check confirmed
`TEMPORARY_HOSTS_STOPPED`; `voxel-lobby.service` remained active with its existing
128 MiB memory limit and 40% CPU quota. The isolated runtime and evidence remain
in `~/voxel-sdr-review-20261028/` for reuse; no game host remains running.

The reusable local harness is `tools/Review-NativeMultiplayer.ps1`. Acceptance
used `-Steamworks -RemoteHost ubuntu@172.81.178.223` with separate output roots;
the building run additionally used `-NoPvp -Work -RenderClient`. The launcher is
`tools/run-steam-server.sh`. Remote runs have bounded lifetimes and isolated saves.
