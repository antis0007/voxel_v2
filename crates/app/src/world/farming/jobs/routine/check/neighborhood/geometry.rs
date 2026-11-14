use super::*;

pub(super) fn census(w: &World) -> BTreeMap<[i32;3],B> {
    let base=w.farming.jobs.stockpile.map(IVec3::from).unwrap_or(IVec3::ZERO)-IVec3::Y;
    let mut cells=BTreeMap::new();
    for x in -24..=24 {for z in -24..=24 {for y in 0..=10 {
        let at=base+IVec3::new(x,y,z);let block=w.get(at);
        if matches!(block,B::Planks|B::Chest) {cells.insert(at.to_array(),block);}
    }}}
    cells
}

pub(super) fn check(w: &World, initial: &BTreeMap<[i32;3],B>) -> Result<usize> {
    let current=census(w);
    ensure!(initial.iter().all(|(at,b)|current.get(at)==Some(b)), "Existing timber geometry changed");
    let paid:BTreeMap<_,_>=site(w)?.projects.iter().flat_map(|p|&p.cells).map(|o|(o.at,o.block())).collect();
    for (at,b) in &current {
        ensure!(initial.get(at)==Some(b)||paid.get(at)==Some(b), "Unfunded timber geometry at {at:?}: {b:?}");
    }
    Ok(current.len()-initial.len())
}
