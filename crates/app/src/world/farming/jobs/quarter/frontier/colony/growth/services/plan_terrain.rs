use super::*;

pub(super) fn columns(q: &Parcel, hub: [i32; 2], paths: impl Iterator<Item = [i32; 2]>) -> BTreeSet<[i32; 2]> {
    let b = q.bounds();
    let mut out = BTreeSet::from([hub]);
    for x in b[0]..=b[2] { for z in b[1]..=b[3] { out.insert([x, z]); } }
    out.extend(paths);
    out
}

pub(super) fn admit(w: &mut World, columns: &BTreeSet<[i32; 2]>, y: i32) -> Result<(), String> {
    let mut chunks = BTreeSet::new();
    for &[x, z] in columns {
        for dy in [-4, 13] {
            chunks.insert(engine_core::chunk_coord_from_world_block(IVec3::new(x, y + dy, z)).to_array());
        }
    }
    if chunks.len() > 16 { return Err("Service Chunk Budget".into()); }
    if let Some(c) = chunks.into_iter().find(|c| !w.chunks.contains_key(&IVec3::from(*c))) {
        let admitted = w.admit_edit_chunk(c.into());
        return Err(format!("Service Chunk {}: {},{},{}", if admitted {"Admitted"} else {"Unavailable"}, c[0], c[1], c[2]));
    }
    Ok(())
}

pub(super) fn floor(w: &World, [x, z]: [i32; 2], reference: i32) -> Option<[i32; 3]> {
    (reference - 3..=reference + 3).rev().find_map(|y| {
        let at = [x, y, z];
        clear(w, at, 2).then_some(at)
    })
}

pub(super) fn clear(w: &World, at: [i32; 3], height: i32) -> bool {
    let p = IVec3::from(at);
    known(w, p - IVec3::Y).is_some_and(B::is_solid)
        && (0..=height).all(|dy| known(w, p + IVec3::Y * dy) == Some(B::Air))
        && (-1..=height).all(|dy| !protected(w, p + IVec3::Y * dy))
}

pub(super) fn protected(w: &World, p: IVec3) -> bool {
    crate::world::farming::base::overlaps(w, p, p)
}

pub(super) fn paid_floor(w: &World, s: &Site, project: usize, order: usize) -> bool {
    let Some(p) = s.projects.get(project) else { return false; };
    let Some(o) = p.cells.get(order) else { return false; };
    let high = p.cells.iter().map(|o| p.work_layer(*o)).max().unwrap_or(0);
    o.block() == B::Planks && s.project_layers.get(project).is_some_and(|y| *y > high + 1)
        && known(w, o.cell()) == Some(B::Planks)
        && jobs::stand(w, o.cell() + IVec3::Y, w.voxel_scale.meters())
        && (-1..=2).all(|dy| !protected(w, o.cell() + IVec3::Y * dy))
}

pub(super) fn reserved(w: &World, s: &Site, base: usize, columns: &BTreeSet<[i32; 2]>) -> Result<bool, String> {
    let j = &w.farming.jobs;
    let prod = j.production.as_ref().ok_or("Production Unavailable")?;
    if s.projects.len() > 18 || s.projects.iter().map(|p| p.cells.len()).sum::<usize>() > 4096
        || prod.beds.len() > 256 || j.mine.len() > 64 || j.build.len() > 256 || s.timber.len() > 128 {
        return Err("Service Ownership Budget".into());
    }
    let low = columns.first().map_or(0, |p| p[0]);
    let high = columns.last().map_or(0, |p| p[0]);
    let plots: Vec<_> = w.farming.plots.range((low, i32::MIN, i32::MIN)..=(high, i32::MAX, i32::MAX))
        .take(4097).map(|(&(x, _, z), _)| [x, z]).collect();
    if plots.len() > 4096 { return Err("Service Crop Survey Budget".into()); }
    if plots.iter().any(|p| columns.contains(p)) || prod.beds.iter().any(|bed| {
        let margin = if bed.crop == B::Sapling { 5 } else { 0 };
        columns.iter().any(|p| (p[0] - bed.at[0]).abs() <= margin && (p[1] - bed.at[2]).abs() <= margin)
    }) { return Ok(true); }
    let owned: BTreeSet<_> = s.projects.iter().skip(base).flat_map(|p| p.cells.iter().map(|o| o.at)).collect();
    Ok(s.timber.iter().chain(j.mine.iter().map(|(p, _)| p)).any(|p| columns.contains(&point(*p)))
        || s.projects.iter().take(base).flat_map(|p| &p.cells).any(|o| columns.contains(&point(o.at)))
        || j.build.iter().filter(|o| !owned.contains(&o.at)).any(|o| columns.contains(&point(o.at))))
}
