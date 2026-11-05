use super::resident_lod;

#[test]
fn an_approaching_chunk_refines_at_once() {
    // Resident LOD 1 at the first full-detail ring: no coarse mesh inside it.
    assert_eq!(resident_lod(1, 0, 0), 0);
    assert_eq!(resident_lod(2, 1, 1), 1);
}

#[test]
fn a_receding_chunk_keeps_detail_for_one_ring() {
    // Desired LOD 1 one ring past the band edge, inner neighbour still LOD 0.
    assert_eq!(resident_lod(0, 1, 0), 0);
    // Two rings past, the inner neighbour wants LOD 1 too: coarsen.
    assert_eq!(resident_lod(0, 1, 1), 1);
}

#[test]
fn a_matching_mesh_stays() {
    assert_eq!(resident_lod(1, 1, 0), 1);
    assert_eq!(resident_lod(0, 0, 0), 0);
}
