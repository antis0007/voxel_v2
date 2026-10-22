//! Remap spatial draw ranges into bounded buffers on the horizon worker.
//! Referenced vertices stay shared within a page; no triangle is dropped.
use super::batches::DrawBatch;
use crate::mesh::{ChunkMesh, Vertex3};
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Page {
    pub mesh: ChunkMesh,
    pub batches: Vec<DrawBatch>,
}

pub(super) fn pack(
    mesh: ChunkMesh,
    batches: Vec<DrawBatch>,
    limit: u64,
    total_limit: usize,
    cancel: &AtomicBool,
) -> Option<Vec<Page>> {
    let vertex_limit =
        (limit / std::mem::size_of::<Vertex3>() as u64).min(u32::MAX as u64) as usize;
    let index_limit = (limit / 4).min(u32::MAX as u64) as usize;
    if vertex_limit < 3 || index_limit < 3 || super::mesh_bytes(&mesh)>total_limit {
        return None;
    }
    if mesh.vertices.len() <= vertex_limit && mesh.indices.len() <= index_limit {
        return Some(vec![Page { mesh, batches }]);
    }
    let mut pages = Vec::new();
    let mut page = Page {
        mesh: ChunkMesh::empty(),
        batches: Vec::new(),
    };
    let mut remap = vec![u32::MAX; mesh.vertices.len()];
    let mut packed_bytes=0usize;
    for batch in batches {
        let mut start = page.mesh.indices.len() as u32;
        for tri in
            mesh.indices[batch.indices.start as usize..batch.indices.end as usize].chunks_exact(3)
        {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            let additional = tri
                .iter()
                .filter(|&&i| remap[i as usize] == u32::MAX)
                .count();
            if page.mesh.vertices.len() + additional > vertex_limit
                || page.mesh.indices.len() + 3 > index_limit
            {
                if start < page.mesh.indices.len() as u32 {
                    let mut range = batch.clone();
                    range.indices = start..page.mesh.indices.len() as u32;
                    page.batches.push(range);
                }
                pages.push(page);
                page = Page {
                    mesh: ChunkMesh::empty(),
                    batches: Vec::new(),
                };
                remap.fill(u32::MAX);
                start = 0;
            }
            for &source in tri {
                let local = &mut remap[source as usize];
                if *local == u32::MAX {
                    packed_bytes=packed_bytes.checked_add(std::mem::size_of::<Vertex3>())?;
                    if packed_bytes>total_limit { return None; }
                    *local = page.mesh.vertices.len() as u32;
                    page.mesh.vertices.push(mesh.vertices[source as usize]);
                }
                page.mesh.indices.push(*local);
                packed_bytes=packed_bytes.checked_add(4)?;
                if packed_bytes>total_limit { return None; }
            }
        }
        if start < page.mesh.indices.len() as u32 {
            let mut range = batch;
            range.indices = start..page.mesh.indices.len() as u32;
            page.batches.push(range);
        }
    }
    if !page.mesh.indices.is_empty() {
        pages.push(page);
    }
    Some(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_duplication_is_admitted_against_the_total_publication_budget() {
        let make=||ChunkMesh { vertices:[[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,0.0,1.0],[1.0,0.0,1.0]].map(|pos|Vertex3 { pos,normal:[0.0,1.0,0.0],color:[0.5;3],surface:[0.9,0.0,3.0,0.0],ao_strength:-1.0 }).to_vec(),indices:vec![0,2,1,1,2,3] };
        let cancel=AtomicBool::new(false);
        let mut mesh=make();let budget=super::super::mesh_bytes(&mesh);
        let batches=super::super::batches::partition(&mut mesh,1.0,(0,0));
        assert!(pack(mesh,batches,168,budget,&cancel).is_none());
        let mut mesh=make();let batches=super::super::batches::partition(&mut mesh,1.0,(0,0));
        let pages=pack(mesh,batches,168,360,&cancel).unwrap();
        assert_eq!(pages.len(),2);
        assert_eq!(pages.iter().map(|p|super::super::mesh_bytes(&p.mesh)).sum::<usize>(),360);
        assert_eq!(pages.iter().map(|p|p.mesh.indices.len()).sum::<usize>(),6);
    }
}
