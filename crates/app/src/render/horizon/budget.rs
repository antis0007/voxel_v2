//! One publication allowance per request, selected from one observed snapshot.
//! The procedural representation keeps its192MiB cap. Headroom is NOT reserved.
const MIB: usize = 1024 * 1024;
pub(super) const BASE_LIMIT: usize = 192*MIB;
pub(super) const MAX_PUBLICATION_LIMIT: usize = 512*MIB;
const TIERS: [usize;6] = [BASE_LIMIT,256*MIB,320*MIB,384*MIB,448*MIB,MAX_PUBLICATION_LIMIT];
const EDIT_SCRATCH: usize = 64*MIB;
const MAX_SAMPLE_AGE_MS: u64 = 5_000;

#[derive(Clone,Copy)]
struct Headroom {
    gpu_live: bool, gpu_age_ms: Option<u64>, gpu_budget: usize,
    gpu_usage: usize, gpu_policy: usize, cpu_available: Option<usize>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum Denied { GpuUnknown,GpuStale,GpuPolicy,GpuPressure,CpuUnknown,CpuPressure }
impl Denied {
    pub fn label(self) -> &'static str { match self {
        Self::GpuUnknown=>"gpu_headroom_unknown",Self::GpuStale=>"gpu_sample_stale",
        Self::GpuPolicy=>"gpu_policy_ceiling",Self::GpuPressure=>"gpu_memory_pressure",
        Self::CpuUnknown=>"cpu_headroom_unknown",Self::CpuPressure=>"cpu_memory_pressure",
    } }
}
#[derive(Clone,Copy,Debug)]
pub(super) struct Decision { pub limit: usize, pub denial: Option<Denied> }
#[derive(Clone,Copy)]
enum Phase { Build { retained_base: usize }, Upload { unallocated_gpu: usize } }
fn cpu_build_peak(limit: usize, retained_base: usize) -> usize {
    // 2xbase capacity +2L source +2L destination +L realloc overlap +2L
    // weld/remap workspace +2xowner scratch +L margin. Credit only actual
    // retained capacity of the exact reusable base, once; Arcs are not summed.
    (2*BASE_LIMIT).saturating_add(limit.saturating_mul(8))
        .saturating_add(2*EDIT_SCRATCH).saturating_sub(retained_base.min(2*BASE_LIMIT))
}
fn admission_at(h: Headroom, phase: Phase, limit: usize) -> Result<(),Denied> {
    debug_assert!((BASE_LIMIT..=MAX_PUBLICATION_LIMIT).contains(&limit));
    if !h.gpu_live || h.gpu_age_ms.is_none() { return Err(Denied::GpuUnknown); }
    if h.gpu_age_ms.unwrap()>MAX_SAMPLE_AGE_MS { return Err(Denied::GpuStale); }
    if h.gpu_policy<limit { return Err(Denied::GpuPolicy); }
    let reserve=(h.gpu_budget/5).max(512*MIB);
    let free=h.gpu_budget.saturating_sub(h.gpu_usage).saturating_sub(reserve);
    let gpu_remaining=match phase {
        Phase::Build {..}=>3*limit,
        Phase::Upload {unallocated_gpu}=>unallocated_gpu.saturating_add(limit+16*MIB),
    };
    if free<gpu_remaining { return Err(Denied::GpuPressure); }
    let available=h.cpu_available.ok_or(Denied::CpuUnknown)?;
    let cpu_remaining=match phase {
        Phase::Build {retained_base}=>cpu_build_peak(limit,retained_base),
        Phase::Upload {..}=>2*limit,
    };
    if available<cpu_remaining { return Err(Denied::CpuPressure); }
    Ok(())
}
fn select(h: Headroom, retained_base: usize) -> Decision {
    // Every tier uses the SAME snapshot. No allocations are made by selection.
    let phase=Phase::Build {retained_base};
    for limit in TIERS.into_iter().rev().filter(|&limit|limit>BASE_LIMIT) {
        if admission_at(h,phase,limit).is_ok() { return Decision {limit,denial:None}; }
    }
    Decision {limit:BASE_LIMIT,denial:admission_at(h,phase,TIERS[1]).err()}
}
fn observe() -> Headroom {
    let gpu=crate::render::gpu_residency_status();
    let bytes=|mib:usize|mib.saturating_mul(MIB);
    Headroom {
        gpu_live:gpu.detected&&gpu.live,gpu_age_ms:gpu.sample_age_ms,
        gpu_budget:bytes(if gpu.dedicated_mib>0 {gpu.os_budget_mib.min(gpu.dedicated_mib)} else {gpu.os_budget_mib}),
        gpu_usage:bytes(gpu.process_usage_mib),gpu_policy:bytes(gpu.limit_mib.min(gpu.ceiling_mib)),
        cpu_available:cpu_available(),
    }
}
pub(super) fn choose(retained_base: usize) -> Decision { select(observe(),retained_base) }
pub(super) fn build_admission(limit:usize,retained_base:usize)->Result<(),Denied> {
    admission_at(observe(),Phase::Build {retained_base},limit)
}
pub(super) fn upload_admission(limit:usize,unallocated_gpu:usize)->Result<(),Denied> {
    admission_at(observe(),Phase::Upload {unallocated_gpu},limit)
}
pub(super) fn telemetry_grace(denial:Option<Denied>,elapsed:std::time::Duration)->bool {
    matches!(denial,Some(Denied::GpuUnknown|Denied::GpuStale))&&elapsed<std::time::Duration::from_secs(6)
}
pub(super) fn admits(current:usize,additional:usize,limit:usize)->bool {
    current.checked_add(additional).is_some_and(|total|total<=limit)
}
// Windows follows the repository's existing read-only kernel32 FFI style.
// On other platforms we currently have no verified RAM/commit measurement, so
// expansion is denied rather than inventing capacity from a configured ceiling.
fn usable_cpu_bytes(physical: u64, commit: u64, address_space: u64) -> usize {
    usize::try_from(physical.min(commit).min(address_space)).unwrap_or(usize::MAX)
}
#[cfg(windows)]
fn cpu_available() -> Option<usize> {
    #[repr(C)]
    #[derive(Default)]
    struct MemoryStatus {
        length: u32, load: u32, total_phys: u64, avail_phys: u64,
        total_page: u64, avail_page: u64, total_virtual: u64, avail_virtual: u64,
        avail_extended: u64,
    }
    #[link(name="kernel32")]
    unsafe extern "system" { fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32; }
    let mut status = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, ..Default::default() };
    // Correct MEMORYSTATUSEX layout; initialized writable storage lives through
    // this synchronous read-only OS query. No handles are acquired.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then(|| usable_cpu_bytes(status.avail_phys, status.avail_page, status.avail_virtual))
}
#[cfg(not(windows))]
fn cpu_available() -> Option<usize> { None }


#[cfg(test)]
mod tests {
    use super::*;
    fn ample()->Headroom { Headroom {gpu_live:true,gpu_age_ms:Some(0),gpu_budget:24*1024*MIB,
        gpu_usage:3*1024*MIB,gpu_policy:5*1024*MIB,cpu_available:Some(16*1024*MIB)} }
    #[test]
    fn every_cpu_and_gpu_tier_boundary_is_monotone_and_bounded() {
        for (index,&limit) in TIERS.iter().enumerate().skip(1) {
            let prior=TIERS[index-1];
            let peak=cpu_build_peak(limit,0);
            assert_eq!(select(Headroom {cpu_available:Some(peak),..ample()},0).limit,limit);
            assert_eq!(select(Headroom {cpu_available:Some(peak-1),..ample()},0).limit,prior);
            assert_eq!(select(Headroom {gpu_policy:limit,..ample()},0).limit,limit);
            assert_eq!(select(Headroom {gpu_policy:limit-1,..ample()},0).limit,prior);
            let h=ample();let reserve=(h.gpu_budget/5).max(512*MIB);
            assert_eq!(select(Headroom {gpu_usage:h.gpu_budget-reserve-3*limit,..h},0).limit,limit);
            assert_eq!(select(Headroom {gpu_usage:h.gpu_budget-reserve-3*limit+1,..h},0).limit,prior);
        }
        assert_eq!(select(ample(),0).limit,MAX_PUBLICATION_LIMIT);
        assert_eq!(BASE_LIMIT,192*MIB);
        assert_eq!(cpu_build_peak(256*MIB,0),2560*MIB);
        assert_eq!(cpu_build_peak(MAX_PUBLICATION_LIMIT,0),4608*MIB);
    }
    #[test]
    fn unknown_stale_and_low_telemetry_keep_legacy_limit() {
        for (h,reason) in [
            (Headroom {gpu_live:false,..ample()},Denied::GpuUnknown),
            (Headroom {gpu_age_ms:None,..ample()},Denied::GpuUnknown),
            (Headroom {gpu_age_ms:Some(5_001),..ample()},Denied::GpuStale),
            (Headroom {gpu_policy:BASE_LIMIT,..ample()},Denied::GpuPolicy),
            (Headroom {gpu_usage:24*1024*MIB,..ample()},Denied::GpuPressure),
            (Headroom {cpu_available:None,..ample()},Denied::CpuUnknown),
            (Headroom {cpu_available:Some(1100*MIB),..ample()},Denied::CpuPressure),
        ] {let decision=select(h,0);assert_eq!(decision.limit,BASE_LIMIT);assert_eq!(decision.denial,Some(reason));}
        assert_eq!(select(ample(),0).limit,MAX_PUBLICATION_LIMIT);
    }
    #[test]
    fn retained_base_is_credited_once_and_never_fictitiously() {
        let limit=MAX_PUBLICATION_LIMIT;let capacity=184*MIB;
        let before=Headroom {cpu_available:Some(cpu_build_peak(limit,0)),..ample()};
        assert_eq!(admission_at(before,Phase::Build {retained_base:0},limit),Ok(()));
        let after=Headroom {cpu_available:Some(before.cpu_available.unwrap()-capacity),..before};
        assert_eq!(admission_at(after,Phase::Build {retained_base:capacity},limit),Ok(()));
        assert_eq!(admission_at(after,Phase::Build {retained_base:0},limit),Err(Denied::CpuPressure));
        assert_eq!(cpu_build_peak(limit,usize::MAX),cpu_build_peak(limit,2*BASE_LIMIT));
    }
    #[test]
    fn owned_pages_and_partial_buffers_do_not_revoke_admission() {
        for &limit in TIERS.iter().skip(1) {
            let h=ample();let reserve=(h.gpu_budget/5).max(512*MIB);
            let initial=Headroom {cpu_available:Some(cpu_build_peak(limit,0)+32*MIB),
                gpu_usage:h.gpu_budget-reserve-3*limit-8*MIB,..h};
            assert_eq!(admission_at(initial,Phase::Build {retained_base:0},limit),Ok(()));
            for allocated in [0,limit/2,limit] {
                let live=Headroom {cpu_available:Some(initial.cpu_available.unwrap()-limit),gpu_usage:initial.gpu_usage+allocated,..initial};
                assert_eq!(admission_at(live,Phase::Upload {unallocated_gpu:limit-allocated},limit),Ok(()));
            }
            assert_eq!(admission_at(Headroom {cpu_available:Some(2*limit-1),..initial},Phase::Upload {unallocated_gpu:0},limit),Err(Denied::CpuPressure));
        }
    }
    #[test]
    fn physical_commit_address_space_and_total_admission_are_checked() {
        let plenty=(16*1024*MIB) as u64;let low=(512*MIB) as u64;
        for (physical,commit,address) in [(low,plenty,plenty),(plenty,low,plenty),(plenty,plenty,low)] {
            assert_eq!(select(Headroom {cpu_available:Some(usable_cpu_bytes(physical,commit,address)),..ample()},0).limit,BASE_LIMIT);
        }
        assert!(admits(MAX_PUBLICATION_LIMIT-8,8,MAX_PUBLICATION_LIMIT));
        assert!(!admits(MAX_PUBLICATION_LIMIT-8,9,MAX_PUBLICATION_LIMIT));
        assert!(!admits(usize::MAX,1,usize::MAX));
    }
    #[test]
    fn unknown_to_fresh_can_change_allowance_but_grace_is_bounded() {
        use std::time::Duration;
        assert!(telemetry_grace(Some(Denied::GpuUnknown),Duration::ZERO));
        assert!(!telemetry_grace(Some(Denied::GpuUnknown),Duration::from_secs(6)));
        assert!(!telemetry_grace(Some(Denied::CpuPressure),Duration::ZERO));
        assert_eq!(select(Headroom {gpu_live:false,..ample()},0).limit,BASE_LIMIT);
        assert_eq!(select(ample(),0).limit,MAX_PUBLICATION_LIMIT);
    }
}
