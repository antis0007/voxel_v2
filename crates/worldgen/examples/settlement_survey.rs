//! Read-only settlement coordinates for visiting actual generated places.
//! Usage: settlement_survey [seed] [generator] [x_m] [z_m] [radius_m]
use engine_core::VoxelScalePreset;
use engine_worldgen::{WorldSampleConfig, WorldType, civilization};
use glam::Vec3;
use std::{error::Error, str::FromStr, time::Instant};

fn arg<T: FromStr>(args: &mut impl Iterator<Item = String>, fallback: T) -> Result<T, Box<dyn Error>>
where T::Err: Error + 'static {
    args.next().map_or(Ok(fallback), |v| Ok(v.parse()?))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let cfg = WorldSampleConfig {
        seed: arg(&mut args, 74)?, generator_version: arg(&mut args, 286)?,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter, planet: None,
    };
    let centre = Vec3::new(arg(&mut args, 323328.0)?, 0.0, arg(&mut args, 2304.0)?);
    let radius: f32 = arg(&mut args, 255.0)?;
    if !centre.is_finite() || !radius.is_finite() || !(1.0..=1536.0).contains(&radius) {
        return Err("Use finite coordinates and a radius from 1 to 1536 metres".into());
    }
    println!("id,x_m,y_m,z_m,households,livelihood,peer,connected");
    let started = Instant::now();
    let mut sites = civilization::settlements_near(&cfg, centre, radius);
    let surveyed = if sites.iter().all(|s| s.history.households == 0) && radius < 767.0 {
        sites = civilization::settlements_near(&cfg, centre, 767.0);
        767.0
    } else { radius };
    sites.sort_by(|a, b| a.center.distance_squared(centre).total_cmp(&b.center.distance_squared(centre)));
    for site in &sites {
        let peer = site.id ^ 8;
        let linked = sites.iter().find(|s| s.id == peer)
            .map(|other| civilization::linked(&cfg, *site, *other).to_string()).unwrap_or_default();
        println!("{},{},{},{},{},{:?},{},{}", site.id, site.center.x, site.center.y,
            site.center.z, site.history.households, site.history.livelihood, peer, linked);
    }
    eprintln!("[settlement-survey] seed={} generator={} radius_m={} sites={} inhabited={} elapsed_s={:.3}",
        cfg.seed, cfg.generator_version, surveyed, sites.len(),
        sites.iter().filter(|s| s.history.households > 0).count(), started.elapsed().as_secs_f64());
    Ok(())
}
