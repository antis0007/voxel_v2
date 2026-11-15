use engine_core::VoxelScalePreset;

/// First shared default for Hills, Ridged and Planet.
pub const SHARED_SURFACE_GENERATOR: u32 = 321;
/// Shared regional civilization and Anchor outskirts.
pub const REGIONAL_CIVILIZATION_GENERATOR: u32 = 322;
pub const CURRENT_GENERATOR_VERSION: u32 = REGIONAL_CIVILIZATION_GENERATOR;
/// The starter workshop has a flooded supply gallery off its mine decline.
pub const WORKSHOP_SUPPLY_GENERATOR: u32 = 241;
/// Regional landmarks follow their habitats and farmhouses cluster in villages.
pub const CURATED_SURFACE_GENERATOR: u32 = 240;
/// Primary and weathered ore minerals share the established refining chains.
pub const ORE_MINERAL_GENERATOR: u32 = 235;
/// Connected ruin routes, grounded vegetation and purposeful salvage.
pub const ANCHOR_EXPLORATION_GENERATOR: u32 = 229;
/// Wild flora pack: water, shore, wild crop, desert, small tree and cave plants.
pub const WILD_FLORA_GENERATOR: u32 = 210;
/// Branch-attached foliage shaped by the tree's growth habit.
pub const DISTINCT_FOLIAGE_GENERATOR: u32 = 207;
/// Broad woodland crowns, shrub colonies and larger ground plants.
pub const CANOPY_UNDERSTORY_GENERATOR: u32 = 194;
/// Larger Anchor cities with zoned blocks and building-specific salvage.
pub const ANCHOR_CITY_GENERATOR: u32 = 190;
/// Anchor landmarks, street-level exploration routes and damaged skylines.
pub const ANCHOR_RUIN_ROUTE_GENERATOR: u32 = 200;
/// Terrain-following Anchor wards and street-connected transit landmarks.
pub const ANCHOR_TERRACE_GENERATOR: u32 = 216;
/// New aspen, cypress and acacia forms in their native habitats.
pub const WOODLAND_VARIETY_GENERATOR: u32 = 185;
/// Species-specific branching and crowns for large woodland trees.
pub const TREE_MORPHOLOGY_GENERATOR: u32 = 182;
/// Larger biome-specific forest canopies and old-growth trees.
pub const TALL_WOODLAND_GENERATOR: u32 = 180;
/// Localized bamboo groves and climate-weighted ground flora.
pub const SURFACE_FLORA_GENERATOR: u32 = 177;
/// Connected Anchor districts and rebuilt ruin architecture.
pub const REBUILT_RUINS_GENERATOR: u32 = 179;
/// Palms and figs, reachable canopy vines and shallow rainforest swales.
pub const RAINFOREST_FLOOR_GENERATOR: u32 = 169;
/// Fallen rainforest trunks in canopy gaps and on stable forest floors.
pub const RAINFOREST_DEADWOOD_GENERATOR: u32 = 168;
/// Amazonian rainforest canopy with distinct tall and wide emergent trees.
pub const AMAZONIAN_CANOPY_GENERATOR: u32 = 167;
/// Rainforest stand openings, fern colonies and leaf litter.
pub const RAINFOREST_UNDERSTORY_GENERATOR: u32 = 166;
/// Taller rainforest canopy and broad-crowned emergent trees.
pub const TALL_RAINFOREST_GENERATOR: u32 = 165;
/// Layered rainforest canopies and clustered undergrowth in new worlds.
pub const RAINFOREST_GENERATOR: u32 = 162;
pub const HILLS_CANE_GENERATOR: u32 = 149;
/// Adds the editable spillway beside Fernwake's orchard.
pub const FERNWAKE_SPILLWAY_GENERATOR: u32 = 153;
/// River valleys retain their physical relief in distant Planet sampling.
pub const WATERSHED_LOD_GENERATOR: u32 = 145;
/// Tree-only release of the leaf relief and branch-attached crown work.
/// Keep their original gates for previously authored higher-version worlds.
pub const TREE_DETAIL_GENERATOR: u32 = 127;
/// Recovered biome families, regional roads and abandoned districts.
pub const RECOVERED_SYSTEMS_GENERATOR: u32 = 144;
/// New customized planets share the standard Planet terrain pipeline.
/// CUSTOM_PLANET_VERSION (136) remains the minimum recipe format and legacy gate.
pub const CUSTOM_PLANET_GENERATOR_VERSION: u32 = PLANET_GENERATOR_VERSION;
/// Connected spherical tectonics, climate, drainage and local terrain.
pub const SEAMLESS_PLANET_GENERATOR: u32 = 142;
/// Player-built thresholds and the red, open-sky Hell terrain. Older realms
/// retain their climate, material distribution and saved portal addresses.
pub const HELL_PORTAL_GENERATOR: u32 = 143;
/// Shelves, fungal colonies and volcanic exposure for newly linked Hell realms.
pub const HELL_ECOLOGY_GENERATOR: u32 = 146;
/// Fungal Hell with explorable camp ruins.
pub const HELL_RUINS_GENERATOR: u32 = 147;
/// Ruins surveyed for dry ground and an open approach.
pub const HELL_SURVEYED_RUINS_GENERATOR: u32 = 148;
/// Hell terrain driven by its parent planet recipe.
pub const HELL_MIRROR_GENERATOR: u32 = 150;
/// Regional Hell terrain, ecology and hazards.
pub const HELL_BIOME_GENERATOR: u32 = 159;
/// Plate-driven coasts and bathymetry for new Planet worlds.
pub const COASTAL_PLANET_GENERATOR: u32 = 151;
/// Conserved hillslope transport alongside runoff incision.
pub const TALUS_PLANET_GENERATOR: u32 = 152;
/// Sedimentary beds shape inland slopes and their exposed rock.
pub const BEDDING_PLANET_GENERATOR: u32 = 154;
/// Discharge-scaled rivers and depositional deltas on the spherical watershed.
pub const ALLUVIAL_PLANET_GENERATOR: u32 = 155;
/// Reconstructs solved land heights with shared vertex slopes.
pub const CONTINUOUS_RELIEF_PLANET_GENERATOR: u32 = 156;
/// Erodes joints into gullies and forms talus at the foot of sedimentary cliffs.
pub const SCREE_PLANET_GENERATOR: u32 = 157;
/// Seasonal soil over shallow ground ice in cold, wet sediment.
pub const PERMAFROST_PLANET_GENERATOR: u32 = 158;
/// Evolving river mouths with conservative shallow-water and sediment transport.
pub const MORPHODYNAMIC_PLANET_GENERATOR: u32 = 160;
/// Age-progressive oceanic shields and erosion of volcanic islands.
pub const HOTSPOT_PLANET_GENERATOR: u32 = 161;
/// Compose coastal erosion and deposits with the native shoreline relief.
pub const COASTAL_RELIEF_PLANET_GENERATOR: u32 = 163;
/// Collapsed summits and lava fields on active oceanic shields.
pub const VOLCANIC_LANDFORM_PLANET_GENERATOR: u32 = 164;
/// Tidal channels, point bars and floodplain benches on sediment-fed shelves.
pub const ESTUARY_PLANET_GENERATOR: u32 = 175;
/// Broader, curved approaches where the solved river meets the sea.
pub const COASTAL_APPROACH_PLANET_GENERATOR: u32 = 178;
/// Eroded composite cones, collapse scars and volcanic aprons on Planet worlds.
pub const STRATOVOLCANO_PLANET_GENERATOR: u32 = 181;
/// Time-evolved spherical crust for newly generated planets.
pub const CONTINENTAL_DRIFT_GENERATOR: u32 = 183;
/// Continuous bed elevations across the open-ocean shoreline.
pub const CONTINUOUS_COAST_PLANET_GENERATOR: u32 = 191;
/// Local runoff incision and talus on volcanic edifices.
pub const VOLCANIC_FLANK_EROSION_GENERATOR: u32 = 192;
/// Finite placer deposits in spherical river beds and low gravel benches.
pub const PLACER_PLANET_GENERATOR: u32 = 196;
/// Valley-guided rivers with pools, point bars and unequal banks.
pub const RIVER_MORPHOLOGY_GENERATOR: u32 = 217;
/// Tilted collapse blocks and nested scarps on active hotspot shields.
pub const CALDERA_FAULTS_GENERATOR: u32 = 218;
/// Tidal river mouths and suspended sediment from the coastal hydraulic solve.
pub const COASTAL_DELTA_GENERATOR: u32 = 230;
/// Inland gravel transport and bank retreat in broad alluvial valleys.
pub const MOBILE_RIVERBED_GENERATOR: u32 = 231;
/// Surveyed regional roads between Anchor district gateways.
pub const ANCHOR_ROAD_GENERATOR: u32 = 233;
/// Wider lowland floodplains, with migration and neck capture over a whole bend.
pub const FLOODPLAIN_MEANDER_GENERATOR: u32 = 234;
pub const PLANET_FREIGHT_GENERATOR: u32 = 236;
/// Floodplain relief from successive positions of the migrating bank.
pub const FLOODPLAIN_SCROLL_GENERATOR: u32 = 237;
/// Flood abrasion of exposed rock and buried coal-bearing sedimentary beds.
pub const BEDROCK_RIVER_GENERATOR: u32 = 238;
/// Terrain grading and building details around planetary freight sites.
pub const PLANET_FREIGHT_SITE_GENERATOR: u32 = 239;
/// Regional settlement history follows solved planetary terrain.
pub const CIVILIZATION_GENERATOR: u32 = 242;
/// The planetary freight site includes an independent service crossing.
pub const PLANET_FREIGHT_DETOUR_GENERATOR: u32 = 243;
/// Dormant: district viaducts aim along a line toward the braid exchange, break only at a found cause.
pub const ANCHOR_LINE_GENERATOR: u32 = 250;
/// Tectonic terrain with refined erosion and routed groundwater.
pub const WATERSHED_EROSION_GENERATOR: u32 = 272;
/// Climate strata, eruptions and fault history coupled to plate motion.
pub const GEOLOGICAL_HISTORY_GENERATOR: u32 = 280;
pub const EVOLVED_WATERSHEDS_GENERATOR: u32 = 281;
/// Separate plate, event and sediment clocks; subsiding volcanic reef platforms.
pub const RESOLVED_HISTORY_GENERATOR: u32 = 282;
/// Historical sea-level cycles and offshore-only bathymetry.
pub const CHANGING_SEAS_GENERATOR: u32 = 283;
/// Momentum-routed tributaries and continuous main-stem reaches.
pub const BRANCHING_WATERSHEDS_GENERATOR: u32 = 284;
/// Climate-driven ice history, terrain-linked woodland and connected travel routes.
pub const LANDSCAPE_CYCLES_GENERATOR: u32 = 286;
/// Extended continental and climate evolution for newly created planets.
pub const DEEP_TIME_GENERATOR: u32 = 287;
/// Deep-time history with a fractal, rock-dependent shoreline planform.
pub const FRACTAL_COAST_GENERATOR: u32 = 288;
/// Generator 288 plus wave-driven longshore shorelines and rock-grain coasts.
pub const PROCESS_COAST_GENERATOR: u32 = 289;
/// Generator 288 coasts plus clean riverbeds, ponds and sandier shores.
pub const CLEAN_RIVERBED_GENERATOR: u32 = 290;
/// Generator 290 terrain plus ravines, fissure swarms and sinkholes (opt-in).
pub const GROUND_BREAK_GENERATOR: u32 = 293;
/// Generator 290 terrain plus fine relief, shore forms, climate plants and plant stamps (opt-in).
pub const LIVING_SURFACE_GENERATOR: u32 = 295;
/// Generator 290 terrain plus lowland rivers that sit in their plain: levees, backswamps, marsh (opt-in).
pub const LOWLAND_RIVER_GENERATOR: u32 = 296;
/// Generator 290 terrain plus faulted rift valleys, the 293 ground breaks and the 295 living surface (opt-in).
pub const RIFT_VALLEY_GENERATOR: u32 = 297;
/// Generator 297 terrain plus basalt dikes: rift-parallel and volcano-radial swarms that cut the strata (opt-in).
pub const DIKE_SWARM_GENERATOR: u32 = 298;
/// Anchor city outskirts: lanes that fray out past the kerb and lots thinning with distance (opt-in).
pub const ANCHOR_OUTSKIRTS_GENERATOR: u32 = 299;
/// Generator 298 terrain and 296 lowland rivers plus slope-aligned gullies, rock benches and terracettes (opt-in).
pub const CARVED_SURFACE_GENERATOR: u32 = 300;
/// Dungeon tilework, false-wall caches and shared mechanical fittings.
pub const DUNGEON_MECHANISMS_GENERATOR: u32 = 310;
/// Generator 300 terrain plus rock relief: solved bed resistance raises risers and caprock benches (opt-in).
pub const ROCK_RELIEF_GENERATOR: u32 = 304;
pub const PLANET_GENERATOR_VERSION: u32 = CURRENT_GENERATOR_VERSION;
pub const WORLD_MIN_Y_METERS: f32 = -16.0;
pub const WORLD_MAX_Y_METERS: f32 = 512.0;
pub const WATER_LEVEL_METERS: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Deserialize)]
pub enum WorldType {
    Superflat,
    /// Legacy surface terrain retained for existing saves and generation reviews.
    Hills,
    /// Legacy highlands retained for existing saves and generation reviews.
    Ridged,
    /// A planet's region in the shared Nether dimension; seed is the parent planet seed.
    Nether,
    Planet,
    /// Maze shell. New worlds start on the expedition; older generators keep their shape.
    Manyfold,
    /// Hanging precinct. Charted arches leave for the Nether and the Manyfold.
    Hall,
    /// A pocket dimension entered through a placed gate; the kind picks its
    /// generator. Save code 7 + kind index. No historical world is a realm, so
    /// the first realm generator needs no version gate; later changes do.
    Realm(RealmKind),
}

/// Pocket dimensions sharing one crossing and one save slot scheme.
///
/// Adding a realm (each step is one arm or one file; the compiler lists the
/// exhaustive matches that need an arm):
/// 1. A variant here, appended, with `code`, `title`, `short`, `has_sky`.
/// 2. A pure generator `engine_worldgen::<realm>` with `column`, `block`,
///    `spawn`, and its dispatch lines in `engine_worldgen::sample_column_uncached`
///    and `sample_block_in_world`. No gate is needed for a realm's first layout.
/// 3. A gate block and its arm in `app::world::lightless_gate::{gate_realm, file, arrival}`.
/// 4. Presentation: `portals::realm::realm_lighting`, motes in
///    `mesh::models::realm_air::families`, spawns via `habitat::Context::at`.
/// 5. Optional life: a module like `app::world::lightless_life` (sidecar state,
///    seeded steps, away-time replay from the day clock).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Deserialize)]
pub enum RealmKind {
    /// Lightless caverns under every world. Dense ore; the dark is lethal.
    Lightless,
    /// The Gilded Cave: palace gallery, coin cavern and pillar grotto.
    Gilded,
    /// The Highfold: islands hanging over a shallow sea, reached from high open ground.
    Highfold,
    /// The Gloaming: a dusk forest of giant trees, reached from deep woods.
    Gloaming,
}

impl RealmKind {
    pub const ALL: [RealmKind; 4] = [RealmKind::Lightless, RealmKind::Gilded, RealmKind::Highfold, RealmKind::Gloaming];
    /// Save and checkpoint code; append new kinds, never reorder.
    pub const fn code(self) -> u8 {
        match self {
            RealmKind::Lightless => 7,
            RealmKind::Gilded => 8,
            RealmKind::Highfold => 9,
            RealmKind::Gloaming => 10,
        }
    }
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == code)
    }
    /// Player-visible name, as an arrival or a map title shows it.
    pub const fn title(self) -> &'static str {
        match self {
            RealmKind::Lightless => "The Lightless",
            RealmKind::Gilded => "The Gilded Cave",
            RealmKind::Highfold => "The Highfold",
            RealmKind::Gloaming => "The Gloaming",
        }
    }
    /// One-word label for world lists.
    pub const fn short(self) -> &'static str {
        match self {
            RealmKind::Lightless => "Lightless",
            RealmKind::Gilded => "Gilded Cave",
            RealmKind::Highfold => "Highfold",
            RealmKind::Gloaming => "Gloaming",
        }
    }
    /// Open sky: daylight, weather-free air and surface streaming bands.
    pub const fn has_sky(self) -> bool {
        matches!(self, RealmKind::Highfold | RealmKind::Gloaming)
    }
}

impl WorldType {
    /// One byte shared by saves, checkpoints, cache keys and preferences.
    pub const fn code(self) -> u8 {
        match self {
            WorldType::Superflat => 0,
            WorldType::Hills => 1,
            WorldType::Ridged => 2,
            WorldType::Nether => 3,
            WorldType::Planet => 4,
            WorldType::Manyfold => 5,
            WorldType::Hall => 6,
            WorldType::Realm(kind) => kind.code(),
        }
    }
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => WorldType::Superflat,
            1 => WorldType::Hills,
            2 => WorldType::Ridged,
            3 => WorldType::Nether,
            4 => WorldType::Planet,
            5 => WorldType::Manyfold,
            6 => WorldType::Hall,
            c => WorldType::Realm(RealmKind::from_code(c)?),
        })
    }
    pub fn realm(self) -> Option<RealmKind> {
        match self {
            WorldType::Realm(kind) => Some(kind),
            _ => None,
        }
    }
    /// Closed worlds with their own column generator and no sky.
    pub fn is_interior(self) -> bool {
        match self {
            WorldType::Hall => true,
            WorldType::Realm(kind) => !kind.has_sky(),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldSampleConfig {
    pub generator_version: u32,
    pub seed: u32,
    pub world_type: WorldType,
    pub voxel_scale: VoxelScalePreset,
    pub planet: Option<crate::PlanetSettings>,
}

impl WorldSampleConfig {
    /// Both ordinary planets and new Hell realms use the cube-sphere chart.
    pub fn is_planet(self) -> bool {
        self.world_type == WorldType::Planet
            || self.is_hell()
    }
    /// Dedicated dimension recipe. Later ordinary-world generators must not
    /// silently change the layout or identity of their historical Nether links.
    pub fn is_hell(self) -> bool {
        self.world_type == WorldType::Nether && Self::is_hell_generator(self.generator_version)
    }
    pub fn is_hell_generator(version: u32) -> bool {
        matches!(version, HELL_PORTAL_GENERATOR | HELL_ECOLOGY_GENERATOR | HELL_RUINS_GENERATOR | HELL_SURVEYED_RUINS_GENERATOR | HELL_MIRROR_GENERATOR | HELL_BIOME_GENERATOR)
    }
    pub fn hell_ruins(self) -> bool {
        self.world_type == WorldType::Nether && matches!(self.generator_version, HELL_RUINS_GENERATOR | HELL_SURVEYED_RUINS_GENERATOR | HELL_MIRROR_GENERATOR | HELL_BIOME_GENERATOR)
    }
    pub fn hell_ecology(self) -> bool {
        self.world_type == WorldType::Nether && matches!(self.generator_version, HELL_ECOLOGY_GENERATOR | HELL_RUINS_GENERATOR | HELL_SURVEYED_RUINS_GENERATOR | HELL_MIRROR_GENERATOR | HELL_BIOME_GENERATOR)
    }
    pub fn hell_mirror(self) -> bool {
        self.world_type == WorldType::Nether && matches!(self.generator_version, HELL_MIRROR_GENERATOR | HELL_BIOME_GENERATOR)
    }

    /// Whether this generator can persist a planet recipe.
    pub fn supports_planet_recipe(self) -> bool {
        self.hell_mirror() || (self.world_type == WorldType::Planet && self.generator_version >= crate::CUSTOM_PLANET_VERSION)
    }
    pub fn planet_settings(self) -> Option<crate::PlanetSettings> {
        self.supports_planet_recipe().then_some(self.planet).flatten()
    }

    pub fn planet_fingerprint(self) -> u64 {
        self.planet_settings().map_or(0, crate::PlanetSettings::fingerprint)
    }

    pub fn civilization_enabled(self) -> bool {
        self.planet_settings().is_none_or(|settings| settings.civilization == crate::CivilizationMode::Living)
    }

    pub fn voxel_meters(self) -> f32 {
        self.voxel_scale.meters()
    }

    pub fn inv_voxel_meters(self) -> f32 {
        self.voxel_meters().recip()
    }

    pub fn world_x(self, x: i32) -> f32 {
        x as f32 * self.voxel_meters()
    }

    pub fn world_y(self, y: i32) -> f32 {
        y as f32 * self.voxel_meters()
    }

    pub fn world_z(self, z: i32) -> f32 {
        z as f32 * self.voxel_meters()
    }

    pub fn blocks_for_height(self, meters: f32) -> i32 {
        (meters * self.inv_voxel_meters()).round() as i32
    }

    pub fn blocks_for_span(self, meters: f32) -> i32 {
        ((meters * self.inv_voxel_meters()).ceil() as i32).max(1)
    }

    pub fn min_world_y(self) -> i32 {
        (if self.generator_version >= 2 {
            -64.0
        } else {
            WORLD_MIN_Y_METERS
        } * self.inv_voxel_meters())
        .floor() as i32
    }

    pub fn max_world_y(self) -> i32 {
        (WORLD_MAX_Y_METERS * self.inv_voxel_meters()).ceil() as i32
    }

    pub fn water_level(self) -> i32 {
        self.blocks_for_height(WATER_LEVEL_METERS)
    }

    pub fn clamp_height(self, y: i32) -> i32 {
        y.clamp(self.min_world_y() + 4, self.max_world_y() - 3)
    }
}
