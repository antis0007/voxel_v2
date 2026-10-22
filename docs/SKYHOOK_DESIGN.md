# Skyhooks, elevators and orbital spectacle

Status: design + physics core (`src/world/orbital/skyhook.rs`, tested). Rendering and events not built yet.

## Review of what exists (2026-10-22)

| Piece | Finding |
| --- | --- |
| Meridian Lift (`world/orbital.rs`) | A winch on a fixed anchorage 18 km above the site, swinging the carriage on an 1800 m sideways arc. It is not a skyhook and doesn't claim to be. Keep it as the early-game "old commons" lift. |
| Megastructures (`orbital/megastructures.rs`) | Real two-body circular orbits, f64 observer frame, eclipse test: good. **Bug in feel:** `seconds()` maps one game day (480 s real) to 86 400 s, so 420 km LEO orbits once every ~31 real seconds and a pass lasts ~3 s. That reads as a streak, not a setpiece. |
| Fix | Put orbits on an **orbital clock of real seconds** (`skyhook::clock`): LEO passes take 5–8 min on screen, which matches what you would see in reality. The sun stays on the fast day. Only eclipse timing goes out of step with the sun, and that is unnoticeable. Move `megastructures::contacts` onto this clock after an A/B capture. |

## Physics (implemented)

A rotovator: the hub sits on a circular prograde equatorial orbit, and the tether spins in the orbit plane in the same sense.

- Spin `Ω = (n·r_hub − ω_earth·r_tip) / L`, so at the bottom of each turn the tip's ground-relative velocity is **zero** (the cusp of a cycloid). The tip comes straight down, stops, and goes straight back up.
- Tether length is bisected so that a whole number of turns fit in one synodic pass. The cusp lands on the **same home port on every pass**. Every other cusp lands somewhere else on the planet, so the hook is visibly working the world even when it isn't your event.
- Tests check that at three successive passes the tip is at its cusp altitude, moving at less than 1 m/s over the ground, and over the port to within 1e-5 rad.

| Size | Tether | Turn | Tip load | Pass | Role |
| --- | --- | --- | --- | --- | --- |
| Needle (courier) | ~160 km | ~2.4 min | ~34 g | ~95 min | Cargo pods only. Port raids. |
| Talon (freight) | ~420 km | ~6.5 min | ~12 g | ~100 min | Containers, machine crates. |
| Ark (two-stage) | ~1150 km | ~21 min | ~4 g | ~120 min | Districts, buildings, people. |

**Two stages.** The rotovator tip never goes below 70–90 km; real physics carries it that far. Stage two is the magitech part: a **drop line** (a braid of the old commons, or a resonance-bound "anchor shard") pays out 70 km from the tip and catches the payload during the dwell. Reeling the line in and out stretches the instantaneous cusp into a playable 30–90 s window. Diegetically this is why the dwell is short and deadly, and why missing it means waiting a whole pass.

## Spectacle (render plan)

- **Never a sky card.** Use the same world-space `strut`/`part` path as the megastructures, with the f64→local projection from `skyhook::contacts`. Draw the tether with `tether()` for coverage, so a 20 m cable at 300 km stays a solid one-pixel line, not dots.
- **The clock hand.** The hub crosses the sky in minutes while the arm sweeps like a second hand. At sunset the tether catches light above the terminator: a bright scythe over a dark ground (sunlight test per segment along `along(t, s)`).
- **Descent.** In the last 60 s the tip grows from a star to a falling city of rigging. A sonic boom arrives late (distance / 343 s). The clouds part: write into the existing cloud-density field along the drop line, and reuse `weather_volume` shafts.
- **Lift.** The payload volume (box or sphere of chunks) is copied to a `Model` and replaced by air, with dust and debris particles at the cut. The model rides `along(t, 1)` upward with a slow tumble damped by the line. Captures from it show the ground hole receding.
- **Elevator base (Diamond Age / Blame!).** A geostationary elevator is static: a hairline from the horizon to the zenith, with climbers as slow lights (about 200 km/h, so you see them crawl). Its base is an arcology. The **Feed** (Diamond Age) runs matter lines from it outward. This becomes the late-game hub that skyhooks feed.

## Gameplay integration

**Calling one** uses the existing channels (`orbital::Channel::{Radio, Resonance}`):
- *Radio:* book through the receiver ledger. It costs uplink charge and gives a guaranteed window: the next port cusp, shown in the UI.
- *Resonance (magic):* pull the hook off resonance. It works from any site, but retuning the tether bleeds orbit (`hub_radius` decays). Hooks that are pulled too often become **derelict**, and their cusps start striking random places. That gives an FTL-style "fleet is coming" pressure clock.

**Event families** (RimWorld storyteller: seeded, scaled to wealth and tier):
1. *Port lift (Needle/Talon):* a guarded pickup facility. You defend or steal the cargo pod for one dwell window. Bosses ride down on the line. Late-game magical enemies (void-tethered wraiths) spawn from the drop line itself.
2. *Harvest (Ark, hostile):* an enemy faction books a lift on *your* base. The countdown appears in the radio panel one pass ahead. You can cut the drop line (a massive structure with a damage model), jam the resonance, or let it go and lose the district. Cutting the line drops a falling, burning braid, Noita-style: the debris carves terrain.
3. *Exodus (Ark, friendly):* you pay to lift your own district into orbit, which unlocks the orbital layer and the megastructures as destinations.
4. *Stray cusp:* a derelict hook tears through the countryside. It leaves a fresh trench and scattered loot, and it repeats every pass until someone fixes it.

**Physics in play (Noita):** everything inside the lifted volume keeps its blocks, fluids and entities. Fluids spill from the open edges during the climb. Anything left half inside the cut volume shears off. The hole left behind fills with rain and water from the simulation.

**Save format:** the hooks are a pure function of seed and clock, so they add nothing to the save. Bookings and derelict state should extend `orbital::State` behind the next save-version bump. Lifted volumes need their own record, which is a version bump too, so plan them together.

## Next steps
1. A/B capture, then move megastructures onto `skyhook::clock`.
2. Render the hub, tether and tip (world-space, coverage-tethered) and capture a dawn pass.
3. Port-lift event with a booking via the radio, behind a save bump.
