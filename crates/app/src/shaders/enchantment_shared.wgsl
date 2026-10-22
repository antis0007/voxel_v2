// Surface-bound magic on the world's existing 5 cm lattice. No extra pass,
// texture, displacement or alpha layer; subpixel decoration converges to zero.
fn enchantment_face_uv(p_m: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let a = abs(normal);
    if a.y >= a.x && a.y >= a.z { return p_m.xz; }
    if a.x >= a.z { return p_m.zy; }
    return p_m.xy;
}

fn enchantment_band(coordinate: f32, phase: f32, width: f32) -> f32 {
    // Group existing texels into wide diagonal ribbons, without a second grid.
    let distance = abs(fract((coordinate + phase) / 48.0) - 0.5) * 48.0;
    return 1.0 - smoothstep(width, width + 1.5, distance);
}

fn enchantment_bands(uv_m: vec2<f32>, footprint_m: f32, seconds: f32) -> vec2<f32> {
    let resolved = 1.0 - smoothstep(0.65, 2.0, footprint_m * PIXEL_RES);
    if resolved <= 0.0 || light.magic[3].x <= 0.0 { return vec2<f32>(0.0); }
    let texel = floor(uv_m * PIXEL_RES) + vec2<f32>(0.5);
    // Incommensurate, slow sweeps keep the finish alive without a synchronized
    // whole-object flash. Brightness moves smoothly across fixed pixel clusters.
    let direction=vec2<f32>(cos(light.magic[4].y),sin(light.magic[4].y));
    let density=light.magic[4].x;
    let time=seconds*light.magic[3].y;
    let first=enchantment_band(dot(texel,direction)*1.25*density,time*4.2,2.0*light.magic[3].z);
    let second=enchantment_band(dot(texel,vec2<f32>(-direction.y,direction.x))*1.2*density,-time*2.7+17.0,light.magic[3].z);
    let seed=surface_hash(texel);
    let sparkle=step(0.985-light.magic[4].z*0.055,seed)*pow(max(0.0,sin(time*1.8+seed*31.0)),12.0)*light.magic[4].z;
    return (vec2<f32>(first,second)+vec2<f32>(sparkle*0.35)) * resolved * light.magic[3].x;
}

// Resonant crystal uses only the first ribbon. Keep its material path from
// evaluating an unused second diagonal, and skip both once the grid is unresolved.
fn enchantment_primary(uv_m: vec2<f32>, footprint_m: f32, seconds: f32) -> f32 {
    let resolved = 1.0 - smoothstep(0.65, 2.0, footprint_m * PIXEL_RES);
    if resolved <= 0.0 || light.magic[3].x <= 0.0 { return 0.0; }
    let texel = floor(uv_m * PIXEL_RES) + vec2<f32>(0.5);
    let direction=vec2<f32>(cos(light.magic[4].y),sin(light.magic[4].y));
    return enchantment_band(dot(texel,direction)*1.25*light.magic[4].x,seconds*4.2*light.magic[3].y,2.0*light.magic[3].z)*resolved*light.magic[3].x;
}

fn enchantment_violet(bands: vec2<f32>) -> vec3<f32> {
    // Linear-light violet and cool blue. The crossing is briefly silver; the
    // untouched surface keeps its original wood, leather or metal identity.
    let base=light.magic[0].rgb*bands.x+light.magic[1].rgb*bands.y;
    let fringe=abs(bands.x-bands.y)*light.magic[3].w*0.25;
    return mix(base,light.magic[2].rgb*max(bands.x,bands.y),fringe)
        + mix(light.magic[0].rgb,light.magic[2].rgb,0.5)*bands.x*bands.y*0.30;
}
