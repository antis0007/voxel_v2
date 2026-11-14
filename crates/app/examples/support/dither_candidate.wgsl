// REJECTED for production: slower in repeated 4K resolve measurements on RTX 4090.
// Retained only to reproduce the numeric/GPU comparison.
fn dithered_surface_write_candidate(c: vec3<f32>, noise: f32) -> vec3<f32> {
    let linear = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));

    // Noise is +/- half an sRGB code. Expand the inverse transfer locally
    // around the original linear colour: (1+q)^2.4 = 1+2.4q+1.68q²+0.224q³.
    // On the nonlinear branch |q| < 0.021, so the omitted term is below
    // 1e-6 sRGB codes (float roundoff dominates). Remove the second RGB pow
    // without flattening the dither in shadows. Anchor at the knee for crossings from the linear toe;
    // evaluate that toe exactly, including black and negative noise.
    let anchor = max(linear, vec3<f32>(0.0031308));
    let encoded_anchor = 1.055 * pow(anchor, vec3<f32>(1.0 / 2.4));
    let encoded = select(encoded_anchor - 0.055, linear * 12.92,
        linear < vec3<f32>(0.0031308));
    let shifted = clamp(encoded + vec3<f32>(noise / 255.0),
        vec3<f32>(0.0), vec3<f32>(1.0));
    let q = (shifted + 0.055 - encoded_anchor) / encoded_anchor;
    let nonlinear = anchor + anchor * q * (2.4 + q * (1.68 + q * 0.224));
    return clamp(select(nonlinear, shifted / 12.92, shifted < vec3<f32>(0.04045)),
        vec3<f32>(0.0), vec3<f32>(1.0));
}
