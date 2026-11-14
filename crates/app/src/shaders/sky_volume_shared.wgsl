// Local sky visibility probes (render/sky_volume.rs), sampled for ambient light.
// Direct sun and moon never read this: their occlusion is the shadow map.

// origin.xyz: probe window origin in cells, origin.w: edge (0 = not baked yet).
// scale.x: voxels per cell.
struct SkyVolumeWindow {
    origin: vec4<i32>,
    scale: vec4<i32>,
};
@group(1) @binding(3) var sky_volume_probes: texture_3d<u32>;
@group(1) @binding(4) var<uniform> sky_volume_window: SkyVolumeWindow;
@group(1) @binding(5) var sky_volume_coarse_probes: texture_3d<u32>;
@group(1) @binding(6) var<uniform> sky_volume_coarse_window: SkyVolumeWindow;

struct LocalSky {
    // Cosine-weighted sky transmittance for the face, 0..1; open ground is 1.
    visibility: f32,
    // How much to trust it: 0 outside the windows or between unbaked probes.
    weight: f32,
};

struct ProbeSample {
    sky: LocalSky,
    // Window edge fade alone, 1 well inside the window.
    fade: f32,
};

// `voxel` is an absolute voxel-space point one half voxel off the surface,
// `n` the geometric normal. Inside the fine window the fine level decides,
// even where every probe around the point is solid and it defers to the
// column term; the coarse level only fills the fine fade band and beyond.
fn local_sky_visibility(voxel: vec3<f32>, n: vec3<f32>) -> LocalSky {
    let fine_sample = sample_sky_probes(sky_volume_probes, sky_volume_window, voxel, n);
    let fine = fine_sample.sky;
    if fine_sample.fade >= 1.0 { return fine; }
    let coarse = sample_sky_probes(sky_volume_coarse_probes, sky_volume_coarse_window, voxel, n).sky;
    // mix(mix(column, coarse, wc), fine, wf) written as one mix over the column.
    let weight = 1.0 - (1.0 - fine.weight) * (1.0 - coarse.weight);
    if weight < 1e-4 { return LocalSky(1.0, 0.0); }
    let visibility = ((1.0 - fine.weight) * coarse.weight * coarse.visibility
        + fine.weight * fine.visibility) / weight;
    return LocalSky(visibility, weight);
}

// Trilinear over the eight nearest probe centres of one level, skipping solid
// and unbaked ones, so walls never leak a far-side probe in.
fn sample_sky_probes(probes: texture_3d<u32>, window: SkyVolumeWindow, voxel: vec3<f32>, n: vec3<f32>) -> ProbeSample {
    let edge = window.origin.w;
    if edge <= 0 { return ProbeSample(LocalSky(1.0, 0.0), 0.0); }
    // Coarse cells holding a surface are often solid; stepping out to half a
    // cell off the face reaches the air probes above it.
    let scale = f32(max(window.scale.x, 1));
    let p = voxel / scale + n * (0.5 - 0.5 / scale);
    let lo = vec3<f32>(window.origin.xyz);
    let inside = min(p - lo, lo + vec3<f32>(f32(edge)) - p);
    let fade = smoothstep(1.0, 8.0, min(inside.x, min(inside.y, inside.z)));
    if fade <= 0.0 { return ProbeSample(LocalSky(1.0, 0.0), 0.0); }
    let q = p - vec3<f32>(0.5);
    let base = vec3<i32>(floor(q));
    let f = q - floor(q);
    // +y, +x, -x, +z, -z
    var up = 0.0;
    var xs = vec2<f32>(0.0);
    var zs = vec2<f32>(0.0);
    var total = 0.0;
    for (var i = 0; i < 8; i += 1) {
        let o = vec3<i32>(i & 1, (i >> 1) & 1, (i >> 2) & 1);
        let cell = base + o;
        let slot = ((cell % vec3<i32>(edge)) + vec3<i32>(edge)) % vec3<i32>(edge);
        let probe = textureLoad(probes, slot, 0);
        if (probe.x >> 8u) != 2u { continue; }
        let t = select(vec3<f32>(1.0) - f, f, o == vec3<i32>(1));
        let w = t.x * t.y * t.z;
        up += f32(probe.x & 255u) * w;
        xs += vec2<f32>(f32(probe.y & 255u), f32(probe.y >> 8u)) * w;
        zs += vec2<f32>(f32(probe.z & 255u), f32(probe.z >> 8u)) * w;
        total += w;
    }
    if total < 0.02 { return ProbeSample(LocalSky(1.0, 0.0), fade); }
    let s = 1.0 / (255.0 * total);
    up *= s;
    xs *= s;
    zs *= s;
    // A face turned down sees no sky directly; how enclosed the space around
    // it is stands in, so ceilings in caves darken and open eaves do not.
    let down = (xs.x + xs.y + zs.x + zs.y) * 0.25;
    let n2 = n * n;
    let visibility = n2.y * select(down, up, n.y >= 0.0)
        + n2.x * select(xs.y, xs.x, n.x >= 0.0)
        + n2.z * select(zs.y, zs.x, n.z >= 0.0);
    return ProbeSample(LocalSky(clamp(visibility, 0.0, 1.0), fade * smoothstep(0.02, 0.25, total)), fade);
}
