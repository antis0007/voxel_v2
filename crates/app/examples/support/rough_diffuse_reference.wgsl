// Pre-2026-11-08 reference, retained only for paired numeric/GPU profiling.
fn oren_nayar_reference(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.0);
    if nl <= 0.0 || nv <= 0.0 {
        return 0.0;
    }
    let sigma2  = roughness * roughness;
    let a       = 1.0 - sigma2 / (2.0 * (sigma2 + 0.33));
    let b_coef  = 0.45 * sigma2 / (sigma2 + 0.09);
    let lp = l - n * nl;
    let vp = v - n * nv;
    let lv = lp / max(length(lp), 1e-4);
    let vv = vp / max(length(vp), 1e-4);
    let cos_phi = max(dot(lv, vv), 0.0);
    let alpha   = max(nl, nv);
    let beta    = min(nl, nv);
    // alpha/beta above are COSINES: the larger angle has the smaller cosine.
    let sin_alpha = sqrt(max(1.0 - beta * beta, 0.0));
    let tan_beta = sqrt(max(1.0 - alpha * alpha, 0.0)) / max(alpha, 1e-3);
    return nl * (a + b_coef * cos_phi * sin_alpha * tan_beta);
}
