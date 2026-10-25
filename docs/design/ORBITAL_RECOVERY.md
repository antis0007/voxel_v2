# Orbital pass forecasts and relay recovery

The radio ledger now forecasts the next qualifying pass of each of the four
orbital installations. Windows use centre elevation above 15 degrees and world
minutes, with estimated duration and peak elevation. The search covers one world
day at one-minute intervals. Work is spread across updates (64 ephemeris
evaluations maximum per update), cached for five world minutes and invalidated
on a changed observer chart cell. This is geometric prediction; weather can
still prevent a live uplink. It does not retime or respawn the structures.

## Play loop

1. Complete receiver commissioning ML-C01.
2. Build a level-4 directional dish near the receiver. Learn directional
   tracking through the existing aimed service handshake.
3. Choose **Track derelict relay**. The dish follows the moving relay while its
   centre is above 15 degrees, then parks until it can acquire the next pass.
   **Point dish** on a service corridor restores normal service tracking.
4. Consult **Pass forecast**. During a relay window, operate the intact receiver
   within 6 m and the dish within 8 m. The dish requires clear overhead space;
   severe weather may require waiting or the level-5 phased array.
5. **Survey relay & order recovery** pays 2 steel and 1 ancient circuit.
   The Commons recovery service returns 6 ancient circuits and 2 crystals
   through the existing scheduled receiver capsule. This is one recovery per
   world, not a repeatable material source or simulated boarding expedition.

The action backend rechecks proximity, equipment, tracking knowledge, dish mode,
clearance, weather, actual current relay elevation and cost. Forecast/UI values
cannot authorize payment. Ordinary portable/service radio range is unchanged;
the calibrated dish has a separate orbital-survey capability.

## Persistence and inventory

Save version 30 extends the existing receiver stage word without adding payload
fields: 0 uncommissioned, 1 commissioning paid, 2 commissioning claimed,
3 relay recovery paid, 4 both contracts claimed. Older saves retain their original
stages. Versions below 30 reject the new stages. The save header and writer
version both identify 30. No new machine or resource IDs are added.

Stage transitions only advance. Payment records a fixed due date; leaving the
area, missing a descent, reloading, or restoring a dismantled console cannot
restart the entitlement. Both cargo types use a cloned inventory transaction:
all stacks must fit before the live inventory and completion stage change.
Collection at stage 4 and repeat ordering at stages 3/4 do nothing. Pending
recovery uses the same persistent capsule presentation as commissioning.

Dish relay-tracking mode reuses the saved empty service target together with
the tracking lesson; a selected corridor remains an explicit service target.
Pass forecast caches are presentation-only and are not serialized.

The ledger view is refreshed only while inventory page 8 is visible. Forecasts
use geometric ephemerides without weather/solar lighting, with at most 32 samples
and a 250 microsecond cooperative work budget per visible frame. An unfinished
chart survives refresh-bucket changes rather than restarting on slow frames.

Implementation lives in `crates/app/src/world/orbital/{passes,receiver,radio}.rs`,
the orbital ledger UI, and the dish model. No tests or native gameplay scenario
were added or run for this increment. Local compilation/build status is recorded
under `target/orbital-recovery-*-20261024.log`.
