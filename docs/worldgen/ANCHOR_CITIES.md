# Anchor city generation

[Anchor Studio](ANCHOR_STUDIO.md) is the visual editor for building designs and their placement in a review district.

The city survey tries a 440 × 320 m footprint before falling back to a compact district. Expanded cities have a through avenue, five cross streets, six secondary streets and four approaches to natural ground. Their layout reserves the transit structure, freight or utility landmarks, and streets before placing buildings. Smaller streets bend through the outer wards. The median of five terrain readings sets each ward's grade, and the outside cross streets absorb the transition. Parcel placement, entrances, foundations and salvage all use the surveyed grade.

Office districts put a service concourse under the main span. Players can approach along the low route or use a stair from the upper street. Some crossings have failed, while others still carry the streets across the cut. Port districts have shipping-container yards, a two-level freight hall where a parcel fits, and an open carriage wreck with a deck entrance. The elevated transit stops attach to service stairs and carry their own weathered boarding shelters.

Room and floor plans follow the buildings' former uses. Clinics and offices open into reception, workshops have a broad work floor and back rooms, and housing has compact rooms off a shared passage. Damage can cut through outer rooms while leaving the stair core standing. Salvage uses the existing persistent container inventory. Its category follows the room or landmark, such as medical stock in clinics, freight in sorting bays and survey equipment at relays.

## References used

- Tu Bui's `tu-bui-dmz-support-beam.jpg`, `tu-bui-switchback-overall.jpg` and `tu-bui-aftermath.jpg` informed the routes under large structures, grade changes and localized collapses.
- Jung Park's `jung-park-jung-park-11.jpg` and `jung-park-boomtown.jpg` informed the readable hull approach, exposed ribs and service access at the foot of the megastructure.
- James Paick's `james-paick-titanfall-1.jpg` informed the contrast between small roofed homes and tall public structures. Luke Aegis's `luke-aegis-ln62-nemesis-headquarters.jpg` informed the use of restrained color at access points.
- For comparison, the industrial waterfront and the narrower street show two different relationships between utility and street frontage. [Maciej Rebisz's Arasaka Waterfront](https://mac.artstation.com/projects/ZlgdvG) and [Hiroshi Sakakibara's Little China Street](https://hiroshisakakibara.artstation.com/projects/oAlKlq).
- [Architecture materials](../design/ARCHITECTURE_MATERIALS_20261026.md), [world inspirations](../design/WORLD_INSPIRATIONS_20261026.md) and [composed vistas](COMPOSED_VISTAS.md) cover material legibility, physical approaches and review.

## Review

Seed `424242` has a reference office district around `(-49, -1002)` and a port around `(-74, 2019)` in Hills. `Review-Places.ps1` supports `anchor:office`, `anchor:port`, `anchor:concourse`, `anchor:descent`, `anchor:wreck` and `anchor:terminal`. Use the same seed, generator, voxel scale and time of day when comparing render changes. Camera review shows the generated geometry. Traversal and loot interactions require player input.

At 1 m voxel scale the sampled descent had continuous support in the center and two metres to either side, with seven one-block drops to the concourse. The office and port previews still show a regular street grid from above. The stronger distinctions are the height changes, frontage and purpose of each landmark at street level.

The Hills sample produced 45 planned salvage caches in the office district and 67 in the port. Native captures of the concourse and wreck used a hashed copy of the workspace executable. The captures and their manifests are in `target/anchor-terrace-216-review/native`. The same review folder has sampled block geometry for the city layouts, waterworks, foundry and conservatory.

## Exploration pass

Generator 229 connects two marked stairs through the office relay. The shared tile color follows the concourse between them. The wreck has a street approach at the torn end and a loading apron at the cargo door. The door cuts through the interior rib as well as the outer skin.

The city checks tree footprints against its deck and approaches before sampling foliage. Plaza trees grow from planter beds as a single connected shape. The concourse uses moss and retaining walls to frame the route, and the crash debris stays near the point of impact. Landmark salvage follows the routes, while the public plaza serves as open space.

On Hills seed `424242` at 1 m voxel scale, the generated block samples show supported tree stems, a clear concourse centerline, and continuous support with two clear blocks above it across three lanes on each stair and at the cargo exit. The geometry previews and route profiles are in `target/anchor-exploration-229-review`.
