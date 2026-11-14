"""Diagnostic parsing only; no world generation is executed."""
import copy
import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import foundry_perf_headless as h
import foundry_perf_compare as comparison
from test_foundry_perf_headless import HeadlessTests

RIVER = '[river-attempt] outcome=accepted origin=[1, 2, 3] init_ms=12.3 hydro_ms=400.1 budget_ok=true bank_ok=true inlets=3 outlet_depth=0.8 terrain_calls=5565 rock_calls=16695 cell_steps=27379800'
REJECT = '[river-attempt] outcome=initial-reject origin=[1, 2, 3] init_ms=10 inlets=0 outlet_depth=0 terrain_calls=5565 rock_calls=16695'
SUMMARY = '[river-attempt-summary] ranked=20 considered_max=24 attempted=7 overlap_skipped=3 accepted=6 elapsed_ms=800.0'
MEANDER = '[meander-attempt] rank=0 accepted=false elapsed_ms=25.5'
SLOPE = '[slope-build] nodes=484 total_ms=8 inputs_ms=5 input_total_nodes=484 fixed_nodes=20 sample_stride=64 sample_phase=2 sampled_nodes=8 sampled_fixed=1 input_node_loop_ms=4 input_post_ms=1 sampled_projection_volcano_ms=0.002 sampled_ground_ms=0.03 sampled_exposure_noise_collision_ms=0.004 sampled_columns_ms=0.04 sampled_remaining_ms=0.001'


class DiagnosticTests(unittest.TestCase):
    def test_river_semantics(self):
        rows=h.diagnostics('\n'.join([RIVER,REJECT,SUMMARY,MEANDER]))
        self.assertEqual([r['kind'] for r in rows],['river-attempt','river-attempt','river-attempt-summary','meander-attempt'])
        self.assertEqual(rows[0]['derived_counts']['cell_steps'],27379800)
        self.assertNotIn('cell_steps',rows[0]['counts'])
        self.assertEqual(rows[2]['durations_ms'],{'elapsed_ms':800.0})
        self.assertIn('never sum',rows[0]['scope'])
        self.assertIn('excludes final tile packing',rows[0]['scope'])
        self.assertNotIn('total', rows[2])

    def test_sampled_time_is_separate(self):
        r=h.diagnostics(SLOPE)[0]
        self.assertEqual(r['counts']['sampled_nodes'],8)
        self.assertEqual(r['sampled_durations_ms']['sampled_columns_ms'],0.04)
        self.assertNotIn('sampled_columns_ms',r['durations_ms'])
        self.assertEqual(r['durations_ms']['input_node_loop_ms'],4)

    def test_invalid_records(self):
        for line in (RIVER.replace('hydro_ms=400.1','hydro_ms=NaN'),
                     RIVER.replace('init_ms=12.3',''),RIVER.replace('outcome=accepted','outcome=unknown'),
                     SUMMARY.replace('accepted=6','accepted=8'),SUMMARY.replace('accepted=6',''),
                     MEANDER.replace('accepted=false','accepted=maybe'),
                     SLOPE.replace('sample_stride=64','sample_stride=0'),
                     SLOPE.replace('sample_phase=2','sample_phase=64'),
                     SLOPE.replace('sampled_nodes=8','sampled_nodes=999'),
                     SLOPE.replace('fixed_nodes=20','fixed_nodes=0'),
                     SLOPE.replace('sampled_columns_ms=0.04','sampled_columns_ms=-1')):
            with self.subTest(line=line):
                with self.assertRaises(ValueError): h.diagnostics(line)

    def test_identity_integration(self):
        fixture=HeadlessTests();fixture.setUp()
        try:
            text=fixture.text+'\n'+'\n'.join([RIVER,SUMMARY,MEANDER,SLOPE])
            with self.assertRaises(ValueError):fixture.analyse(text)
            c=copy.deepcopy(fixture.c)
            c['instrumentation']={'profile':'river-and-slope-sampled','diagnostic_kinds':['river-attempt','river-attempt-summary','meander-attempt','slope-build']}
            r=fixture.analyse(text,c)
            self.assertEqual(r['workload']['instrumentation']['sample_strides'],[64])
            r.update(source_capture='a',raw_artifacts={'raw/stdout.log':'one'})
            other=fixture.analyse(text.replace('sample_stride=64','sample_stride=32'),c)
            other.update(source_capture='b',raw_artifacts={'raw/stdout.log':'two'})
            self.assertEqual(comparison.compare_reports([r],[other])['status'],'incomparable')
            other=fixture.analyse(text.replace('fixed_nodes=20','fixed_nodes=21'),c)
            self.assertEqual(r['workload'],other['workload'])
        finally:fixture.doCleanups()

if __name__=='__main__': unittest.main()
