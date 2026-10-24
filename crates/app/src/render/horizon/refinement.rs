//! Bounded source-error scouting; original ring edges remain authoritative.
use super::*;

struct Cell {
    index: usize,
    key: (i32, i32),
    detail: u8,
    step: f32,
    parent: f32,
    high: f32,
    materials: [crate::world::BlockId; 4],
    priority: f32,
}

fn strictly_inside(first: f32, last: f32, middle: i32) -> bool {
    let middle = middle as f32;
    first < middle && middle < last
}

pub(super) fn sample(
    cfg: WorldSampleConfig, center: (i32, i32), focus: Vec3, pixels: f32, reach: f32,
    cancel: &AtomicBool, samples: &mut Samples, old: &mut Samples,
) -> Option<HashMap<usize, (i32, i32)>> {
    let mut chosen = HashMap::new();
    if !canonical_source() || cfg.world_type == crate::world::WorldType::Superflat {
        return Some(chosen);
    }
    let scale = cfg.voxel_meters();
    let sea = cfg.water_level() as f32 + 1.0 - crate::mesh::WATER_SURFACE_INSET;
    let mut cells = Vec::new();
    let mut work = Vec::new();
    for level in 0..LEVELS {
        let step = ring_step(level);
        if step / scale < 2.0 { continue; }
        let xa = ring_axis(center.0, level, scale)?;
        let za = ring_axis(center.1, level, scale)?;
        let lo = HALF / 2;
        let hi = HALF * 3 / 2;
        for z in 0..SIDE - 1 {
            if cancel.load(Ordering::Relaxed) { return None; }
            for x in 0..SIDE - 1 {
                if !cell_in_domain(x,z,step,reach) {continue;}
                if level > 0 {
                    if (lo..hi).contains(&x) && (lo..hi).contains(&z) { continue; }
                    // Preserve every existing fine/coarse transition triangle.
                    if ((lo..hi).contains(&x) && (z + 1 == lo || z == hi))
                        || ((lo..hi).contains(&z) && (x + 1 == lo || x == hi)) { continue; }
                }
                let keys = [(xa[x].1,za[z].1),(xa[x+1].1,za[z].1),
                    (xa[x].1,za[z+1].1),(xa[x+1].1,za[z+1].1)];
                let [Some(a),Some(b),Some(c),Some(d)] = keys.map(|key|samples.get(&key)) else {continue;};
                let sx=i64::from(keys[0].0)+i64::from(keys[3].0);
                let sz=i64::from(keys[0].1)+i64::from(keys[3].1);
                if sx & 1 != 0 || sz & 1 != 0 {continue;}
                let key=((sx/2) as i32,(sz/2) as i32);
                // A valid coarse lattice does not imply a representable
                // midpoint at large coordinates. Retain its original cell.
                if !strictly_inside(xa[x].0,xa[x+1].0,key.0)
                    || !strictly_inside(za[z].0,za[z+1].0,key.1) {continue;}
                let parent=if diagonal_a_to_d(a.0,b.0,c.0,d.0) {(a.0+d.0)*0.5}else{(b.0+c.0)*0.5};
                cells.push(Cell{index:level*SIDE*SIDE+z*SIDE+x,key,detail:sample_detail(level),step,
                    parent,high:a.0.max(b.0).max(c.0).max(d.0),materials:[a.1,b.1,c.1,d.1],priority:0.0});
                // ALWAYS scout with the same fixed point policy, regardless of
                // retained exact samples. Cache warmth cannot change selection.
                work.push((key,1,8.0));
            }
        }
    }
    let scout_count=cells.len();
    debug_assert!(scout_count <= LEVELS*(SIDE-1)*(SIDE-1));
    let mut scouts=Samples::new();
    evaluate_samples(cfg,cancel,work,&mut scouts)?;
    for cell in &mut cells {
        let &(height,material,_)=scouts.get(&cell.key)?;
        if !height.is_finite() || cell.high.max(height) < sea {continue;}
        let point=Vec3::new(cell.key.0 as f32,height,cell.key.1 as f32)*scale;
        let distance=point.distance(focus).max(cell.step);
        let error=(height-cell.parent).abs()*scale*pixels/distance;
        let peak=(height-cell.high).max(0.0)*scale*pixels/distance;
        let distinct=cell.materials.iter().all(|&m|m!=material)
            || ((height<sea)!=(cell.parent<sea));
        let distribution=if distinct {cell.step*pixels/distance*0.25}else{0.0};
        cell.priority=error.max(distribution)+peak*2.0;
    }
    drop(scouts);
    cells.retain(|cell|cell.priority.is_finite() && cell.priority>=0.35);
    cells.sort_unstable_by(|a,b|b.priority.total_cmp(&a.priority).then(a.index.cmp(&b.index)));
    let mut work=Vec::new();
    // Exact (policy 0) samples cost milliseconds each and keep the original
    // budget. Far rings use the cheap fixed evaluator (microseconds), so they
    // get their own, larger budget instead of competing for the exact one:
    // before, ~3.5k of 16k refinements reached the 6-24 km mountain ranges.
    let (mut exact_left,mut cheap_left)=(MAX_REFINEMENT_SAMPLES,MAX_FAR_REFINEMENT_SAMPLES);
    let budgeted=cells.iter().filter(|cell| {
        let left=if cell.detail==0 {&mut exact_left} else {&mut cheap_left};
        if *left==0 {return false;}
        *left-=1;
        true
    });
    for cell in budgeted {
        if let Some(existing)=samples.get(&cell.key) {
            if existing.2!=cell.detail {continue;}
        } else if let Some(value)=old.remove(&cell.key).filter(|v|v.2==cell.detail) {
            samples.insert(cell.key,value);
        } else {work.push((cell.key,cell.detail,cell.step));}
        chosen.insert(cell.index,cell.key);
    }
    drop(cells);
    evaluate_samples(cfg,cancel,work,samples)?;
    println!("[horizon-refinement] scouted={scout_count} selected={} limit={MAX_REFINEMENT_SAMPLES} far_limit={MAX_FAR_REFINEMENT_SAMPLES} focus_m={focus:?} focal_pixels={pixels:.1}",chosen.len());
    Some(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrepresentable_midpoint_keeps_coarse_cell_eligible() {
        assert!(strictly_inside(0.0,4.0,2));
        assert_eq!(33_554_434i32 as f32,33_554_432.0);
        assert!(!strictly_inside(33_554_432.0,33_554_436.0,33_554_434));
        assert!(cell_in_domain(HALF,HALF,ring_step(0),domain_reach(FAR_PLANE as u32)),"coarse coverage is independent of refinement eligibility");
    }
}
