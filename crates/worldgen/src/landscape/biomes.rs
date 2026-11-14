//! Literary landscapes are ecological variants of existing relief, not isolated
//! structures stamped into it. Shared by exact columns and horizon sampling.
use super::*;
mod habitat;

pub(super) fn blend(cfg: &WorldSampleConfig, x: f32, z: f32, h: f32, slope: f32,
    heat: f32, wet: f32, base: Biome) -> (Biome,f32) {
    if cfg.generator_version >= 95 && crate::geo::integrated(cfg) {
        habitat::blend(cfg,x,z,base)
    } else {
        let selected=select(cfg,x,z,h,slope,heat,wet,base);
        (selected, if selected==base {0.0} else {1.0})
    }
}

pub(super) fn select(cfg: &WorldSampleConfig, x: f32, z: f32, h: f32, slope: f32,
    heat: f32, wet: f32, base: Biome) -> Biome {
    use Biome::*;
    if cfg.generator_version < 94 || x.hypot(z) < 130.0 { return base; }
    let u = x*0.91-z*0.41;
    let v = x*0.41+z*0.91;
    let warp = (value_noise2(u/95.0,v/95.0,cfg.seed.wrapping_add(19401))-0.5)*65.0;
    let region = value_noise2((u+warp)/280.0,(v-warp)/280.0,cfg.seed.wrapping_add(19403));
    if base == Coast && heat < 0.48 && region > 0.52 { return LastShore; }
    if matches!(base, Ocean | Coast | Alpine | Badlands | Desert | Tundra) { return base; }
    if wet > 0.62 && heat > 0.28 && h < 130.0 && region > 0.72 { return FungalGrove; }
    if wet > 0.65 && heat > 0.50 && h < 75.0 && slope < 0.18 && region > 0.40 { return CycadMarsh; }
    if (60.0..190.0).contains(&h) && heat > 0.45 && wet > 0.50 && region > 0.43 { return LostPlateau; }
    if h > 85.0 && wet > 0.62 && (0.24..0.58).contains(&heat) && region > 0.40 { return CloudForest; }
    if h > 20.0 && (0.28..0.58).contains(&heat) && (0.50..0.80).contains(&wet) && region < 0.40 { return RedwoodGrove; }
    if h < 90.0 && slope < 0.28 && (0.38..0.72).contains(&heat)
        && (0.42..0.78).contains(&wet) && region > 0.52 { return OvergrownGardens; }
    base
}

/// Restore characteristic soils after generic geology; channel, lake and
/// settlement passes still own their ground and retain placement priority.
pub(super) fn surface(cfg: &WorldSampleConfig, x: i32, z: i32, s: &Landscape,
    top: &mut BlockId, under: &mut BlockId) {
    if cfg.generator_version < 94 || s.slope > 0.55 || matches!(*top, BlockId::SnowGrass | BlockId::Ice) { return; }
    let biome = if cfg.generator_version >= 95 {
        let t=value_noise2(cfg.world_x(x)/7.0,cfg.world_z(z)/7.0,cfg.seed.wrapping_add(19541));
        if t>=s.ecotone {return;}
        s.variant_biome
    } else {s.biome};
    let patch = value_noise2(cfg.world_x(x)/18.0,cfg.world_z(z)/18.0,cfg.seed.wrapping_add(19411));
    let pair = match biome {
        Biome::LostPlateau => (if patch>0.76 {BlockId::Limestone} else {BlockId::Grass},BlockId::Limestone),
        Biome::CycadMarsh => (if patch>0.60 {BlockId::GroveMoss} else {BlockId::Mud},BlockId::Clay),
        Biome::CloudForest => (if patch>0.24 {BlockId::GroveMoss} else {BlockId::Dirt},BlockId::Dirt),
        Biome::RedwoodGrove => (if patch>0.72 {BlockId::GroveMoss} else {BlockId::Dirt},BlockId::Dirt),
        Biome::OvergrownGardens => (if patch>0.78 {BlockId::GroveMoss} else {BlockId::Grass},BlockId::Dirt),
        Biome::FungalGrove => (if patch>0.76 {BlockId::GroveMoss} else {BlockId::FungalSoil},BlockId::Dirt),
        Biome::LastShore => (if patch>0.83 {BlockId::GroveMoss} else if patch<0.22 {BlockId::Sandstone} else {BlockId::Clay},BlockId::Sandstone),
        _ => return,
    };
    (*top,*under) = pair;
}
