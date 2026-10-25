//! Debug preview: a ray-cast picture of one room of a planned complex, for
//! checking room dressing without starting the game.
use super::*;

/// `V2_VIEW=seed,quarters,kind,version[,out]` writes a raw RGB picture of
/// the first room of that kind (`Debug` name, or a damage name) from a
/// corner toward the far wall. `quarters` is 4 for 1 m, 2 for 0.5 m, 1 for
/// 0.25 m. Convert with any tool that reads `w h` then RGB bytes.
#[test]
#[ignore = "debug picture"]
fn view_room() {
    let a: Vec<String> = std::env::var("V2_VIEW").unwrap().split(',').map(str::to_owned).collect();
    let scale = match a[1].as_str() {
        "4" => VoxelScalePreset::OneMeter,
        "2" => VoxelScalePreset::HalfMeter,
        _ => VoxelScalePreset::QuarterMeter,
    };
    let version: u32 = a[3].parse().unwrap();
    let c = WorldSampleConfig { generator_version: version, ..cfg(a[0].parse().unwrap(), scale) };
    let want = a[2].as_str();
    let (_, d) = planned(&c, 12)
        .into_iter()
        .find(|(_, d)| d.rooms.iter().any(|r| format!("{:?}", r.kind) == want || format!("{:?}", r.damage) == want))
        .unwrap();
    let r = d.rooms.iter().find(|r| format!("{:?}", r.kind) == want || format!("{:?}", r.damage) == want).unwrap();
    println!("{:?} {:?} {:?} {:?} w {} d {} h {}", d.job, r.kind, r.damage, r.ceiling, r.w(), r.d(), r.h);
    let v = c.voxel_meters();
    let height = c.blocks_for_height(1000.0);
    let (_, cz) = r.centre();
    let eye = Vec3::new(r.x0 + 2.0, r.floor + 1.7, cz + 0.3);
    let (qx, qz) = collapse::centre(r);
    let (eye, target) = if r.damage == Damage::Collapsed {
        let (cx, _) = r.centre();
        let ex = if qx < r.w() * 0.5 { r.x1 - 1.5 } else { r.x0 + 1.5 };
        (Vec3::new(ex, r.floor + 1.7, cz + 0.3), Vec3::new(r.x0 + qx, r.floor + r.h * 0.7, r.z0 + qz) + (Vec3::new(cx, 0.0, cz) - Vec3::new(r.x0 + qx, 0.0, r.z0 + qz)) * 0.0)
    } else {
        (eye, Vec3::new(r.x1, r.floor + r.h * 0.45, r.z0 + r.d() * 0.12))
    };
    let fwd = (target - eye).normalize();
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let (w, h) = (480usize, 300usize);
    let mut px = Vec::with_capacity(w * h * 3);
    let mut cache = std::collections::HashMap::new();
    println!("eye {eye} block {:?} contains {} min {} max {}", d.sample(&c, eye, height), d.contains(eye), d.min, d.max);
    let mut at = |p: IVec3| *cache.entry(p).or_insert_with(|| d.sample(&c, (p.as_vec3() + Vec3::splat(0.5)) * v, height).unwrap_or(B::Stone));
    for j in 0..h {
        for i in 0..w {
            let sx = (i as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let sy = 1.0 - (j as f32 + 0.5) / h as f32 * 2.0;
            let dir = (fwd + right * sx * 0.9 + up * sy * 0.56).normalize();
            px.extend_from_slice(&trace(&mut at, eye / v, dir, 400));
        }
    }
    let out = a.get(4).cloned().unwrap_or_else(|| "../../target/ruin_view.rgb".to_owned());
    let mut bytes = format!("{w} {h}\n").into_bytes();
    bytes.extend(px);
    std::fs::write(&out, bytes).unwrap();
    println!("wrote {out}");
}

/// Voxel DDA from `o` (voxel units) along `dir`: the first non-air block,
/// shaded by face and distance.
fn trace(at: &mut impl FnMut(IVec3) -> B, o: Vec3, dir: Vec3, steps: usize) -> [u8; 3] {
    let mut p = o.floor().as_ivec3();
    let step = dir.signum().as_ivec3();
    let inv = Vec3::ONE / dir.abs().max(Vec3::splat(1e-6));
    let frac = o - o.floor();
    let mut t = Vec3::new(
        if dir.x > 0.0 { 1.0 - frac.x } else { frac.x } * inv.x,
        if dir.y > 0.0 { 1.0 - frac.y } else { frac.y } * inv.y,
        if dir.z > 0.0 { 1.0 - frac.z } else { frac.z } * inv.z,
    );
    let mut face = 0;
    let mut dist = 0.0;
    for _ in 0..steps {
        let b = at(p);
        if b != B::Air && b != B::Water {
            let base = b.material().render.color.base;
            let shade = match face { 1 => if step.y < 0 { 1.0 } else { 0.45 }, 0 => 0.78, _ => 0.62 };
            let glow = if matches!(b, B::Torch | B::GlowCap | B::Lava | B::Crystal | B::StarGlass) { 1.6 } else { 1.0 };
            let fog = (-dist / 160.0f32).exp();
            let c = |k: usize| ((base[k] * shade * glow * fog).clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            return [c(0), c(1), c(2)];
        }
        if t.x < t.y && t.x < t.z {
            p.x += step.x;
            dist = t.x;
            t.x += inv.x;
            face = 0;
        } else if t.y < t.z {
            p.y += step.y;
            dist = t.y;
            t.y += inv.y;
            face = 1;
        } else {
            p.z += step.z;
            dist = t.z;
            t.z += inv.z;
            face = 2;
        }
    }
    [0, 0, 0]
}

/// `V2_NEAR=seed` lists the rooms of complexes near (300, 0), as the review
/// cameras search them, at each scale.
#[test]
#[ignore = "debug dump"]
fn near_review() {
    let seed: u32 = std::env::var("V2_NEAR").unwrap().parse().unwrap();
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let c = cfg(seed, scale);
        let i0 = (300.0 / crate::dungeon::CELL).round() as i32;
        for ring in 0..12i32 {
            for z in -ring..=ring {
                for x in i0 - ring..=i0 + ring {
                    if (x - i0).abs().max(z.abs()) != ring {
                        continue;
                    }
                    if let Some(d) = crate::dungeon::complex_at(&c, x, z) {
                        let rooms: Vec<_> = d.rooms.iter().map(|r| format!("{:?}/{:?}", r.kind, r.damage)).collect();
                        println!("{scale:?} ({x},{z}) {:?}: {}", d.job, rooms.join(" "));
                    }
                }
            }
        }
    }
}
