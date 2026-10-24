// Continuous kilometer-scale organization shared by cloud banks and rain.
// Hash noise avoids a repeating volume-texture period across the horizon.
fn weather_bank_noise(p: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    return mix(mix(hash_cell1(cell, 4127), hash_cell1(cell + vec2<i32>(1, 0), 4127), u.x),
               mix(hash_cell1(cell + vec2<i32>(0, 1), 4127),
                   hash_cell1(cell + vec2<i32>(1, 1), 4127), u.x), u.y);
}

fn weather_bank(xz: vec2<f32>) -> f32 {
    let q = vec2<f32>(xz.x * 0.82 + xz.y * 0.57, -xz.x * 0.57 + xz.y * 0.82);
    return weather_bank_noise(q / vec2<f32>(2400.0, 1500.0)) * 0.72
        + weather_bank_noise(q / 670.0 + vec2<f32>(31.7, -19.3)) * 0.28;
}

// Polynomial smooth maximum for blending overlapping cloud thermals.
// This keeps the low-cost implicit field, but removes hard seams that make
// each updraft read as a stack of separate balls.
fn cloud_smooth_max(a: f32, b: f32, radius: f32) -> f32 {
    let h = max(radius - abs(a - b), 0.0) / max(radius, 1e-4);
    return max(a, b) + h * h * radius * 0.25;
}

// Kilometer-scale masses. A second, coarser lattice than the puffs, with its
// own holes, stretches and heights, so a sky can hold one huge bank beside
// empty air. Cheap ellipsoids only: the puff lattice supplies the silhouette.
fn broad_cloud_mass(xz: vec2<f32>, h: f32, coverage: f32, bank: f32, storm: f32, size: f32,
    thick_m: f32) -> f32 {
    let cell_size = size * 2.7;
    let cell = vec2<i32>(floor(xz / cell_size));
    let odds = saturate1(coverage * mix(0.35, 1.25, bank) + storm * 0.28);
    var field = -2.0;
    for (var z = -1; z <= 1; z += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let id = cell + vec2<i32>(x, z);
            let seed = hash_cell1(id, 2203);
            let jitter = hash_cell2(id, 901);
            let center = (vec2<f32>(id) + jitter) * cell_size;
            let local = xz - center;
            let stretch = mix(0.28, 1.25, hash_cell1(id, 15));
            // The longest ellipse axis is kept to 0.68 cell. Cloud density
            // starts near dome -0.74 (lobes reach +0.59), which is 1.32 axis
            // lengths out, so every visible contour lies inside one cell width
            // and the 3x3 lookup never truncates it. Truncating a still-dense
            // dome with a steep taper drew a straight vertical wall.
            var reach = cell_size * mix(0.28, 0.82, hash_cell1(id, 77));
            let support = min(reach * max(stretch, 1.0), cell_size * 0.68);
            reach = support / max(stretch, 1.0);
            // Beyond 1.45 axis lengths the dome is below -1.1: nothing visible.
            let cutoff = support * 1.45;
            if dot(local, local) > cutoff * cutoff { continue; }
            // Coverage raises the odds. It does not turn every cell on, so a
            // deck still has corridors and one-sided storms stay one-sided.
            // Weather varies at every sample, even within one lobe. A binary
            // lottery removed a still-positive dome at seed == odds, slicing
            // clouds along weather contours. Move its signed field continuously
            // out of the density range instead; do not multiply a signed field
            // toward zero (which would manufacture density outside the lobe).
            let presence = smoothstep(seed - 0.08, seed + 0.08, odds);
            if presence <= 0.001 { continue; }
            // A mass is never much taller than it is wide: narrow cells with a
            // tall draw stood up as kilometre-high pillars.
            let height = min(mix(0.18, 0.72, hash_cell1(id, 44)) + storm * 0.22,
                reach * min(stretch, 1.0) * 2.4 / max(thick_m, 1.0));
            let hh = (h - mix(0.02, 0.16, seed)) / max(height, 0.12);
            // Rotation preserves length. Dividing by the largest ellipse axis
            // bounds its horizontal quadratic below, hence the field above.
            // A smooth-max input at least its radius below field adds nothing.
            let largest_axis = support;
            let upper = 1.0 - dot(local, local) / (largest_axis * largest_axis)
                - hh * hh - (1.0 - presence) * 4.0;
            if upper + 0.001 <= field - 0.30 { continue; }
            let axis = safe_normalize2(jitter - vec2<f32>(0.5), vec2<f32>(1.0, 0.0));
            let side = vec2<f32>(-axis.y, axis.x);
            let d = vec2<f32>(dot(local, axis), dot(local, side)) / vec2<f32>(reach, reach * stretch);
            // Smooth compact support inside one cell width: the 3x3 lookup
            // can never omit a contributing mass, and no edge is a hard cut.
            let taper = smoothstep(0.95, 1.0, length(local) / (cell_size * 0.98)) * 4.0;
            let dome = 1.0 - dot(d, d) - hh * hh - taper;
            field = cloud_smooth_max(field, dome - (1.0 - presence) * 4.0, 0.30);
        }
    }
    return field;
}

// Cloud geometry in meters. Overlapping, tilted thermals form a connected
// three-dimensional field. A thermal has a ragged foot, offset mid-level
// shoulders and an independently lifting crown instead of a single ball.
fn thermal_profile(id: vec2<i32>, jitter: vec2<f32>, h: f32, radius: f32,
    cluster: f32, storm: f32, thick_m: f32) -> vec4<f32> {
    let kind = hash_cell1(id, 947);
    var stretch = mix(0.50, 1.40, jitter.y);
    let growth = hash_cell1(id, 1733);
    var height = mix(0.26, 0.92, growth) + cluster * 0.06
        + storm * smoothstep(0.42, 0.90, growth) * 0.30;
    if kind > 0.80 {
        stretch = mix(0.20, 0.40, jitter.x);
        height = mix(0.16, 0.38, growth);
    } else if kind < 0.14 {
        stretch = mix(0.80, 1.08, jitter.y);
        height = mix(0.75, 1.20, growth) + storm * 0.18;
    } else if kind < 0.28 {
        height = mix(0.12, 0.28, growth);
        stretch = mix(0.55, 1.30, jitter.y);
    }
    height = min(height, radius * min(stretch, 1.0) * 3.0 / max(thick_m, 1.0));
    let foot = hash_cell1(id, 2309) * mix(0.035, 0.12, 1.0 - storm);
    return vec4<f32>(stretch, height, growth, h - foot);
}

// Bound the transformed horizontal radius with triangle inequalities before
// evaluating rotations and angular lobes. The largest possible waist and
// radial modulation are 1.32 and 1.20; 1.585 rounds their product upward.
// Each lobe then subtracts a LOWER bound on its squared horizontal distance.
// Smooth max is monotone, so the same blends bound the complete field above.
fn thermal_upper_bound(profile: vec4<f32>, id: vec2<i32>, jitter: vec2<f32>,
    radius: f32, size: f32, local: vec2<f32>, original_h: f32, storm: f32) -> f32 {
    let height = profile.y; let growth = profile.z; let h = profile.w;
    // Deformation uses the original layer height, before the cell foot offset.
    // Wind and side axes are unit vectors; |roll| <= .12, hence .0048 * size.
    let bend = abs((original_h - 0.35) * (0.12 + storm * 0.10)) * size;
    let updraft = smoothstep(0.08, 0.92, original_h) * size * (0.10 + storm * 0.06);
    let twisted_min = max(length(local) - bend - updraft - size * 0.0048, 0.0);
    let radial_min = twisted_min / (radius * max(profile.x, 1.0) * 1.585);
    let tower_xy = radial_min * 0.78;
    let shoulder_xy = max(radial_min - 0.434, 0.0) / 1.12;
    let crown_xy = max(radial_min - 0.511, 0.0) / 0.80;
    let base_xy = radial_min / 1.45;
    let arm_xy = max(radial_min - 1.008, 0.0) / 0.92;
    let shoulder = (h - height * mix(0.44, 0.55, jitter.y)) / (height * mix(0.28, 0.36, jitter.x));
    let tower = (h - height * 0.48) / max(height * 0.40, 0.12);
    let crown = (h - height * mix(0.58, 0.74, growth)) / (height * mix(0.31, 0.44, jitter.y));
    var bound = cloud_smooth_max(1.0 - tower_xy * tower_xy - tower * tower,
        1.0 - shoulder_xy * shoulder_xy - shoulder * shoulder, 0.22);
    bound = cloud_smooth_max(bound, 1.0 - crown_xy * crown_xy - crown * crown, 0.24);
    let base = (h - height * mix(0.23, 0.33, jitter.x)) / (height * mix(0.17, 0.22, jitter.y));
    bound = cloud_smooth_max(bound, 0.65 - base_xy * base_xy - base * base, 0.18);
    let arm = (h - height * mix(0.38, 0.64, jitter.x)) / (height * 0.25);
    bound = cloud_smooth_max(bound,
        1.0 - arm_xy * arm_xy - arm * arm - (1.0 - smoothstep(0.42, 0.78, growth)) * 4.0, 0.13);
    if hash_cell1(id, 3001) > 0.42 && radius < size * 0.48 {
        let crumb = (h - height * 0.32) / max(height * 0.42, 0.08);
        let crumb_xy = max(twisted_min - radius * 1.63, 0.0) / max(radius * 0.62, 1.0);
        bound = cloud_smooth_max(bound, 1.0 - crumb_xy * crumb_xy - crumb * crumb, 0.16);
    }
    return bound;
}

fn cumulus_envelope(xz: vec2<f32>, h: f32, coverage: f32, bank: f32, storm: f32, size: f32,
    thick_m: f32, density_floor: f32) -> f32 {
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    // The field below is rotated into the cloud-bank frame at its call site.
    // Rotate the wind too so the cell's shear and the advected density agree.
    let wind = rotate2(safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4)), 0.43);
    // Use the existing bank field to group growth into broad weather systems.
    // This breaks the one-puff-per-cell rhythm without another texture read.
    let cluster = smoothstep(0.24, 0.78, bank);
    let cell_coverage = saturate1(coverage * mix(0.82, 1.16, cluster) + storm * 0.05);
    let odds = saturate1(cell_coverage * mix(0.55, 1.20, cluster) + storm * 0.22);
    let cell = vec2<i32>(floor(xz / size));
    var envelope = max(-2.0, density_floor);
    // Luck cannot exceed 1, so this bounds every cell's presence before any
    // cell hashes or shape evaluation. Sparse weather often rejects the
    // entire neighborhood, including its nine otherwise empty candidates.
    let max_presence = smoothstep(0.34, 0.72, odds * 1.20);
    if max_presence <= 0.001 || 1.102 - (1.0 - max_presence) * 3.0 + 0.001 <= envelope {
        return envelope;
    }
    // Visit the containing cell, then the closest neighboring side. A strong
    // candidate raises the rejection bound for the remaining cells. This
    // reduction is max(), so order cannot change the cloud field (unlike the
    // smooth-max accumulation in broad_cloud_mass).
    let order = array<i32, 3>(0, -1, 1);
    let side_order = select(vec2<i32>(1), vec2<i32>(-1), fract(xz / size) > vec2<f32>(0.5));
    for (var z = 0; z < 3; z += 1) {
        for (var x = 0; x < 3; x += 1) {
            let id = cell + vec2<i32>(order[x], order[z]) * side_order;
            let jitter = hash_cell2(id, 731);
            // Full-cell placement avoids a regularly spaced empty gutter
            // around every cell. Compact support below is exactly one cell,
            // so an omitted cell outside this 3x3 cannot contribute.
            let center = (vec2<f32>(id) + jitter) * size;
            let local = xz - center;
            if dot(local, local) >= size * size { continue; }
            // A per-cell lottery. High coverage makes clouds likely, and unlucky
            // cells stay empty so the spacing is not a filled grid.
            let luck = hash_cell1(id, 4099);
            let presence = smoothstep(0.34, 0.72, odds * mix(0.40, 1.20, luck));
            // No possible contribution: avoid axes, turrets and divisions for
            // absent cells and cells beyond their compact support.
            if presence <= 0.001 { continue; }
            // This fade is subtracted from every lobe after their smooth max.
            // Include it in both upper bounds too: most neighboring cells sit
            // in this wide fade, and cannot beat the current envelope.
            let support_fade = smoothstep(0.50, 0.99, length(local) / size) * 4.0;
            let cell_penalty = (1.0 - presence) * 3.0 + support_fade;
            // Monotone blends of lobe maxima (1,1,1,.65,1,1) peak below
            // 1.102, even with the optional companion. Reject weak cells
            // before hashing their shape/profile when they cannot win.
            if 1.102 - cell_penalty + 0.001 <= envelope { continue; }
            // Squared class biases the field toward scraps and puffs, with a
            // few cells growing into a distinctly larger neighbour.
            let scale_class = hash_cell1(id, 2811);
            let radius_raw = size * mix(0.07, 0.58, scale_class * scale_class)
                * mix(0.88, 1.10, cluster) * mix(0.80, 1.08, presence)
                * (1.0 + storm * 0.16);
            // A thermal wider than half a cell is truncated by the cell window
            // in a vertical wall. Bigger bodies come from broad_cloud_mass.
            let radius = min(radius_raw, size * 0.50);
            let profile = thermal_profile(id, jitter, h, radius, cluster, storm, thick_m);
            if thermal_upper_bound(profile, id, jitter, radius, size, local, h, storm)
                - cell_penalty + 0.001 <= envelope { continue; }
            let seed = hash_cell1(id, 137);
            let random_axis = safe_normalize2(jitter - vec2<f32>(0.5), vec2<f32>(1.0, 0.0));
            // Use a wind-aligned long axis with per-cell angular variation.
            // The wider flow-aligned lobes make neighboring thermals join into
            // sculpted banks instead of a sky full of detached round puffs.
            let crosswind = vec2<f32>(-wind.y, wind.x);
            let axis = safe_normalize2(mix(random_axis, crosswind, 0.36), random_axis);
            let side = vec2<f32>(-axis.y, axis.x);
            // Tilt and twist each tower as it rises. The offset is continuous
            // in height, so the silhouette never changes with camera motion.
            let base_angle = (seed - 0.5) * 0.75;
            // Local phase is continuous across the map; advection makes each
            // cell's slow rotation follow the wind without camera-relative drift.
            let local_phase = dot(local, wind) * 0.0014;
            // Silhouettes evolve slowly while the advected medium moves with
            // the wind. Faster phase rotation looked like jittering blobs.
            let flow_phase = time * wspd * 0.0035 + seed * 6.2831853 + local_phase;
            // Real towers do not corkscrew. Rotation through height was up to
            // ~2.5 rad per tower and read as twisted taffy; keep only a slight
            // lean of the lobes so stacked billows do not line up exactly.
            let roll = sin(flow_phase + h * 2.3 + jitter.y * 2.7) * 0.12;
            let twist = base_angle + (h - 0.38) * mix(-0.22, 0.22, jitter.x);
            // Shear leans the crown downwind in one smooth arc, no S-bends.
            let bend = (h - 0.35) * (0.12 + storm * 0.10);
            // Vertical wind shear pushes the crown downwind, while a slower
            // cross-flow roll curls it back around the rising thermal. Keep the
            // deformation cell-local: a world-origin rotation would amplify
            // tiny time changes into large motion far from the origin.
            let updraft = smoothstep(0.08, 0.92, h) * size * (0.10 + storm * 0.06);
            let shear = wind * updraft + side * (size * roll * 0.04);
            let twisted = rotate2(local - axis * (size * bend) - shear, twist);
            // Puff, long roll, flat shred, or narrow tower. One formula was
            // drawing the same cauliflower in every cell.
            // Cap the aspect ratio at about 1.5 tall to 1 wide. Small radii
            // under a multi-kilometre storm layer drew pencil-thin columns;
            // real congestus and Cb grow wider as they grow taller.
            let stretch = profile.x;
            let height = profile.y;
            let growth = profile.z;
            let delta = vec2<f32>(dot(twisted, axis), dot(twisted, side))
                / vec2<f32>(radius, radius * stretch);
            // Keep the lifting condensation level coherent across a bank; large
            // per-cell foot offsets made each thermal read as a floating ball.
            let local_h = profile.w;
            // A narrow rising core opens into a wider shoulder, then separates
            // into a lopsided crown. The vertical slices overlap smoothly.
            // Reduce the repeated ball-on-ball rhythm while retaining layered
            // growth; cell phase and advected noise supply irregular billows.
            let waist = 1.0 + 0.21 * sin(local_h * 6.2 + seed * 21.0)
                + 0.11 * sin(local_h * 11.3 + jitter.y * 13.0);
            // Break the ellipsoid cross-section with rotating, height-correlated
            // lobes. This keeps the large cloud bank coherent while giving its
            // sides torn cauliflower folds instead of a stack of smooth balls.
            let azimuth = atan2(delta.y, delta.x);
            let radial_lobes = 1.0
                + 0.13 * sin(azimuth * 3.0 + local_h * 10.0 + flow_phase)
                + 0.07 * sin(azimuth * 5.0 - local_h * 16.0 + jitter.y * 6.2831853);
            // Integer azimuth frequencies: a fractional one jumps at the atan2
            // wrap and cut a vertical seam down the back of every tower.
            let tower_xy = delta / (waist * radial_lobes);
            // Close each thermal below its shoulder. A core centred at zero
            // extended below the layer and was sliced into a horizontal shelf.
            let shoulder_h = (local_h - height * mix(0.44, 0.55, jitter.y))
                / (height * mix(0.28, 0.36, jitter.x));
            let tower_h = (local_h - height * 0.48) / max(height * 0.40, 0.12);
            let crown_h = (local_h - height * mix(0.58, 0.74, growth))
                / (height * mix(0.31, 0.44, jitter.y));
            let shoulder_xy = rotate2(tower_xy - vec2<f32>((jitter.x - 0.5) * 0.72,
                (seed - 0.5) * 0.48), roll * 0.30 + (jitter.y - 0.5) * 0.34)
                / vec2<f32>(1.12, 0.77);
            let crown_xy = rotate2(tower_xy + vec2<f32>((seed - 0.5) * 0.90,
                (jitter.y - 0.5) * 0.48), -roll * 0.38 + (seed - 0.5) * 0.52)
                / vec2<f32>(0.80, 0.64);
            let tower = 1.0 - dot(tower_xy * vec2<f32>(1.0, 0.78), tower_xy * vec2<f32>(1.0, 0.78))
                - tower_h * tower_h;
            let shoulder = 1.0 - dot(shoulder_xy, shoulder_xy) - shoulder_h * shoulder_h;
            let crown = 1.0 - dot(crown_xy, crown_xy) - crown_h * crown_h;
            var dome = cloud_smooth_max(tower, shoulder, 0.22);
            dome = cloud_smooth_max(dome, crown, 0.24);
            // A narrow, irregular condensation foot softens each base without
            // bridging whole cells into the repeated horizontal pancake bands.
            let base_xy = tower_xy / vec2<f32>(1.45, 1.18);
            let base_h = (local_h - height * mix(0.23, 0.33, jitter.x))
                / (height * mix(0.17, 0.22, jitter.y));
            let base_shelf = 1.0 - dot(base_xy, base_xy) - base_h * base_h;
            // Preserve negative distance outside the foot. Scaling this signed
            // field toward zero let later positive noise inflate empty space
            // into a broad pale skirt beneath every thermal.
            dome = cloud_smooth_max(dome, base_shelf - 0.35, 0.18);
            // A few cells grow oblique feeder arms. Their angular and vertical
            // offsets vary by cell, breaking the repeated cauliflower rhythm.
            let arm_h = (local_h - height * mix(0.38, 0.64, jitter.x)) / (height * 0.25);
            let arm_xy = (tower_xy - vec2<f32>(mix(0.42, 0.92, seed),
                (jitter.y - 0.5) * 0.82)) / vec2<f32>(0.92, 0.32);
            let arm = 1.0 - dot(arm_xy, arm_xy) - arm_h * arm_h;
            // A disabled signed-distance lobe must be negative, not zero:
            // multiplying by zero manufactured a flat slab outside the tower.
            dome = cloud_smooth_max(dome,
                arm - (1.0 - smoothstep(0.42, 0.78, growth)) * 4.0, 0.13);
            // A smaller companion sits beside many puffs, so a large cloud
            // keeps scraps around it instead of standing alone on the grid.
            let crumb = hash_cell1(id, 3001);
            if crumb > 0.42 && radius < size * 0.48 {
                let crumb_r = radius * mix(0.30, 0.62, hash_cell1(id, 311));
                let crumb_offset = axis * radius * mix(0.75, 1.55, crumb)
                    + side * radius * (jitter.y - 0.5);
                let crumb_delta = (twisted - crumb_offset) / max(crumb_r, 1.0);
                let crumb_h = (local_h - height * 0.32) / max(height * 0.42, 0.08);
                let crumb_dome = 1.0 - dot(crumb_delta, crumb_delta) - crumb_h * crumb_h;
                dome = cloud_smooth_max(dome, crumb_dome, 0.16);
            }
            // Compact support keeps even noise-inflated storm lobes inside
            // the 3x3 neighborhood, so changing cells cannot drop a lobe.
            // Keep the support compact at the actual cell edge, but let the
            // broad base bridge across most of the Voronoi spacing first.
            // The window must be wide: a thermal's arms and shoulders still
            // have positive dome at the cell edge, and a narrow ramp there is a
            // straight vertical wall. Spread it so the cloud thins instead.
            dome -= support_fade;
            envelope = max(envelope, dome - (1.0 - presence) * 3.0);
        }
    }
    return envelope;
}

fn photographic_low_density(
    p: vec3<f32>, base_y: f32, top_y: f32, footprint: f32,
    weather: CellWeather, macro_bias: f32, shadow_lod: bool,
) -> f32 {
    if p.y <= base_y || p.y >= top_y { return 0.0; }
    if weather.coverage < 0.025 { return 0.0; }
    let h = (p.y - base_y) / max(top_y - base_y, 1.0);
    let wind = safe_normalize2(light.weather_globals[0].zw, vec2<f32>(0.9, -0.4));
    let side = vec2<f32>(-wind.y, wind.x);
    let shear = light.weather_globals[1].z;
    let xz = p.xz + layer_wind_advection_offset(LAYER_LOW)
        + side * h * h * shear * 95.0;
    let bank = weather_bank(xz);
    // Group updrafts into weather systems with clear corridors between them.
    // Preserve zero coverage and avoid a second uniform blanket in storms.
    let coverage = saturate1(weather.coverage * macro_bias
        + (bank - 0.5) * 0.72 * smoothstep(0.025, 0.20, weather.coverage));
    let size = mix(760.0, 380.0, saturate1(light.weather_globals[2].x));
    let storm = weather.storm;
    let convective = smoothstep(0.12, 0.55, weather.cloud_type);

    // Both view and shadow rays resolve the same medium-scale lobes. Smaller
    // texture periods provide actual meter-scale detail instead of magnifying
    // a blurred 64-texel volume across a kilometer.
    let shape_freq = 0.0038;
    let shape_lod = footprint_texel_lod(footprint, shape_freq, SHAPE_TEX_SIZE, 6.0);
    let time = light.weather_globals[0].x;
    let wspd = max(light.weather_globals[0].y, 0.0);
    // Advect the same 3D medium through height as wind moves it across the
    // map. Slow vertical evolution produces swelling/rising edges, not a flat
    // texture sliding under a fixed cloud silhouette.
    let evolving_y = p.y + time * wspd * (0.045 + weather.convection * 0.07);
    let shape = sample_shape_global(xz, evolving_y, shape_freq, shape_freq, shape_lod);
    let warp = (shape.gb - vec2<f32>(0.5)) * 185.0;
    // Broad continuous deformation breaks perspective rows without changing
    // cell support or choosing a camera-dependent neighbourhood.
    let organization = vec2<f32>(weather_bank_noise(xz / 1800.0 + vec2<f32>(17.3, -8.1)),
        weather_bank_noise(xz / 1800.0 + vec2<f32>(-31.8, 42.5))) - vec2<f32>(0.5);
    // The same advected 3D field lifts and sinks broad parts of each cloud,
    // while the kilometer-scale organization bends whole banks. Apply this
    // to the density masks too; warping only the interior left flat layer caps.
    let height_warp = (shape.b - 0.5) * 0.38 + organization.y * 0.20;
    // Cumulus condense at a shared lifting level, so a field has one flat
    // underside and differs only in how far each tower climbs. Warping the
    // whole profile floated each cloud at its own height (±30% of the slab).
    // The warp fades in above the base and keeps full strength in the tops.
    let warp_weight = smoothstep(0.02, 0.40, h);
    let shaped_h = clamp(h + height_warp * convective * warp_weight, 0.0, 1.0);
    // Pure cumulus has no density below its condensation floor or above its
    // cap. Resolve that mask before evaluating the expensive thermal field.
    let floor_noise = weather_bank_noise(xz / 150.0 + vec2<f32>(3.3, 8.1));
    let floor_lift = (floor_noise * 0.7 + shape.r * 0.3) * 0.12 * convective;
    if convective == 1.0 && (shaped_h <= floor_lift || shaped_h >= 1.0) { return 0.0; }
    let lobes = (shape.r - 0.5) * 1.18 + (shape.g - 0.5) * 0.48;
    var field = 0.0;
    if convective > 0.0 {
        // Evaluate the broad mass first: a thermal below mass - .24 cannot
        // influence their smooth maximum. This raises the exact rejection
        // floor where a broad bank already supplies the density.
        let mass = broad_cloud_mass(rotate2(xz, 0.17) + organization * size * 0.80,
            shaped_h, coverage, bank, storm, size, top_y - base_y);
        var envelope = -1.0;
        if convective > 0.001 {
            // For pure cumulus, fields below -.45 are discarded below. The
            // envelope meets the broad mass in ONE smooth max of radius .24.
            // Clamping it to F cannot affect a mass >= F+.24; otherwise their
            // blend stays below F+.24 and still produces zero density. This
            // lets the cell bounds reject empty space before expensive lobes.
            let floor = max(mass - 0.24 - 0.002,
                select(-2.0, -0.45 - lobes - 0.24 - 0.002, convective == 1.0));
            envelope = cumulus_envelope(rotate2(xz, 0.43) + organization * size * 1.35 + warp,
                shaped_h, coverage, bank, storm, size, top_y - base_y, floor);
        }
        // Pure sheets have no contribution from either convective field.
        envelope = cloud_smooth_max(envelope, mass, 0.24);
        field = envelope + lobes;
    }

    // Broad, scalloped decks preserve weather variety without forcing every
    // cumulus through the same flat top and bottom height mask.
    // A sheet still needs varying water content and thickness: the old field
    // saturated everywhere in overcast weather, erasing its lighting structure.
    // Rolls and holes at a scale between the banks and the puff grid, so an
    // overcast deck is a field of uneven lumps rather than one ceiling.
    var rolls = 0.0;
    var deck = 0.0;
    // mix() evaluates both operands. Branch at its exact endpoints so fair
    // weather cumulus does not evaluate discarded sheet noise, and vice versa.
    if convective < 1.0 || storm > 0.001 {
        rolls = weather_bank_noise(xz / vec2<f32>(520.0, 240.0) + vec2<f32>(11.0, -6.0));
        deck = (coverage - 0.38) * 2.0 + (shape.r - 0.5) * 0.85
            + (rolls - 0.55) * 0.9
            - smoothstep(0.35, 0.95, h) * 1.2;
        field = mix(deck, field, convective);
    }
    // Fair-weather cumulus must not acquire a second blanket underneath it.
    if storm > 0.001 {
        let anvil_bank = weather_bank(xz + wind * h * h * shear * 600.0);
        let anvil = smoothstep(0.56, 0.78, shaped_h) * (1.0 - smoothstep(0.86, 1.0, shaped_h))
            * storm * authored_anvil_spread() * smoothstep(0.24, 0.68, anvil_bank) * 1.8;
        field = max(field, deck * 0.65 + anvil - 0.60 - (1.0 - storm) * 2.0);
    }
    if field < -0.45 { return 0.0; }

    let detail_freq = 0.018;
    let detail_lod = footprint_texel_lod(footprint, detail_freq, DETAIL_TEX_SIZE, 5.0);
    // Keep one filtered erosion sample into the middle distance: the old
    // 18 m cutoff left kilometer-scale billows as smooth, identical puffs.
    let detail_keep = 1.0 - smoothstep(8.0, 42.0, footprint);
    var detail = vec4<f32>(0.5);
    if detail_keep > 0.01 && !shadow_lod {
        detail = sample_detail_global(xz + warp, evolving_y, detail_freq, detail_freq, detail_lod);
        // Weight the coarse detail channels; the finest Worley channel made grainy,
        // cauliflower-like fringes that do not belong to the scene's soft lighting.
        // Far clouds cannot resolve the fine Worley channels, so they read as
        // smooth puffs. As the footprint grows, move the erosion onto the
        // coarse billow channel and strengthen it: same texture fetch, larger
        // scale lumps where a pixel is several metres wide.
        let far = smoothstep(1.0, 6.0, footprint);
        let erosion_w = mix(vec3<f32>(0.62, 0.30, 0.08), vec3<f32>(0.90, 0.10, 0.0), far);
        let erosion = (1.0 - dot(detail.rgb, erosion_w))
            * light.weather_globals[1].w * light.weather_globals[2].y * 0.46
            * (1.0 + 1.5 * far);
        field -= erosion * detail_keep;
    }
    // At range the mip-filtered textures above average to a constant, so a
    // far puff is a smooth ellipsoid. Hashed value noise does not depend on
    // the mip level: two scales of billow keep far silhouettes lumpy.
    // One noise serves both the far billow and the condensation floor below.
    if !shadow_lod && footprint > 1.0 {
        let far_w = smoothstep(1.0, 6.0, footprint);
        field -= (0.55 - floor_noise) * 0.60 * far_w;
    }
    let softness = 0.15 + smoothstep(8.0, 90.0, footprint) * 0.22;
    // Sheets thin out over hundreds of metres; a narrow ramp cut stratus into
    // flat, hard-edged cut-outs that read as a map pasted on the sky.
    let sheet_ramp = (1.0 - convective) * 0.55;
    let body = smoothstep(-softness - sheet_ramp * 0.4, softness + 0.14 + sheet_ramp, field);
    // Condensation-floor relief lives at a broader scale than edge turbulence.
    // Driving it from the fine Worley channel flattened the whole underside
    // as its mip level increased, leaving a visible plane across distant decks.
    let density_h = mix(h, shaped_h, convective);
    // A ragged condensation floor: the lifting level wanders by a few percent
    // of the slab, so undersides seen edge-on are not one razor-flat plane.
    // Hashed, not texture-driven: the shape sample is mip-blurred to a
    // constant at range, which is exactly where flat bottoms show.
    var base = smoothstep(floor_lift, floor_lift + 0.07, shaped_h);
    var water_content = 1.0;
    if convective < 1.0 {
        let base_freq = 0.0006;
        let base_lod = footprint_texel_lod(footprint, base_freq, SHAPE_TEX_SIZE, 6.0);
        // This floor lookup contributes only to the sheet component.
        let base_shape = textureSampleLevel(shape_noise_tex, cloud_field_sampler,
            stable_noise_uvw(rotate2(xz, 0.91) + organization * 600.0,
                evolving_y, base_freq, base_freq), base_lod).r;
        let base_height = mix(0.015, 0.13, 1.0 - base_shape) + (1.0 - shape.b) * 0.012
            + storm * (1.0 - bank) * 0.055;
        let deck_base = smoothstep(base_height, base_height + 0.075, density_h);
        base = mix(deck_base, base, convective);
        let sheet_field = (coverage - 0.50) * 1.8 + (shape.r - 0.5) * 1.35
            + (bank - 0.5) * 1.15 + (rolls - 0.5) * 1.25
            - smoothstep(0.18, 0.95, h) * 1.1;
        let sheet_water = mix(0.28, 1.08, smoothstep(-0.08, 0.85, sheet_field));
        water_content = mix(sheet_water, 1.0, convective);
    }
    let cap = 1.0 - smoothstep(mix(0.70, 0.94, shape.g), 1.0, density_h);
    let folds = mix(0.68, 1.08, shape.g) * mix(0.87, 1.0, detail.r);
    return body * base * cap * folds * water_content * mix(0.45, 1.60, authored_cloud_density())
        * smoothstep(0.01, 0.12, weather.low_amount);
}
