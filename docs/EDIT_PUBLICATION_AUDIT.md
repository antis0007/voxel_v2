# Edit publication audit

Status: design only. No staged-edit Rust implementation or runtime acceptance.
The current ordinary tool loop changes authoritative blocks/collision before its
queued chunk mesh reaches the GPU. Existing cover remains visible during that gap.

## Existing path

- `main.rs` consumes `CreativeState::next_edit` before place/harvest side effects.
- `ChunkStreamer::rebuild_dirty` patches/queues source, not synchronous geometry.
- Existing building blocks are `generate_padded` plus canonical snapshot overlays,
  `MeshPlan::for_chunk/build_lod`, `gpu_upload::start/check`, and
  `ChunkCache::install_mesh`. Queue writes need normal draw ordering, no GPU idle.
- `MeshBudget::begin_plan/acquire` can block on condition variables; they belong
  on workers and cannot be reused as blocking event-thread edit admission.

## Smallest proposed ordinary-command lane

Hold a peeked command with a monotonic queue token. One bounded background job
uses immutable source and a predicted mutation bundle, builds all affected
chunks and corner halos through the existing mesher, and returns bounded CPU
results. Main-thread upload retains separate prepared GPU resources, charged
against explicit pending capacity, while old world/collision/cover stay active.

After all gameplay, fluid and sandbox geometry producers, validate the current
command, world epoch/configuration, source/neighbor/water dependencies, support,
player AABB and inventory/eligibility. Only a complete validated GPU bundle may
consume the command, run actual gameplay side effects, and install all meshes
before the next draw. Failed gameplay mutation must not install a candidate.
Refresh the streamer's snapshot after commit. Cancel obsolete jobs and discard
all affected cached LODs; dirty previous meshes must not be remembered by
`install_mesh` and later resurrected. Prepared source can be evicted after commit.

## Unresolved limits: not a complete implementation plan

- One cell per full-chunk transaction would make ordinary multi-cell brushes
  potentially unusably slow. A practical solution needs bounded mutation batches,
  not merely a serial single-cell preview/commit loop.
- Ordinary harvest can remove attached torches. Log harvest can fell an entire
  trunk/canopy and create a rigid assembly. Factory placement/dismantling and
  explosive/spell operations have their own multi-cell and inventory effects.
  A single-cell candidate does not cover these operations. Their complete
  mutation/side-effect plans must be represented before calling the gap closed.
- Continuous fluid changes within a mesh dependency region can invalidate every
  asynchronous candidate. Pausing the world simulation is not an acceptable fix;
  bounded work and useful publication under churn remain unresolved.
- Pending CPU/GPU bytes, complete-bundle admission and failure handling need
  explicit reservations without evicting the last valid cover.
- Candidate overlays must preserve canonical roof and water semantics. The
  historical direct block copier bypassed `GeneratedChunk::set_world` roof
  updates; reuse the corrected source path rather than inventing another one.
- World switches, command cancellation, changed inventory/AABB, failed harvest,
  old worker results and asynchronous GPU allocation/validation errors must
  all reject publication without consuming or granting an uncommitted edit.

No edit-publication gap is claimed closed by this audit. The existing immediate
collision versus delayed GPU state remains a known acceptance failure.
