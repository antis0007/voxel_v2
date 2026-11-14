# Generator 301: the earthport

Adds scale and real footings to the generator 65 anchor monument. Dormant: no
world default reaches 301. Generators 65 to 300 keep the monument unchanged.

The monument's datum, 256 m shaft, boarding berth, console, workbench and
smelter cells are not moved, so the app's fittings, braid socket and receiver
contracts hold without changes. Source: `src/meridian_terminal/earthport.rs`.

## Above the datum

- Three flying fins on bearings 30, 150 and 270 degrees, between buttresses
  and off the arrival axis. Each springs from a 26 m foot at 92 to 118 m,
  leans in over the wings and ties into the shaft's top collar at 226 m. Pale
  dressed stone with a dark outer rib; the soffit is 96 m up where it passes
  over the cargo berth.
- A crown cage on the last 20 m of the shaft: six prongs off the piers and a
  tie ring. The braid axis stays open.
- Two low vent stacks and hazard kerbs round every opening.

## Below the datum

The tether pulls up, so the load path goes down and out:

- Root column (6 m radius) from under the berth floor to a strain frame 36 to
  40 m down, inside an annular light well with two open slots beside the
  berth walk.
- Anchor vault: 112 m across, floor 64 m down, shallow dome 30 to 42 m above
  it. Six tendons run from the frame rim to anchor blocks on the floor.
- The six buttress caissons carry on down through the hall as its piers.
- Gallery 20 m above the floor, passing through each pier.
- Freight ramp: one turn of a 6 m road at 66 to 72 m radius, from an open cut
  on the terrace at bearing 45 degrees to a level landing and the vault door
  at 40 degrees. A second door reaches the gallery at 285 degrees.
- Service bearing 160 degrees: a stair from inside the control wing down to
  the ramp, and across the ramp a substation hall with four transformer bays
  under one bus.
- Sump under the strain frame; every cavity has a concrete lining so caves and
  aquifers do not open into the works.

Transformers, bus and tendons are plain blocks. They carry no power, load or
simulation state. The 79 m between the lowest possible datum and the world
floor limits the depth; nothing is placed lower than 74 m.

## Review

`cargo run --release -p engine_worldgen --example earthport_section -- out_dir [generator] [seed] [hills|planet]`
writes four sections and six plans as PPM and prints the member census and
sampler cost. Hills seed 74, 1 m voxels, interleaved best of three:

| Voxels | Generator 300 | Generator 301 |
| --- | --- | --- |
| Below the datum, inside the site box | 10.9 ns | 19.8 ns |
| Above the datum, inside the site box | 24.2 ns | 31.3 ns |
| 1 km from the site | 7.6 ns | 3.3 ns |

Generator 301 rejects voxels outside the site box before the datum lookup, so
every chunk away from the elevator samples faster than before.

Not done: no in-game capture, no walk test, no 0.5 m or 0.25 m run. The fins
have no model cladding. Sections show other generators' caverns and rooms
crossing the ramp ring; the earthport wins those voxels and lines its side.
