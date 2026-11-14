# Space elevator: composition replacement

The previous exchange made horizontal bridge spans its landmark and left the
elevator as a small mechanism beneath them. Adding more detail did not solve
that composition. Generator 65 replaces that layout rather than decorating it.

## Reference and plan

The user supplied Lighthouse as reference. The linked fan wiki could not be
fetched. Bandai Namco's [official OUR SCIENCE article](https://ace7.acecombat.jp/clm/)
describes a tower with six great supports, climber departure facilities and a
cable continuation beyond the tower. These are composition references, not
imported meshes, lore or a claim to reproduce its dimensions.

Hierarchy: approach → arrival court → anchor chamber → curved buttresses →
256 m shaft → continuous atmospheric braid. The six supports and shaft dominate;
all secondary roofs remain below ten metres. The low control wing and loading
berth face the anchor. The former overhead highway slabs and free trusses are
absent from the new plan.

The existing receiver can be restored at the wing console. Workshop and smelter
remain actual block interactions. Delivered stock controls filled crate presence;
the parked service vessel does not pretend to be an automated cargo system.
Earthworks and the approach grade join natural terrain columns. Existing saves
are retained through the generator gate; the redesigned base needs generator 65.

The braid's full existing path remains connected from ground to moving suspension
joint and upper anchorage. Its rendered bundle is wider in the new monument;
this is a visual bundled sleeve, not an increase in the physical fibre section
used by the reduced tether model. Axiom's supported atmospheric corridor remains
distinct from the reference's geostationary elevator.

## Multiplayer appearance

The existing shared animated remote-character model now carries four distinct
delver kits: survey hood/map roll, foundry apron/goggles, courier cape/satchel,
and tether-maintenance spool/canisters. Harnesses and connected enamel patches
make backs readable. Deterministic palettes/skin variants use player ID; no new
profile, network payload or equipment ability is implied. Native review uses
the same draw function as multiplayer, which proves presentation rather than
transport synchronization.

## Evidence

Builds, native captures and executable manifests: `target/elevator-monument/`.
No tests or hosted CI were added or run. Visual acceptance is separated from
walking, moving-platform and network-session acceptance.

The first launch attempt (`native-r1`) failed while a concurrent build held the
release executable; it contains no visual evidence. R2's ground approach showed
the new vertical hierarchy, but the first anchor close-up put the flying review
camera inside a foundation socket and is rejected. A corrected axial close-up
and better cladding coverage are included in the final build.

R2 verified reclamation, intact receiver, actual workshop/smelter block IDs,
commissioning, a 160 kg iron round trip and exact mid-flight save/reload. The
returned stock was collected and lift capacity restored to 77,513.016689 kg.
The reported ledger residual was zero at displayed precision. Time and input
materials were staged only by the disposable review fixture.
