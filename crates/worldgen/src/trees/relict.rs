//! Generator 94 biome silhouettes, with explicit bounded voxel geometry.
use super::{BlockId as B, IVec3, TreeForm, TreeProfile, WorldSampleConfig, foliage, hash2, landscape, old_growth};
use glam::Vec3Swizzles;

pub(super) fn profile(cfg: &WorldSampleConfig, t: &mut TreeProfile, s: &landscape::Landscape,
    age: f32, individual: f32) -> bool {
    use landscape::Biome::*;
    if cfg.generator_version < 94 { return false; }
    let biome = if cfg.generator_version >= 95 {
        if hash2(t.cell_x,t.cell_z,cfg.seed.wrapping_add(19567))>=s.ecotone {return false;}
        s.variant_biome
    } else {s.biome};
    let (mut form, conifer, h, r, c) = match biome {
        CycadMarsh => (TreeForm::Cycad,false,2.5+age*2.0,3.0+age*1.2,1.2),
        LostPlateau if individual<0.52 => (TreeForm::Cycad,false,3.0+age*2.0,3.5+age,1.2),
        LostPlateau if old_growth::active(cfg) => (TreeForm::Umbrella,false,16.0+age*13.0,6.0+age*2.5,4.5+age*2.0),
        LostPlateau => (TreeForm::Umbrella,false,12.0+age*7.0,4.5+age*1.5,4.0),
        CloudForest if old_growth::active(cfg) && age >= 0.28 =>
            (TreeForm::Weeping,false,13.0+age*12.0,4.5+age*2.7,5.5+age*3.0),
        CloudForest => (TreeForm::Weeping,false,7.0+age*5.0,3.5+age*1.5,4.0+age),
        RedwoodGrove if cfg.generator_version >= 95 && age < 0.28 =>
            (TreeForm::Redwood,true,7.0+age*28.0,2.0+age*5.0,1.0),
        RedwoodGrove if old_growth::active(cfg) =>
            (TreeForm::Redwood,true,43.0+age*32.0,6.4+age*2.5,1.0),
        RedwoodGrove => (TreeForm::Redwood,true,22.0+age*12.0,4.0+age*1.5,1.0),
        OvergrownGardens => (TreeForm::Weeping,false,5.0+age*3.0,3.0+age,3.0),
        FungalGrove if cfg.generator_version >= 95 && individual < 0.55 => return false,
        FungalGrove => (TreeForm::Fungal,false,4.0+age*3.0,3.0+age*2.0,1.5+age),
        _ => return false,
    };
    if cfg.generator_version >= engine_voxel::TREE_MORPHOLOGY_GENERATOR {
        form = match (biome, form) {
            (CloudForest, TreeForm::Weeping) => TreeForm::CloudCrown,
            (LostPlateau, TreeForm::Umbrella) => TreeForm::PlateauCrown,
            _ => form,
        };
    }
    t.form=form; t.conifer=conifer;
    t.trunk_h=cfg.blocks_for_span(h); t.canopy_radius=cfg.blocks_for_span(r);
    t.canopy_height=cfg.blocks_for_span(c);
    t.branch_count=5; t.branch_len=t.canopy_radius;
    let fork = if super::conifer::tall_redwood(cfg, t) { 0.44 } else { 0.55 };
    t.branch_start_y=t.ground_y+cfg.blocks_for_span(h*fork);
    true
}

pub(super) fn block(cfg: &WorldSampleConfig, t: &TreeProfile, p: IVec3) -> B {
    let v=cfg.voxel_meters();
    let q=(p-IVec3::new(t.anchor_x,t.ground_y,t.anchor_z)).as_vec3()*v;
    let h=t.trunk_h as f32*v;
    let r=t.canopy_radius as f32*v;
    let ch=t.canopy_height as f32*v;
    let d=q.xz().length();
    // Mushroom caps are not foliage and keep their smooth dome.
    let pad=if t.form==TreeForm::Fungal {0.0} else {foliage::pad(cfg)};
    if q.y<=0.0 || q.y>h+ch || d>r+pad { return B::Air; }
    let seed=hash2(t.cell_x,t.cell_z,cfg.seed.wrapping_add(19431));
    let relief=foliage::Relief::of(cfg,t);
    // Signed metres outside a smooth leaf envelope, through relief from generator 130.
    let shell=|outside:f32,half:f32,surface| match relief {
        Some(relief) => relief.shell(q,v,outside,half,surface),
        None => outside<0.0,
    };
    match t.form {
        TreeForm::Cycad => {
            if q.y<=h && d<(0.42+seed*0.18).max(v*0.56) {return B::Log;}
            let a=q.z.atan2(q.x)+seed*std::f32::consts::TAU;
            let radial=d/r;
            let across=(a*5.0).sin().abs()*d;
            let width=0.22+0.38*(radial*std::f32::consts::PI).sin();
            // Fronds start at the trunk tip, arch out and droop at the rim.
            let frond=h+ch*(0.65*(radial*std::f32::consts::PI).sin()-0.85*radial*radial);
            let (wide,thick)=(width.max(v*0.62),0.26f32.max(v*0.62));
            let outside=(across-wide).max((q.y-frond).abs()-thick);
            if shell(outside,wide.min(thick),foliage::FRONDS) {B::Leaves} else {B::Air}
        }
        TreeForm::Fungal => {
            let cap=h+ch*(1.0-(d/r).powi(2));
            if q.y<=cap && q.y>=cap-0.70f32.max(v) {
                if seed>0.5 {B::UmberCap} else {B::AmberCap}
            } else if d<(0.45+seed*0.25).max(v*0.56) && q.y<h+ch-0.3 {B::MushroomStem}
            else {B::Air}
        }
        TreeForm::Redwood => {
            let taper=(1.15+seed*0.30)*(1.0-q.y/(h+ch)*0.76);
            let angle=q.z.atan2(q.x);
            let flare=if cfg.generator_version>=95 {
                (1.0-q.y/2.8).max(0.0)*(0.65+0.25*(angle*3.0+seed*6.28).cos())
            } else {0.0};
            let wood_top=if cfg.generator_version>=95 {h-(1.6+seed*0.6).max(v)} else {h};
            if q.y<wood_top && d<(taper+flare).max(v*0.56) {return B::Log;}
            if q.y<h*0.35 {return B::Air;}
            let along=((q.y-h*0.35)/(h+ch-h*0.35)).clamp(0.0,1.0);
            let spacing=if cfg.generator_version>=95 {2.8+seed*0.8} else {2.4};
            let level=(q.y+seed*2.0)/spacing;
            let tier=level.fract();
            // Rotating, unequal branch fans break the repeated conical rings.
            // The envelope stays inside the existing crown reservation.
            let fan=if cfg.generator_version>=95 {
                0.86+0.14*(angle*3.0+seed*6.28+level.floor()*1.7).cos()
            } else {1.0};
            let crown=r*(1.0-along).powf(0.65)*(0.72+0.28*(1.0-tier))*fan;
            let crown=crown.max(v*0.55);
            if shell(d-crown,crown,foliage::NEEDLES) {B::PineLeaves} else {B::Air}
        }
        _ => B::Air,
    }
}
