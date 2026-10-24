# GPU-driven rendering (Haar and Aaltonen; visibility buffer)
Source: Ulrich Haar and Sebastian Aaltonen, "GPU-Driven Rendering Pipelines", SIGGRAPH 2015 Advances in Real-Time Rendering (https://advances.realtimerendering.com/s2015/aaltonenhaar_siggraph2015_combined_final_footer_220dpi.pdf); slide text read via a mirror (pdf4pro), so exact claims are unverified against the PDF itself.
Adopt: GPU culling of fixed-size clusters writing indirect/multi-draw arguments with no CPU readback; cluster-level frustum and occlusion culling; a visibility buffer (triangle and instance ids per pixel) as a cache-friendly alternative to a fat G-buffer.
Reject: Ubisoft's 300-occluder depth pre-pass and per-cluster backface baking as first steps; CPU quad-tree culling; the reported 0.7 ms PS4 budget is not our target.
Why: Indirect draws for 8^3 bricks cut CPU work for factory and magic scenes, but integer voxel state must stay out of the culling path and the iGPU budget is tight.
