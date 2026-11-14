// sRGB transfer functions shared by the post-process and present programs.
fn srgb_encode(c: vec3<f32>) -> vec3<f32> {
    let low = c * 12.92;
    let high = 1.055 * pow(max(c, vec3<f32>(0.0031308)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, c < vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, c < vec3<f32>(0.04045));
}

// One native-pixel, one-code dither before the sRGB attachment's final write.
// Noise is stationary and centred on zero; lighting and palette ramps stay intact.
fn dithered_surface_write(c: vec3<f32>, noise: f32) -> vec3<f32> {
    let s = srgb_encode(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
    return srgb_decode(clamp(s + vec3<f32>(noise / 255.0),
        vec3<f32>(0.0), vec3<f32>(1.0)));
}
