// Palette-bound pixel bevel; the rainbow uses the CPU pixel painter.
struct Prism { size: vec4<f32>, light: vec4<f32>, detail: vec4<f32>, padding: vec4<f32> }
@group(0) @binding(0) var<uniform> prism: Prism;
struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(@builtin(vertex_index) index: u32) -> Vertex {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return Vertex(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}
fn grain(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn flame(position: vec2<f32>, extent: vec2<f32>) -> vec4<f32> {
    let p = floor(position / 2.0) * 2.0;
    let time = floor(prism.size.z * 12.0) / 12.0;
    let center = 23.0;
    // The flame stands on the icon's foot (16 px glyph centred in the control)
    // and reaches the top edge; the glyph is painted over its root.
    let base = floor((extent.y * 0.5 + 10.0) / 2.0) * 2.0;
    let height = (base - p.y) / max(base - 2.0, 1.0);
    let curl = sin(height * 8.0 - time * 3.0) * 2.0 + sin(time * 2.2) * height * 3.0;
    let width = (1.0 - height * height) * 8.0;
    let noise = grain(floor(p / 2.0) + vec2<f32>(0.0, floor(time * 6.0)));
    var coverage = 0.0;
    var color = prism.padding.rgb;
    // Heat wash: a banded ember glow spreading right from the icon, with a
    // flickering ragged front and rising shimmer, quantised to four shades.
    let reach = extent.x * (0.55 + 0.08 * sin(time * 2.0 + p.y * 0.15));
    let dist = (p.x - center) / max(reach, 1.0);
    let rise = 0.5 + 0.5 * sin(p.x * 0.09 - time * 2.6 + p.y * 0.2);
    let wash = (1.0 - dist) * (0.55 + 0.45 * rise) - noise * 0.22
        + (1.0 - p.y / extent.y) * 0.18;
    if dist > -0.2 && dist < 1.0 && wash > 0.12 {
        let band = floor(clamp(wash, 0.0, 1.0) * 4.0) / 4.0;
        color = mix(prism.padding.rgb, prism.light.rgb, 0.25 + band * 0.55);
        color = mix(color, prism.detail.yzw, (1.0 - band) * 0.35);
        // Fade the wash to nothing at the button's lower edge so it never ends on a hard line.
        let fade = clamp((extent.y - p.y) / (extent.y * 0.45), 0.0, 1.0);
        coverage = (0.10 + band * 0.30) * fade;
    }
    if height >= 0.0 && height <= 1.0 && abs(p.x - center - curl) < width + noise * 3.0 {
        let heat = floor(clamp(1.0 - abs(p.x - center - curl) / max(width, 1.0), 0.0, 1.0) * 3.0) / 3.0;
        let shade = mix(prism.padding.rgb, prism.detail.yzw, 0.55);
        color = mix(shade, prism.light.rgb, 0.15 + heat * 0.70);
        if heat > 0.65 && height < 0.45 { color = mix(color, vec3<f32>(1.0), 0.65); }
        coverage = 0.92;
    }
    // Analytic particles: staggered births, no simulation buffers or frame history.
    // Two in three rise from the tongue and fade; one in three settles as ash
    // below the foot. Three discrete tail shades retain square silhouettes.
    for (var i = 0; i < 24; i += 1) {
        let seed = grain(vec2<f32>(f32(i), 17.0));
        let age = fract(time * (0.30 + seed * 0.16) + f32(i) * 0.618034);
        let side = select(-1.0, 1.0, i % 2 == 0);
        let spread = 1.6;
        let rising = i % 3 != 2;
        let x = center + side * (2.0 + seed * 5.0 + age * (6.0 + seed * 16.0)) * spread;
        var y = base - 6.0 - age * (base + 4.0);
        if !rising { y = base + 2.0 + age * (extent.y - base - 4.0); }
        let delta = p - floor(vec2<f32>(x, y) / 2.0) * 2.0;
        let along = select(delta.y, -delta.y, rising);
        if abs(delta.x) < 1.0 && along <= 0.0 && along >= -4.0 && age < 0.92 {
            let tail = 1.0 + along / 6.0;
            color = mix(prism.detail.yzw, prism.light.rgb, tail * 0.65);
            coverage = max(coverage, (0.25 + floor((1.0 - age) * 3.0) / 4.0) * tail);
        }
    }
    let alpha = coverage * prism.size.w;
    return vec4<f32>(color * alpha, alpha);
}
fn hue_rgb(h: f32) -> vec3<f32> {
    let k = fract(vec3<f32>(h) + vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0;
    return clamp(abs(k - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}
// Rainbow selection halo outside the control: hue walks the perimeter, the
// glow falls off in three stepped shades on the two-point lattice and
// lattice cells twinkle. The control's interior is left to the slot art.
fn halo(p: vec2<f32>, extent: vec2<f32>) -> vec4<f32> {
    let q = floor(p / 2.0) * 2.0 + vec2<f32>(1.0);
    let outside = max(max(-q.x, q.x - extent.x), max(-q.y, q.y - extent.y));
    if outside < 0.0 { discard; }
    let time = floor(prism.size.z * 12.0) / 12.0;
    let d = q - extent * 0.5;
    let turn = atan2(d.x, -d.y) / 6.2831853;
    let hue = floor(fract(turn * 2.0 + time * 0.22) * 12.0) / 12.0;
    var color = mix(hue_rgb(hue), vec3<f32>(1.0), 0.18);
    let fall = clamp(1.0 - outside / 6.0, 0.0, 1.0);
    let pulse = 0.90 + 0.10 * sin(time * 1.6);
    var alpha = ceil(fall * 3.0) / 3.0 * 0.30 * pulse;
    if outside < 1.5 { alpha = 0.70; }
    // Rare, slow twinkle: at 6 Hz and 3% of cells the ring read as static noise.
    let twinkle = grain(floor(q / 2.0) + vec2<f32>(floor(time * 2.0), 3.0));
    if twinkle > 0.992 && outside >= 1.5 {
        color = mix(color, vec3<f32>(1.0), 0.5);
        alpha = 0.6;
    }
    alpha *= prism.size.w;
    return vec4<f32>(color * alpha, alpha);
}
@fragment fn fs_main(v: Vertex) -> @location(0) vec4<f32> {
    let extent = prism.size.xy;
    let p = floor(v.uv * (extent + vec2<f32>(12.0)) - vec2<f32>(6.0));
    if prism.detail.x > 2.5 { return halo(p, extent); }
    if any(p < vec2<f32>(0.0)) || any(p >= extent) { discard; }
    if prism.detail.x > 1.5 { return flame(p, extent); }
    let rim = min(min(p.x, p.y), min(extent.x - 1.0 - p.x, extent.y - 1.0 - p.y));
    if rim >= 2.0 { discard; }
    let upper = p.y < 2.0 || (p.x < 2.0 && p.y < extent.y - 2.0);
    let shade = select(0.16, 0.52, upper);
    var color = mix(prism.padding.rgb, prism.light.rgb, shade);
    var alpha = select(0.18, 0.42, rim < 1.0) * prism.size.w;
    if prism.detail.x > 0.5 { alpha *= 0.35; }
    if !upper { color *= 0.48; }
    return vec4<f32>(color * alpha, alpha);
}
