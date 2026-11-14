// Architecture uses quiet, connected 5 cm clusters and directional sawn grain.
// Natural limestone and living bark retain their own surface families.
fn architecture_surface(base: vec3<f32>, p: vec3<f32>, n: vec3<f32>, kind: i32, fp: f32) -> vec3<f32> {
    var uv = p.xz;
    if abs(n.x) > 0.5 { uv = p.zy; }
    else if abs(n.z) > 0.5 { uv = p.xy; }
    let t = floor(uv * 20.0);
    let detail = surface_detail_weight(fp * 20.0);
    if kind == 64 {
        let cluster = floor(t / vec2<f32>(9.0, 5.0));
        let shade = select(0.975, 1.01, surface_hash(cluster) > 0.6);
        // Sparse two-pixel trowel marks; the broad wall stays a single warm value.
        let cell = t - cluster * vec2<f32>(9.0, 5.0);
        let mark = cell.x >= 3.0 && cell.x <= 5.0 && cell.y == 2.0
            && surface_hash(cluster + vec2<f32>(31.0, 17.0)) > 0.87;
        return base * mix(0.99, shade - select(0.0, 0.035, mark), detail);
    }
    if kind == 68 {
        let row = floor(t.y / 10.0);
        let shifted = t.x + (row - floor(row / 2.0) * 2.0) * 10.0;
        let stone = vec2<f32>(floor(shifted / 20.0), row);
        let cell = vec2<f32>(shifted - floor(shifted / 20.0) * 20.0, t.y - row * 10.0);
        let mortar = cell.x < 1.0 || cell.y < 1.0;
        let id = surface_hash(stone + vec2<f32>(11.0, 37.0));
        let bevel = cell.y == 1.0 || (cell.x == 1.0 && cell.y < 8.0);
        let lower_edge = cell.y == 9.0 || cell.x == 19.0;
        // A few connected chips interrupt the cut edge without stippling the face.
        let chip_x = 4.0 + floor(id * 11.0);
        let chip = id > 0.73 && cell.x >= chip_x && cell.x <= chip_x + 2.0
            && (cell.y == 1.0 || (cell.y == 2.0 && cell.x == chip_x + 1.0));
        let shade = select(select(0.96, 1.0, id > 0.28), 1.035, id > 0.78);
        var value = shade + select(0.0, 0.04, bevel) - select(0.0, 0.035, lower_edge);
        value = select(value, shade * 0.82, chip);
        value = select(value, 0.72, mortar);
        return base * mix(0.96, value, detail);
    }
    var axis = vec3<f32>(0.0, 1.0, 0.0);
    var grain = vec2<f32>(p.x + p.z, p.y);
    if kind == 66 { axis = vec3<f32>(1.0, 0.0, 0.0); grain = vec2<f32>(p.y + p.z, p.x); }
    if kind == 67 { axis = vec3<f32>(0.0, 0.0, 1.0); grain = vec2<f32>(p.x + p.y, p.z); }
    let g = floor(grain * 20.0);
    let beam = floor(g.x / 10.0);
    let section = floor(g.y / 36.0);
    let id = surface_hash(vec2<f32>(beam, section) + vec2<f32>(7.0, 29.0));
    let local = g - vec2<f32>(beam * 10.0, section * 36.0);
    // Long connected streaks taper in steps; broad sawn faces stay quiet.
    let streak_x = 2.0 + floor(id * 5.0);
    let streak = (local.x == streak_x && local.y >= 5.0 && local.y <= 27.0)
        || (local.x == streak_x + 1.0 && local.y >= 19.0 && local.y <= 30.0);
    // Per-column fibre value keeps the grain readable on a quiet sawn face.
    let fibre = (surface_hash(vec2<f32>(g.x, beam + 13.0)) - 0.5) * 0.09;
    var value = select(0.95, 1.04, id > 0.52) + fibre - select(0.0, 0.14, streak);
    let knot = abs(local - vec2<f32>(5.0, 15.0));
    if id > 0.82 && knot.x + floor(knot.y / 2.0) <= 2.0 {
        value = select(0.86, 0.74, knot.x + knot.y < 1.0);
    }
    if abs(dot(n, axis)) > 0.5 {
        let end_cell = floor(t / 10.0);
        let end_id = surface_hash(end_cell + vec2<f32>(41.0, 13.0));
        let centre = vec2<f32>(4.0 + floor(end_id * 3.0), 5.0);
        let offset = t - end_cell * 10.0 - centre;
        let cross = abs(offset);
        // Octagonal pixel rings, with an off-centre heart and one short split.
        let radius = max(cross.x, cross.y) + min(cross.x, cross.y) * 0.5;
        let ring = floor(radius);
        value = select(0.90, 1.02, ring - floor(ring / 3.0) * 3.0 > 0.0);
        if radius < 1.0 { value = 0.84; }
        if end_id > 0.65 && offset.x == 0.0 && offset.y >= 3.0 { value = 0.80; }
    }
    surface_finish = mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0 + fibre * 1.5 + select(0.0, 0.12, streak), 1.0, 0.0), detail);
    return base * mix(0.98, value, detail);
}
