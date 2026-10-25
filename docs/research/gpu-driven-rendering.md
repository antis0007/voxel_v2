# GPU-driven submission

Reviewed 2026-10-24; R01/R02/C18. Haar/Aaltonen's [SIGGRAPH 2015 slides](https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf) timed out; exact pipeline and device timings remain unverified. The [binary mesher's own renderer](https://github.com/cgerikj/binary-greedy-meshing) documents indirect submission; its API route does not establish our backend support.

## What we adopt

S3 compares bounded brick/cluster culling and indirect submission while authoritative integer state stays separate from presentation. Keep cached CPU draw visibility as a proposed capability fallback.

## What we reject

Do not assume compute, multi-draw, Hi-Z, visibility buffers and every backend are available together. Do not declare a visibility buffer mandatory before measuring its material/transparent workload costs.

## What we improve

R02 specifies each capability branch, allocation and upload budget, including GL/no-compute. Publish frame-time distributions and edit latency on defined classes before selecting the renderer.
