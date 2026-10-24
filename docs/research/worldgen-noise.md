# Worldgen noise (Minecraft 1.18 routers, Terralith, Tectonic)
Source: Minecraft Wiki, "Terrain" (https://minecraft.wiki/w/Terrain, read); Minecraft 1.18 density-function and noise-router internals (Mojang, 2021, unverified: not fetched); Terralith (Modrinth/CurseForge project pages, 2.x era); Tectonic v3 (CurseForge/Modrinth pages, unverified version support).
Adopt: Layered large-scale noise fields (continentalness sets altitude, erosion sets roughness, peaks and valleys shape mountains and rivers, weirdness picks biome variants) combined through splines into a height field; biome selection from the same parameters.
Reject: Float density functions and 3D Perlin in the generator; Terralith-style biome-count sprawl; Tectonic's continent-scale size as a default.
Why: Spline and parameter fields can be quantized to integers with a versioned generator (CURRENT_GENERATOR_VERSION rules), and variable voxel scale needs the parameters in world-space metres.
