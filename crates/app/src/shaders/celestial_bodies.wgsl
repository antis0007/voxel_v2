// Moon, gas giant and the vacuum sun, drawn as lit spheres in direction space.
// Surfaces sit on a pixel grid across the disc (one texel size per body) so
// they read as voxel sprites; light across them stays continuous.

const CEL_TAU: f32 = 6.2831853;
// Texels across the moon's diameter.
const CEL_MOON_TEXELS: f32 = 32.0;
// Texels across the giant's diameter; about two pixels per texel from home.
const CEL_GIANT_TEXELS: f32 = 32.0;

struct CelDisc {
    // Disc coordinate: |q| = 1 on the limb, +x right, +y toward `up`.
    q: vec2<f32>,
    // Basis: right, up, toward the body.
    u: vec3<f32>,
    v: vec3<f32>,
    c: vec3<f32>,
    // 0..1 antialiased disc coverage.
    cover: f32,
    sin_r: f32,
}

// Projects `dir` onto the disc of a body at `centre` with angular radius cos `cos_r`.
// The basis follows `up` so a surface stays fixed while the camera turns.
fn cel_disc(dir: vec3<f32>, centre: vec3<f32>, cos_r: f32, up: vec3<f32>) -> CelDisc {
    var d: CelDisc;
    d.c = normalize(centre);
    var up_t = up - d.c * dot(up, d.c);
    if dot(up_t, up_t) < 1e-6 {
        up_t = vec3<f32>(1.0, 0.0, 0.0) - d.c * d.c.x;
    }
    d.v = normalize(up_t);
    d.u = cross(d.v, d.c);
    d.sin_r = sqrt(max(1.0 - cos_r * cos_r, 1e-10));
    d.q = vec2<f32>(dot(dir, d.u), dot(dir, d.v)) / d.sin_r;
    let r = length(d.q);
    let aa = max(fwidth(r), 0.002);
    d.cover = select(0.0, 1.0 - smoothstep(1.0 - aa, 1.0 + aa, r), dot(dir, d.c) > 0.0);
    return d;
}

// Snaps a disc coordinate to the centre of its texel.
fn cel_snap(q: vec2<f32>, texels: f32) -> vec2<f32> {
    let half = texels * 0.5;
    return (floor(q * half) + 0.5) / half;
}

// Body-local point on the visible hemisphere for disc coordinate `q`.
fn cel_sphere(q: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(q, sqrt(max(1.0 - dot(q, q), 0.0)));
}

// Crater relief and albedo at body-local point `p`: x = height, y = albedo.
fn cel_moon_surface(p: vec3<f32>) -> vec2<f32> {
    let maria = smoothstep(0.50, 0.62, mood_fbm(p * 2.3 + vec3<f32>(4.1, 1.7, 0.3)));
    var height = -0.25 * maria;
    var albedo = mix(0.30, 0.13, maria);
    // Three crater sizes; each cell holds at most one crater wholly inside it.
    for (var i = 0; i < 3; i++) {
        let f = 3.0 * pow(2.2, f32(i));
        let cell = floor(p * f);
        let roll = mood_hash31(cell + vec3<f32>(f32(i) * 17.0));
        if roll < 0.45 { continue; }
        let jitter = vec3<f32>(
            mood_hash31(cell + vec3<f32>(1.0, 7.0, 3.0)),
            mood_hash31(cell + vec3<f32>(5.0, 2.0, 9.0)),
            mood_hash31(cell + vec3<f32>(8.0, 4.0, 6.0))) - 0.5;
        let radius = 0.16 + 0.16 * mood_hash31(cell + vec3<f32>(3.0));
        let d = length(fract(p * f) - 0.5 - jitter * (0.9 - radius * 2.0)) / radius;
        let bowl = 1.0 - smoothstep(0.0, 0.85, d);
        let rim = exp(-pow((d - 0.95) / 0.16, 2.0));
        height += (rim * 0.35 - bowl * 0.5) / (1.0 + f32(i));
        // Young craters keep a bright floor and a halo of ejecta.
        let young = step(0.86, roll);
        albedo += rim * 0.05 - bowl * 0.03 * (1.0 - young) + young * exp(-max(d - 1.0, 0.0) * 2.5) * 0.06;
    }
    // One large rayed crater on the near side.
    let ray_c = normalize(vec3<f32>(-0.32, -0.46, 0.83));
    let along = dot(p, ray_c);
    let side = normalize(p - ray_c * along + vec3<f32>(1e-5));
    let angle = atan2(dot(side, cross(ray_c, vec3<f32>(0.0, 1.0, 0.0))), side.y);
    let rays = pow(mood_noise3(vec3<f32>(cos(angle) * 9.0, sin(angle) * 9.0, 0.5)), 4.0);
    let reach = acos(clamp(along, -1.0, 1.0));
    albedo += rays * exp(-reach * 2.4) * smoothstep(0.03, 0.08, reach) * 0.22;
    return vec2<f32>(height, clamp(albedo, 0.06, 0.48));
}

// The moon as a lit sphere: rgb is radiance, a is coverage of the sky behind.
// `earthshine` is the planet's light on the night side (0 on a flat world).
fn cel_moon(dir: vec3<f32>, centre: vec3<f32>, cos_r: f32, sun: vec3<f32>,
            tint: vec3<f32>, earthshine: f32) -> vec4<f32> {
    let disc = cel_disc(dir, centre, cos_r, vec3<f32>(0.0, 1.0, 0.0));
    if disc.cover <= 0.0 { return vec4<f32>(0.0); }
    let qs = cel_snap(disc.q, CEL_MOON_TEXELS);
    let p = cel_sphere(qs);
    let surface = cel_moon_surface(p);
    // Relief: compare with the texel one step toward the sun along the disc.
    let sun_q = vec2<f32>(dot(sun, disc.u), dot(sun, disc.v));
    let toward = sun_q / max(length(sun_q), 1e-4);
    let next = cel_moon_surface(cel_sphere(qs + toward * (2.0 / CEL_MOON_TEXELS)));
    let slope = surface.x - next.x;
    // Lighting is continuous across texels: the terminator never steps.
    let n = cel_sphere(disc.q);
    let normal = disc.u * n.x + disc.v * n.y - disc.c * n.z;
    let mu0 = dot(normal, sun);
    let mu = max(n.z, 0.02);
    // Lommel-Seeliger keeps a full moon flat; a little Lambert adds roundness.
    let ls = max(mu0, 0.0) / (max(mu0, 0.0) + mu);
    let terminator = 1.0 / (max(mu0, 0.0) + 0.25);
    let relief = clamp(1.0 - slope * 0.6 * terminator, 0.2, 1.8);
    let lit = (1.4 * ls + 0.45 * max(mu0, 0.0)) * relief * smoothstep(-0.04, 0.06, mu0);
    // Maria lean blue and highlands warm, as real basalt and anorthosite do.
    let mineral = mix(vec3<f32>(0.92, 0.96, 1.06), vec3<f32>(1.04, 1.0, 0.94), smoothstep(0.14, 0.28, surface.y));
    let albedo = surface.y / 0.25 * mineral;
    let night = earthshine * mu * vec3<f32>(0.55, 0.70, 1.0);
    return vec4<f32>(tint * albedo * (lit + night) * disc.cover, disc.cover);
}

// Filtered radial profile, with the pixel footprint measured in giant radii.
fn cel_ring_density(rho: f32, footprint: f32) -> f32 {
    let aa = 0.5 * min(footprint, 1.0);
    if rho <= 1.35 - aa || rho >= 2.35 + aa { return 0.0; }
    let edges = smoothstep(1.35 - aa, 1.45 + aa, rho)
        * (1.0 - smoothstep(2.25 - aa, 2.35 + aa, rho));
    let gap_width = sqrt(0.035 * 0.035 + aa * aa);
    let gap_dist = (rho - 1.95) / gap_width;
    let gap = 1.0 - (0.035 / gap_width) * exp(-gap_dist * gap_dist);
    // Fade each frequency before it falls below two pixels.
    let broad = cos(CEL_TAU * rho * 7.0) * (1.0 - smoothstep(0.2, 0.5, footprint * 7.0));
    let fine = cos(CEL_TAU * rho * 31.0) * (1.0 - smoothstep(0.2, 0.5, footprint * 31.0));
    return (0.68 + 0.16 * broad + 0.09 * fine) * gap * edges;
}

// A banded gas giant with an icy ring. `pole` is its spin axis.
fn cel_giant(dir: vec3<f32>, centre: vec3<f32>, cos_r: f32, sun: vec3<f32>,
             pole: vec3<f32>, t: f32) -> vec4<f32> {
    let disc = cel_disc(dir, centre, cos_r, pole);
    let ray_dx = dpdx(dir);
    let ray_dy = dpdy(dir);
    let ring_span = 2.85 * disc.sin_r;
    if dot(dir, disc.c) < sqrt(max(1.0 - ring_span * ring_span, 0.0)) { return vec4<f32>(0.0); }
    // Work in giant radii with the eye at the origin.
    let dist = 1.0 / disc.sin_r;
    let centre_g = disc.c * dist;
    let b = dot(dir, centre_g);
    let nearest = dir * b - centre_g;
    let h = 1.0 - dot(nearest, nearest);
    let sphere_t = b - sqrt(max(h, 0.0));
    let sun_elevation = dot(sun, pole);
    var color = vec3<f32>(0.0);
    var cover = 0.0;
    if disc.cover > 0.0 {
        let qs = cel_snap(disc.q, CEL_GIANT_TEXELS);
        let ps = cel_sphere(qs);
        // True latitude about the spin axis, so bands curve when the pole leans toward the eye.
        let surface = normalize(disc.u * ps.x + disc.v * ps.y - disc.c * ps.z);
        let lat = dot(surface, pole);
        // Use the star-facing meridian as the reference for the animated belts.
        let equatorial_sun = sun - pole * sun_elevation;
        let meridian = normalize(select(disc.u, equatorial_sun, dot(equatorial_sun, equatorial_sun) > 1e-6));
        let spin = t * (CEL_TAU / 200.0);
        let storm_axis = meridian * cos(spin) + cross(pole, meridian) * sin(spin);
        let east = cross(pole, storm_axis);
        let longitude = vec2<f32>(dot(surface, storm_axis), dot(surface, east));
        let swirl = mood_noise3(vec3<f32>(longitude * 3.0, lat * 9.0 + 1.3));
        let band = smoothstep(0.25, 0.75, 0.5 + 0.5 * sin(lat * 15.0 + swirl * 2.4));
        // Rust belts, cream zones and a darker polar hood.
        let belt = mix(vec3<f32>(0.40, 0.17, 0.08), vec3<f32>(0.58, 0.32, 0.15), smoothstep(-0.2, 0.2, swirl));
        let zone = mix(belt, vec3<f32>(0.86, 0.66, 0.42), band) * mix(1.0, 0.62, smoothstep(0.6, 0.95, abs(lat)));
        let storm = exp(-longitude.y * longitude.y / 0.024 - (lat + 0.30) * (lat + 0.30) / 0.009)
            * smoothstep(0.6, 0.9, longitude.x);
        let albedo = mix(zone, vec3<f32>(0.85, 0.36, 0.22), storm * 0.8);
        let normal = normalize(nearest - dir * sqrt(max(h, 0.0)));
        let mu0 = max(dot(normal, sun), 0.0);
        // Ring shadow: the sunward ray from this point crosses the ring plane.
        var shadow = 0.0;
        if h > 0.0 && mu0 > 0.0 && abs(sun_elevation) > 1e-4 {
            let s = -dot(normal, pole) / sun_elevation;
            if s > 0.0 {
                let blur = dist * (length(ray_dx) + length(ray_dy)) / abs(sun_elevation);
                shadow = cel_ring_density(length(normal + sun * s), blur)
                    * smoothstep(0.0001, 0.015, abs(sun_elevation));
            }
        }
        let limb = pow(clamp(-dot(normal, dir), 0.0, 1.0), 0.35);
        // Faint fill keeps the night side's silhouette against the stars.
        let fill = 0.035 * vec3<f32>(0.55, 0.65, 0.9);
        color = albedo * (mu0 * limb * (1.0 - 0.7 * shadow) * 0.8 + fill) * disc.cover;
        cover = disc.cover;
    }
    let view_elevation = dot(dir, pole);
    let elevation_pixel = abs(dot(ray_dx, pole)) + abs(dot(ray_dy, pole));
    if abs(view_elevation) > max(1e-5, 0.5 * elevation_pixel) {
        let along = dot(centre_g, pole) / view_elevation;
        if along <= 0.0 || (along >= sphere_t && cover >= 1.0) { return vec4<f32>(color, cover); }
        let at = dir * along;
        let radial = at - centre_g;
        let rho = length(radial);
        let radial_dir = radial / max(rho, 1e-5);
        // Differentiate the plane intersection, not the projected disc radius.
        let radial_gradient = radial_dir - pole * dot(radial_dir, dir) / view_elevation;
        let footprint = along * (abs(dot(radial_gradient, ray_dx)) + abs(dot(radial_gradient, ray_dy)));
        let density = cel_ring_density(rho, footprint);
        if density > 0.0 {
            // The body's shadow on the ring.
            let to_c = centre_g - at;
            let s = dot(to_c, sun);
            let lit = select(1.0, smoothstep(0.9, 1.05, length(to_c - sun * s)), s > 0.0);
            let ice = mix(vec3<f32>(0.77, 0.66, 0.56), vec3<f32>(0.86, 0.83, 0.76), smoothstep(1.5, 2.25, rho));
            let mu = abs(view_elevation);
            let mu_s = abs(sun_elevation);
            let incidence = mu_s / max(mu_s + mu, 1e-4) * smoothstep(0.0, 0.025, mu_s);
            let phase = dot(dir, sun);
            let forward = 0.2 + 0.8 * pow(max(phase, 0.0), 4.0);
            let scattering = select(forward, 1.0, view_elevation * sun_elevation < 0.0);
            let radiance = ice * (0.025 + 1.25 * incidence * scattering * lit);
            let resolved = 1.0 - smoothstep(0.45, 1.5, footprint);
            let grazing = smoothstep(max(1e-5, 0.5 * elevation_pixel), max(2e-5, 1.5 * elevation_pixel), mu);
            let alpha = (1.0 - exp(-density * 0.65 / max(mu, 0.05))) * resolved * grazing;
            if along < sphere_t {
                color = color * (1.0 - alpha) + radiance * alpha;
            } else {
                color += radiance * alpha * (1.0 - cover);
            }
            cover += alpha * (1.0 - cover);
        }
    }
    return vec4<f32>(color, cover);
}

// The sun seen from above the air: limb-darkened disc, corona streamers and glare.
fn cel_sun_vacuum(dir: vec3<f32>, sun: vec3<f32>, cos_r: f32, vacuum: f32) -> vec3<f32> {
    let cos_angle = clamp(dot(dir, sun), -1.0, 1.0);
    let theta = sqrt(max(2.0 * (1.0 - cos_angle), 0.0));
    let radius = sqrt(max(2.0 * (1.0 - cos_r), 1e-10));
    let aa = max(fwidth(theta), radius * 0.03);
    let disk = 1.0 - smoothstep(radius - aa, radius + aa, theta);
    let x = min(theta / radius, 1.0);
    let mu = sqrt(max(1.0 - x * x, 0.0));
    let core = vec3<f32>(8.0, 7.6, 6.5) * (0.4 + 0.6 * mu) * disk;
    if theta > radius * 9.0 { return core; }
    // Streamers: angular noise around the disc, longer at the poles of the field.
    var side = dir - sun * cos_angle;
    side = side / max(length(side), 1e-6);
    let a = atan2(side.y, dot(side, normalize(cross(sun, vec3<f32>(0.0, 1.0, 0.001)))));
    let streak = mood_noise3(vec3<f32>(cos(a) * 5.0, sin(a) * 5.0, 2.0));
    let out = max(theta / radius, 1.0);
    let corona = pow(out, -3.0) * (0.5 + 1.6 * streak * streak) * (1.0 - disk);
    let pearl = mix(vec3<f32>(1.0, 0.96, 0.90), vec3<f32>(0.86, 0.92, 1.0), smoothstep(1.2, 4.0, out));
    let glare = exp(-(out - 1.0) * 1.6) * (1.0 - disk) * 0.6;
    return core + pearl * (corona * 1.8 * vacuum + glare);
}
