# Multiplayer latency: prediction, interpolation and adaptive lead

The goal is feel: the local player reacts on the next frame rather than one round
trip later, and other players move smoothly instead of stepping once per packet.

## What landed

`crates/net/src/predict.rs` holds three pieces, all keyed by tick with no clock reads,
so behaviour is reproducible in a test:

- `Predictor`: runs the local fixed step immediately, keeps the inputs the host has
  not confirmed, and on a host frame adopts the confirmed state and replays what is
  still in flight. A snapshot older than the confirmed tick is ignored rather than
  rolled back, because the transport is ordered.
- `Interpolator`: renders another player a fixed number of ticks in the past so
  motion is smooth between frames. Positions clamp at both ends, so a late packet
  can never extrapolate a player through a wall.
- `Rtt`: exponentially weighted round-trip and jitter estimates, plus the input lead
  they justify, clamped to 2 through 8 ticks. A fixed lead either drops inputs on a
  slow link or adds latency on a fast one.

## Why the numbers matter

Input lead was fixed at 3 ticks. `Rtt::lead_ticks` now derives it from the measured
path: about 1 tick of lead at 20 ms with no jitter, and the 8 tick ceiling on a link
that needs it. The cap matches what the host session already accepts, so an adaptive
lead cannot push an input outside the window the host validates.

## Verification

Nine tests in the module, and `cargo test -p engine_net` reports 60 passed. They cover:

- prediction moves before any host answer,
- an agreeing host changes nothing and replays only the inputs still in flight,
- a disagreeing host is adopted and replayed, including the case where the client has
  no collision knowledge and the host clamps at a wall,
- stale snapshots are ignored,
- pending inputs stay bounded,
- interpolation renders between samples, clamps at both ends, and drops repeated ticks,
- the lead rises with latency and clamps at the ceiling.

## Not wired, and what that means

Nothing calls these yet. The client still sends inputs and applies frames without
prediction, because the integration points live in `crates/app/src/multiplayer.rs`,
which does not compile while another session restructures the inventory UI. The
module is API complete and tested; it is not yet in the game loop.

## Measured on a simulated link

`crates/net/src/simnet.rs` drives the real `HostSession` and the real `Predictor`
across a link with configured delay and jitter, 300 ticks at 30 Hz, one honest client
and stride 2. Arrival ticks are clamped so each direction stays ordered, which is what
the reliable relay does. Six tests assert the numbers below.

| Link | Lead | Inputs late | Corrections | Worst frame age |
|---|---|---|---|---|
| LAN, 2 ms | 3 ticks | 0 of 297 | 0 | 1 tick |
| Home, 80 ms + 20 ms jitter | 3 ticks | 0 | 0 | 4 ticks |
| Poor, 200 ms + 60 ms jitter | 3 ticks | **231 of 297** | **227** | 5 ticks |
| Poor, 200 ms + 60 ms jitter | 8 ticks | 0 | 0 | 4 ticks |

Two findings worth keeping:

- On a 200 ms link a fixed 3 tick lead loses three quarters of the player's inputs.
  The host never sees the motion, so the client is corrected on most frames. That is
  the concrete case for a lead that follows a measured round trip, and it is why
  `Rtt::lead_ticks` exists.
- At 8 ticks on the same link, nothing is late and nothing is corrected, at the cost
  of frames arriving about four ticks old. That trade is the whole latency budget in
  one table.

Bandwidth for a one player session is about 1.0 KiB/s from host to client: a frame
carries only the players who acted, so the 16 slot ceiling costs nothing when empty.

## Next, in order

1. Wire `Predictor` into the local player step and `Interpolator` into remote player
   rendering. This is the change that makes latency feel different, and it is blocked
   on `crates/app` compiling.
2. Split the lobby lock so a directory browse cannot stall relay dispatch, and
   coalesce a peer's per-tick sends.
3. Extend the simulator to several clients at once, so relay fan-out and the host's
   per-tick cost are measured rather than assumed. The single client harness exists;
   it says nothing about what a full server does.
