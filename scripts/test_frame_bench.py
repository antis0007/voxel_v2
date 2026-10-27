#!/usr/bin/env python3
"""Tests for scripts/frame_bench.py: report parsing and the gate rule.

    python -I scripts/test_frame_bench.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import frame_bench  # noqa: E402

REPORT = """\
[benchmark] starting world=Hills voxel_scale=OneMeter seed=74 weather=Some(Clear) motion=static warmup_s=6.0 duration_s=15.0
[benchmark] warmup complete
[benchmark] adapter="NVIDIA GeForce RTX 4090 (Vulkan)" res=1600x900 view_chunks=28 lod_start=8 lod_levels=4 motion=static samples=1200
[benchmark] avg_fps=80.10 avg_frame_ms=12.484 p50_frame_ms=12.100 p95_frame_ms=14.900 p99_frame_ms=19.250 max_frame_ms=31.000 fps_1pct=51.95
[benchmark] cpu_avg_ms ui=0.210 gameplay=0.400 snapshot=0.010 stream=1.300 cull=0.200 torch=0.000 weather=0.050 uniforms=0.020 render_call=2.100 horizon=0.300
[benchmark] gpu_avg_ms passes=9.800 shadow=2.100 terrain=3.300 indirect=0.150 raymarch=3.900 temporal=0.100 sky=0.080 composite=0.120 outline=0.010 ui=0.030
[benchmark] latest_scene visible_chunks=900 gpu_meshes=880 draw_chunks=700 terrain_tris=4900000 torches=0 shadow_draws=40 terrain_draws=1020 weather_rects=0 weather_bytes=0
noise line without the tag p95_frame_ms=999
"""


def sample(p95, p99=20.0, draws=1000, adapter="GPU A"):
    return {"adapter": adapter, "p95_frame_ms": p95, "p99_frame_ms": p99, "draws": draws}


class ParseTest(unittest.TestCase):
    def test_parse_reads_tagged_and_untagged_lines(self):
        m = frame_bench.parse_report(REPORT)
        self.assertEqual(m["adapter"], "NVIDIA GeForce RTX 4090 (Vulkan)")
        self.assertEqual(m["res"], "1600x900")
        self.assertAlmostEqual(m["p95_frame_ms"], 14.9)
        self.assertAlmostEqual(m["cpu_avg_ms.stream"], 1.3)
        self.assertAlmostEqual(m["gpu_avg_ms.raymarch"], 3.9)
        self.assertEqual(m["latest_scene.terrain_draws"], 1020)
        self.assertEqual(m["draws"], 1060)

    def test_report_without_timing_is_rejected(self):
        with self.assertRaises(frame_bench.BenchError):
            frame_bench.parse_report("[benchmark] warmup complete\n[device-lost] driver\n")


class GateTest(unittest.TestCase):
    def test_best_pass_decides_and_contention_is_forgiven(self):
        runs = [sample(30.0), sample(15.4), sample(22.0)]
        verdict = frame_bench.compare(runs, sample(15.0))
        self.assertEqual(verdict.failures, [])
        self.assertAlmostEqual(verdict.current["p95_frame_ms"], 15.4)

    def test_frame_time_over_five_percent_fails(self):
        verdict = frame_bench.compare([sample(15.9), sample(16.2)], sample(15.0))
        self.assertTrue(any("p95_frame_ms" in f for f in verdict.failures))

    def test_slow_frames_from_contended_passes_are_unverified_not_failed(self):
        runs = [dict(sample(20.0), contended=True), dict(sample(21.0), contended=True)]
        verdict = frame_bench.compare(runs, sample(15.0))
        self.assertEqual(verdict.failures, [])
        self.assertTrue(verdict.unverified)
        mixed = runs + [dict(sample(19.0), contended=False)]
        self.assertTrue(frame_bench.compare(mixed, sample(15.0)).failures)

    def test_draw_calls_over_ten_percent_fail_even_when_contended(self):
        runs = [dict(sample(15.0, draws=1200), contended=True)]
        self.assertTrue(any("draws" in f for f in frame_bench.compare(runs, sample(15.0, draws=1000)).failures))

    def test_draw_calls_over_ten_percent_fail_on_the_median(self):
        runs = [sample(15.0, draws=900), sample(15.0, draws=1200), sample(15.0, draws=1150)]
        verdict = frame_bench.compare(runs, sample(15.0, draws=1000))
        self.assertTrue(any("draws" in f for f in verdict.failures))

    def test_summary_takes_every_timing_from_one_pass(self):
        a = dict(sample(30.0, p99=40.0), p50_frame_ms=20.0)
        b = dict(sample(15.0, p99=18.0), p50_frame_ms=12.0)
        record = frame_bench.summarize([a, b], {"rev": "x"})
        self.assertEqual((record["p50_frame_ms"], record["p95_frame_ms"], record["p99_frame_ms"]), (12.0, 15.0, 18.0))

    def test_frame_medians_ignore_cascade_spikes(self):
        row = lambda shadow: {"terrain_draws": "1000", "shadow_draws": shadow, "stream_ms": "1",
                              "render_cpu_ms": "2", "gpu_pass_ms": "9", "gpu_shadow_ms": "3"}
        m = frame_bench.frame_medians([row("0"), row("0"), row("5000")])
        self.assertEqual(m["draws"], 1000)
        self.assertEqual(frame_bench.frame_medians([]), {})

    def test_other_adapter_is_not_comparable(self):
        with self.assertRaises(frame_bench.BenchError):
            frame_bench.compare([sample(15.0, adapter="GPU B")], sample(15.0))

    def test_scene_arguments_pin_seed_generator_and_size(self):
        for name, args in frame_bench.scene_args().items():
            joined = " ".join(args)
            for flag in ("--benchmark-seed=", "--benchmark-generator-version=", "--benchmark-width=",
                         "--benchmark-duration=", "--benchmark-weather="):
                self.assertIn(flag, joined, name)


if __name__ == "__main__":
    unittest.main(verbosity=1)
