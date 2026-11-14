# Generator 93: regional biome variety

Fresh worlds retain generator 92's compact terrain and clustered settlements.
The ecology layer now strengthens the solved temperature and rainfall contrast
and adds rotated, smoothly warped regional variation at 360–480 m scales.
These are input field scales, not guaranteed biome sizes or discovery distances.
Physical erosion, river discharge, temperature in Celsius and permafrost retain
their existing solve. The adjusted ecological fields drive biome selection,
surface policy and existing biome-specific vegetation forms.

Previously all terrain above 124 m became alpine. Generator 93 keeps warm
wooded uplands until 205 m, with a lower cold treeline above 170 m. Existing
slope-based rock exposure still applies independently.

Fens can occupy very wet, flat inland ground from 16.5 to 90 m instead of only
16.5 to 24 m. A coherent 150 m patch field breaks up wet flats; it does not
create standing water. Shoreline marsh rules remain in place.

Exact and horizon terrain use the same ecological transform. Distant tree
admission uses the same sea-level normalization and altitude lapse scale.
Wetland selection avoids sediment because the horizon uses a sediment estimate.

Offline worldgen compile check passed. No tests were added or run. Biome
coverage across seeds and native visual/exploration acceptance remain unmeasured.
The executable has not been rebuilt. Older saves keep their generator; create a
fresh world after rebuilding to use these rules.
