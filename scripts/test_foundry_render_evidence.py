"""No GPU, game launch or world writes required."""
import csv
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import foundry_perf_metrics as metrics

RECEIPT = "[shader-validation] world_fnv1a64=0123456789abcdef build_match=true hardware_weather_filtering=false"

class RenderEvidenceTests(unittest.TestCase):
    def test_exact_shader_receipt(self):
        r = metrics.auxiliary("startup\n" + RECEIPT, "")["shader_validation"]
        self.assertEqual(r["status"], "reported")
        self.assertEqual(r["records"][0]["line"], 2)
        self.assertEqual(r["records"][0]["fnv1a64"], "0123456789abcdef")
        self.assertFalse(r["records"][0]["hardware_weather_filtering"])
        self.assertIn("not all shaders", r["scope"])

    def test_missing_and_malformed_not_success(self):
        r = metrics.shader_validation("", "")
        self.assertEqual(r["status"], "unavailable")
        self.assertEqual(r["records"], [])
        r = metrics.shader_validation(RECEIPT.replace("0123456789abcdef", "invalid"), "")
        self.assertEqual(r["status"], "unavailable")
        self.assertEqual(r["rejected_records"], 1)

    def test_mismatch_and_multiple_variants_preserved(self):
        r = metrics.shader_validation(RECEIPT, RECEIPT.replace("build_match=true", "build_match=false").replace("filtering=false", "filtering=true"))
        self.assertEqual(len(r["records"]), 2)
        self.assertFalse(r["records"][1]["build_match"])
        self.assertTrue(r["records"][1]["hardware_weather_filtering"])
        self.assertEqual(r["records"][1]["stream"], "stderr")

    def test_receipts_bounded(self):
        r = metrics.shader_validation("\n".join([RECEIPT] * 20), "")
        self.assertEqual(len(r["records"]), 16)
        self.assertEqual(r["omitted_records"], 4)

    def test_optional_probe_column_does_not_rewrite_legacy_scope(self):
        modern = metrics.timing_scope(["gpu_sky_probe_ms"])
        self.assertNotIn("sky probe", modern["known_unmeasured_work"])
        self.assertEqual(modern["legacy_subtotal_excludes"], ["gpu_sky_probe_ms"])
        self.assertIn("sky probe", metrics.timing_scope([])["known_unmeasured_work"])
        self.assertFalse(modern["complete_frame_cost"])

    def test_sample_coverage_never_implies_full_workload(self):
        with tempfile.TemporaryDirectory() as folder:
            p = Path(folder) / "frames.csv"
            row = dict(total_ms=10, gpu_supported="true", gpu_sample_count=1,
                       gpu_frame_id=1, render_frame_id=2, gpu_config_id=4,
                       render_config_id=4, gpu_pass_ms=3, gpu_sky_probe_ms=0.25)
            with p.open("w", newline="") as f:
                writer = csv.DictWriter(f, fieldnames=row.keys())
                writer.writeheader(); writer.writerow(row); writer.writerow(row)
            r = metrics.read_frames(p, 16)
        self.assertEqual(r["gpu_coverage"]["unique_samples"], 1)
        self.assertEqual(r["gpu_coverage"]["excluded_rows"]["duplicate_sample"], 1)
        self.assertEqual(r["gpu_ms"]["gpu_pass_ms"]["p50"], 3)
        self.assertEqual(r["gpu_ms"]["gpu_sky_probe_ms"]["p50"], 0.25)
        scope = r["gpu_coverage"]["timing_scope"]
        self.assertFalse(scope["complete_frame_cost"])
        self.assertFalse(scope["inventory_verified_against_capture"])
        self.assertNotIn("sky probe", scope["known_unmeasured_work"])

if __name__ == "__main__": unittest.main()
