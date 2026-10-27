# Mycopunk netcode study

Reviewed 2026-10-27 from the local decompile in `downloads/mycopunk-20261026/code`
(gitignored, see its README). Reference only: no code or data was copied. Findings
feed `docs/superpowers/specs/2026-10-26-multiplayer-design.md`.

## Shape

Unity Netcode for GameObjects over the Facepunch Steamworks transport (Steam
Networking Sockets, so Steam's relay handles NAT). One host, at most 4 players
(`Online.SetMaxLobbyPlayers(4)`). About 1,000 RPC sites: 375 client-to-server
calls open to any client, 273 broadcasts, 149 "everyone but the owner".

## What it does that we adopt

| Technique | Where | Our use |
|---|---|---|
| Owner-authoritative player movement; remotes interpolate | `ClientNetworkTransform`, `PlayerNetworkTransform` | Section 4: client owns its pose, server validates and corrects |
| Shooter's client decides hits on enemies, shows damage at once, server applies | `EnemyPart.Damage`, `Damage_ServerRpc` | Section 5: client claims, server checks plausibility and computes the amount |
| Rigid bodies synced round-robin, at most 20 per tick, velocity in half floats, clients dead-reckon between updates | `GameManager.SendSyncedPositionsToClients_Server` | Section 5: dead reckoning under a priority accumulator |
| Level terrain regenerated on each client from a 28-byte mission message (seed and indices) | `GameManager`, `SendNamedMessage("MissionData")`, `TerrainGenerator` | Section 6: pristine chunks generated locally |
| Enemy legs, bob and animation computed on each client from the root transform | `EnemyIKLeg`, `syncedBob` | Section 5: replicate root pose and state bytes only |
| Join payload carries build id, player id and a modded flag; mismatches are refused before the session | `GameManager.WriteJoinPayload` | Section 10: content manifest, which also names the differing files |

## What it does that we avoid

- The rigid-body batch goes on a **reliable** channel, so one lost packet holds back
  every later position. Our state channel is unreliable, newest wins.
- Positions and rotations are full `float32` and quaternion (28 bytes plus ids).
  Ours are chunk-relative `i16` and `u16` angles.
- Round-robin without priority: a near object waits its turn behind far ones.
  Ours weights by distance and speed.
- Clients write their own health and call most server RPCs without checks. Fine
  among four friends, not for open servers. Ours validates every claim.
- Terrain is never edited at runtime (`TerrainModifier` and `TerrainCarver` act at
  generation only), so seed-only join is the whole story there. Ours must also
  stream edit deltas and cellular state.
- Late join is limited (`JoinResult.OnlyAllowRejoin`, `DroppingIntoMission`).
  Ours joins at any time.
