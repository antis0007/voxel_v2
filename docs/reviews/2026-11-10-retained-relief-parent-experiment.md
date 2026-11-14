# Retained relief parent experiment: rejected for default integration

## Decision
Do not integrate this candidate into V13. It removes repeated work when the relief disk cache is disabled, but gives no meaningful measured improvement on the current default cold or warm cache paths. The additional retained memory and lifetime/concurrency machinery are not justified by these default-path measurements.

## Scope and implementation
Private source: parent-reuse-source. Production proposal SHA256: 68f0e7cc083eaaa17ed0ea7be013883b1ccdfaed41aaac9ac8987725743bc3e9.
A transient raw 8 m Stage is retained after detail-first initialization and consumed by fine initialization. Full query configuration, tile key and a publication-only World lifetime token guard reuse; the immutable coarse Stage belongs to the same Levels. Fine closes the slot before computation so late concurrent detail completion cannot retain memory. Mutable direct builds and older generators do not retain. No lock is added to already-built sampling paths. No Stage equations are changed.
Maximum extra array payload is 92,928 bytes per pending tile; 192 local map entries represent about 17.02 MiB. Existing HOT/live Arcs can exceed that map count, so this is not a strict global memory cap. Memory is released on fine initialization or slot eviction/drop.

## Freshness and measurements
Fresh archived candidate probe SHA256: 70e5ce169036e2b69799bdb897d70d163ebf961a69f17c315f74daed080c46af.
Archived V13 probe SHA256: dc440990150bd05eb667c2e24ffa87e47ea0c5ae206f22a5766dbb65bb0f5244.
Compiler artifacts verified fresh=false. Parent-reuse-binary holds build receipts and hashes. Source was changed afterward only by the additive test overlay; that overlay was not compiled into this benchmark binary.

Fresh processes, seed 74, generator 287, Planet, one-metre scale, no custom planet recipe, six sequential padded production chunks, warm geology cache. Reported times cover chunk generation, not geology setup, native time-to-play, FPS or concurrent worker contention.

| Relief policy | V13 median | Candidate median | Change | Repeats each |
|---|---:|---:|---:|---:|
| Disabled | 25.457139 s | 23.279083 s | -8.56% | 3 |
| Initially empty directory | 27.455268 s | 27.321771 s | -0.49% | 2 |
| Warm directory | 1.219468 s | 1.214207 s | -0.43% | 2 |

All runs preserved checksum 686900d727e4a31d over all 235,824 output blocks, solid count 149,328. Stage requests fell from 489 to 439. Crucially, V13's initially empty disk-cache run already serves those 50 duplicate calls as same-session hits. Both variants store exactly 439 unique Stages, 34,091,239 payload bytes. Therefore the cache-disabled win does not establish a meaningful default first-load win. The sub-0.5% default differences are smaller than run variation and are not accepted as speedups.

Raw logs, per-run identities, samples and evaluation: parent-reuse-screen/. No run was silently discarded; the initial candidate prime is explicitly excluded from warmed-geology comparisons.

## Verification limits
Production candidate compiled. Every measured chunk output matched the baseline. Independent static review covered lifetime/config matching, parent lineage, short mutex scope and closed-slot race behavior.
Additive frozen-Levels, mixed-config, complete-array and real-thread-race tests were prepared and applied privately, but NOT compiled or executed after the default-path screen failed the value gate. No full correctness acceptance, legacy validation, save validation, quarter-scale validation or native build is claimed for this rejected candidate.

## Delivery safety and follow-up
No shared engine source, V13 executable, launcher or user save was changed by this experiment. No visible native app was opened. All benchmark processes have ended.
Keep the proposal as a conditional option if measurements later show disk-disabled operation or contested/skipped cache writes dominate. Before revival, run the prepared exactness/concurrency tests and measure the real contention workload. For default first-load work, prioritize the unique Stage input/solver costs rather than eliminating disk hits already measured at low cost.