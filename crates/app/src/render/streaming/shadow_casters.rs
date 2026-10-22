//! Optional exact proofs for resident shadow inputs. Water geometry is excluded
//! only when the entire indexed triangle is clipped by the shadow vertex shader.
//! Proof work runs on mesh workers; publication compares identities, not arrays.
use crate::mesh::{ChunkMesh, Vertex3};
use glam::IVec3;
use std::{
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

// One allowance covers cached, queued, working and retired-but-pinned proofs.
// Exhaustion never delays meshing or evicts world/GPU data: it means redraw.
const BYTE_LIMIT: usize = 128 * 1024 * 1024;
const PROOF_LIMIT: usize = 1024 * 1024;
const INDEX_LIMIT: usize = 262_144;
const CACHE_ENTRIES: usize = 512;
static LEASED_BYTES: AtomicUsize = AtomicUsize::new(0);
type Corner = [u32; 6];

struct Lease(usize);
impl Lease {
    fn reserve(bytes: usize) -> Option<Self> {
        LEASED_BYTES
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(bytes).filter(|&n| n <= BYTE_LIMIT)
            })
            .ok()?;
        Some(Self(bytes))
    }
    fn grow(&mut self, bytes: usize) -> Option<()> {
        if bytes > self.0 {
            let extra = Self::reserve(bytes - self.0)?;
            self.0 = bytes;
            std::mem::forget(extra);
        }
        Some(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        LEASED_BYTES.fetch_sub(self.0, Ordering::Relaxed);
    }
}

struct Proof {
    corners: Vec<Corner>,
    // Declared last so the payload is freed before its charge is released.
    _lease: Lease,
}
struct Installed {
    coord: IVec3,
    lod: u8,
    proof: Arc<Proof>,
    _lease: Lease,
}

/// Strong identity pins prevent address reuse while a worker references an old
/// installed baseline. A new identity is issued at every successful install.
#[derive(Clone)]
pub(super) struct Baseline(Arc<Installed>);

#[derive(Default)]
pub(super) struct Prepared {
    proof: Option<Arc<Proof>>,
    equal_to: Option<Baseline>,
}

/// Retains only inputs consumed by shadow_shader.wgsl. No hash authorizes reuse.
fn corner(vertex: &Vertex3) -> Option<Corner> {
    if !vertex
        .pos
        .iter()
        .chain(vertex.surface[1..3].iter())
        .all(|v| v.is_finite())
        || !vertex.ao_strength.is_finite()
    {
        return None;
    }
    Some([
        vertex.pos[0].to_bits(),
        vertex.pos[1].to_bits(),
        vertex.pos[2].to_bits(),
        vertex.surface[1].to_bits(),
        vertex.surface[2].to_bits(),
        u32::from(vertex.ao_strength < 0.0),
    ])
}

fn visit(
    mesh: &ChunkMesh,
    cancel: &AtomicBool,
    mut emit: impl FnMut(Corner) -> bool,
) -> Option<usize> {
    if mesh.indices.len() > INDEX_LIMIT || !mesh.indices.len().is_multiple_of(3) {
        return None;
    }
    let mut count = 0usize;
    for (index, tri) in mesh.indices.chunks_exact(3).enumerate() {
        if index % 256 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        let a = mesh.vertices.get(tri[0] as usize)?;
        let b = mesh.vertices.get(tri[1] as usize)?;
        let c = mesh.vertices.get(tri[2] as usize)?;
        let corners = [corner(a)?, corner(b)?, corner(c)?];
        // Exact existing keys only. Other packed water keys conservatively
        // remain in the proof; mixed triangles retain ALL corners and winding.
        if [a, b, c]
            .iter()
            .all(|v| v.surface[2] == 8.0 || v.surface[2] == 72.0)
        {
            continue;
        }
        for value in corners {
            if !emit(value) {
                return None;
            }
            count += 1;
        }
    }
    (!cancel.load(Ordering::Relaxed)).then_some(count)
}

impl Prepared {
    /// Final immutable mesh only, after water encoding. Called off-thread.
    pub(super) fn from_mesh(
        mesh: &ChunkMesh,
        baseline: Option<&Baseline>,
        cancel: &AtomicBool,
    ) -> Self {
        // Count and compare together: unchanged water updates need one scan,
        // while changed meshes still get a complete validation before capture.
        let mut expected = baseline.map(|b| b.0.proof.corners.iter());
        let mut equal = expected.is_some();
        let Some(count) = visit(mesh, cancel, |value| {
            if let Some(corners) = expected.as_mut() {
                equal &= corners.next() == Some(&value);
            }
            true
        }) else {
            return Self::default();
        };
        if let Some(baseline) = baseline
            && equal && baseline.0.proof.corners.len() == count {
                return Self {
                    proof: Some(baseline.0.proof.clone()),
                    equal_to: Some(baseline.clone()),
                };
            }
        Self {
            proof: Self::capture(mesh, count, cancel),
            equal_to: None,
        }
    }

    fn capture(mesh: &ChunkMesh, count: usize, cancel: &AtomicBool) -> Option<Arc<Proof>> {
        let payload = count.checked_mul(size_of::<Corner>())?;
        if payload > PROOF_LIMIT {
            return None;
        }
        let overhead = size_of::<Proof>() + 2 * size_of::<usize>();
        let mut lease = Lease::reserve(payload.checked_add(overhead)?)?;
        let mut corners = Vec::new();
        corners.try_reserve_exact(count).ok()?;
        lease.grow(
            corners
                .capacity()
                .checked_mul(size_of::<Corner>())?
                .checked_add(overhead)?,
        )?;
        visit(mesh, cancel, |value| {
            corners.push(value);
            true
        })?;
        Some(Arc::new(Proof {
            corners,
            _lease: lease,
        }))
    }
}

struct Entry {
    baseline: Baseline,
    used: u64,
}

#[derive(Default)]
pub(super) struct Cache {
    entries: Vec<Entry>,
    clock: u64,
    // Fixed-capacity lookup bookkeeping is charged separately from each blob.
    lookup_lease: Option<Lease>,
}

impl Cache {
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn baseline(&mut self, coord: IVec3, lod: u8) -> Option<Baseline> {
        self.clock = self.clock.wrapping_add(1);
        let baseline = self
            .entries
            .iter_mut()
            .find(|e| e.baseline.0.coord == coord && e.baseline.0.lod == lod)
            .map(|entry| {
                entry.used = self.clock;
                entry.baseline.clone()
            });
        // Make room before optional worker captures. Otherwise a full cache of
        // static chunks could permanently deny proofs to frequently changing ones.
        if LEASED_BYTES.load(Ordering::Relaxed) > BYTE_LIMIT - PROOF_LIMIT {
            for _ in 0..4 {
                let oldest = self
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        baseline
                            .as_ref()
                            .is_none_or(|b| !Arc::ptr_eq(&b.0, &e.baseline.0))
                    })
                    .min_by_key(|(_, e)| e.used)
                    .map(|(i, _)| i);
                let Some(i) = oldest else {
                    break;
                };
                self.entries.swap_remove(i);
            }
        }
        baseline
    }

    /// Called ONLY at validation-complete installation, not upload submission.
    /// World/LOD/reset paths using the ordinary install wrapper pass no proof.
    pub(super) fn install(
        &mut self,
        coord: IVec3,
        lod: u8,
        same_lod: bool,
        prepared: Prepared,
    ) -> bool {
        let previous = self
            .entries
            .iter()
            .position(|e| e.baseline.0.coord == coord);
        let unchanged = same_lod
            && previous.is_some_and(|i| {
                prepared.equal_to.as_ref().is_some_and(|expected| {
                    Arc::ptr_eq(&self.entries[i].baseline.0, &expected.0)
                        && expected.0.coord == coord
                        && expected.0.lod == lod
                })
            });
        if let Some(i) = previous {
            self.entries.swap_remove(i);
        }
        if let Some(proof) = prepared.proof {
            self.remember(coord, lod, proof);
        }
        unchanged
    }

    fn remember(&mut self, coord: IVec3, lod: u8, proof: Arc<Proof>) {
        if self.lookup_lease.is_none() {
            let Some(mut lease) = Lease::reserve(CACHE_ENTRIES * size_of::<Entry>()) else {
                return;
            };
            let mut entries = Vec::new();
            if entries.try_reserve_exact(CACHE_ENTRIES).is_err() {
                return;
            }
            if lease
                .grow(entries.capacity() * size_of::<Entry>())
                .is_none()
            {
                return;
            }
            self.entries = entries;
            self.lookup_lease = Some(lease);
        }
        if self.entries.len() == CACHE_ENTRIES {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.used)
                .map(|(i, _)| i)
                .unwrap();
            self.entries.swap_remove(oldest);
        }
        let Some(lease) = Lease::reserve(size_of::<Installed>() + 2 * size_of::<usize>()) else {
            return;
        };
        self.clock = self.clock.wrapping_add(1);
        self.entries.push(Entry {
            baseline: Baseline(Arc::new(Installed {
                coord,
                lod,
                proof,
                _lease: lease,
            })),
            used: self.clock,
        });
    }
}
