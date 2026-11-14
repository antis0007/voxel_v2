"""Headless analyzer tests use retained real receipts, never execute worldgen."""
import copy
import importlib.util
import json
import re
import tempfile
import unittest
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import foundry_perf as perf
import foundry_perf_headless as headless
import foundry_perf_compare as compare

BASE = Path(__file__).parent / 'testdata/foundry_headless'
ORIGINAL = BASE / 'native_reference.py'


def context(text):
    r = headless.one(text, '[planet-chunk-result]')
    c = {'workload_kind': 'headless-chunks', 'workload_id': 'six-chunks',
         'hardware': {'test_fixture': True},
         'instrumentation': {'profile': 'probe-only', 'diagnostic_kinds': []},
         'build': {'configuration_hash': 'a'*64, 'source_snapshot': 'b'*64},
         'world_config': {'seed': int(r['seed']), 'generator': int(r['version']),
                          'voxel_scale': r['scale'], 'world_type': 'Planet', 'full_config_sha256': 'c'*64},
         'geology_cache_state': 'hit' if '[geology-cache] hit' in text else 'miss-or-disabled',
         'chunk_coordinates': [[int(x) for x in xyz] for xyz in re.findall(r'coord=IVec3\((-?\d+), (-?\d+), (-?\d+)\)', text)],
         'expected_output': {'world_checksum': headless.one(text, '[worldgen] spherical')['checksum'],
                             'checksum': r['checksum'], 'blocks': int(r['blocks']), 'solid_count': int(r['solid_count'])}}
    return c


class HeadlessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.raw = Path(self.tmp.name)/'input'
        self.raw.mkdir()
        self.text = (BASE/'fine-reuse-screen/B-1.log').read_text(encoding='utf-8-sig')
        self.c = context(self.text)

    def analyse(self, text=None, c=None):
        text = self.text if text is None else text
        c = self.c if c is None else c
        (self.raw/'stdout.log').write_text(text)
        (self.raw/'manifest.json').write_text(json.dumps({'exit_code': 0, 'sha256': 'd'*64, 'world_config': c['world_config']}))
        return perf.analyse(self.raw, c, 1000/60)

    def test_real_all_repetitions_scales_cold_and_warm(self):
        for folder in ('fine-reuse-screen', 'fine-reuse-quarter-screen'):
            for p in (BASE/folder).glob('*.log'):
                with self.subTest(path=p.name, folder=folder):
                    text = p.read_text(encoding='utf-8-sig')
                    r = self.analyse(text, context(text))
                    self.assertIsNone(r['telemetry'])
                    self.assertFalse(r['merge_authorized'])
                    self.assertEqual(len(r['workload']['chunk_coordinates']), 6)
                    self.assertNotIn('fps', r['headless']['metrics_ms'])
                    self.assertNotIn('declared_population', r['workload'])

    def test_invalid_receipts(self):
        cases = [('', self.c), (self.text.replace('test result: ok.', 'test result: FAILED.'), self.c),
                 (self.text+self.text, self.c), (self.text.replace('count=6', 'count=0'), self.c),
                 (self.text.replace('count=6', 'count=1025'), self.c),
                 (self.text.replace('index=1', 'index=0'), self.c),
                 (self.text.replace('[planet-chunk] index=1', '[missing] index=1'), self.c),
                 (self.text.replace('index=1', 'index=1 index=1'), self.c),
                 (self.text.replace('solve_ms=10004.4', 'solve_ms=0'), self.c),
                 (self.text.replace('solve_ms=10004.4', 'solve_ms=inf'), self.c),
                 (self.text.replace('generate_ms=1892.014', 'generate_ms=NaN'), self.c),
                 (self.text.replace('generate_ms=1892.014', 'generate_ms=0'), self.c),
                 (self.text.replace('generate_ms=1892.014', 'generate_ms=-1'), self.c),
                 (self.text.replace('coord=IVec3(166, 0, 34)', 'coord=IVec3(2147483648, 0, 34)'), self.c),
                 (self.text.replace('scale=1m count', 'scale=quarter count'), self.c),
                 (self.text.replace('checksum=686900d727e4a31d', 'checksum=bad'), self.c),
                 (self.text.replace('solid_count=149328', 'solid_count=999999'), self.c),
                 (self.text.replace('version=287', 'version=288'), self.c)]
        for text, c in cases:
            with self.subTest(text=text[-250:]):
                with self.assertRaises(ValueError): self.analyse(text, c)

    def test_context_and_manifest_gates(self):
        for field, value in [('chunk_coordinates', []), ('expected_output', {}), ('geology_cache_state', 'miss-or-disabled'),
                             ('workload_kind', 'unknown'), ('hardware', None)]:
            c=copy.deepcopy(self.c); c[field]=value
            with self.subTest(field=field):
                with self.assertRaises(ValueError): self.analyse(c=c)
        self.analyse()
        manifest=json.loads((self.raw/'manifest.json').read_text()); manifest['exit_code']=1
        (self.raw/'manifest.json').write_text(json.dumps(manifest))
        with self.assertRaises(ValueError): perf.analyse(self.raw, self.c, 16)

    def test_ingest_raw_recheck_and_tamper(self):
        self.analyse()
        output=Path(self.tmp.name)/'report'
        r=perf.ingest(self.raw, output, self.c)
        self.assertEqual(compare.checked_report(output/'performance.json')['headless'], r['headless'])
        self.assertIn('Nested spans', (output/'performance.md').read_text())
        edited=json.loads((output/'performance.json').read_text()); edited['headless']['metrics_ms']['solve_ms']=1
        (output/'performance.json').write_text(json.dumps(edited))
        with self.assertRaises(ValueError): compare.checked_report(output/'performance.json')
        (output/'performance.json').write_text(json.dumps(r))
        (output/'raw/stdout.log').write_text('tampered')
        with self.assertRaises(ValueError): compare.checked_report(output/'performance.json')

    def test_compare_duplicate_and_mismatch(self):
        r=self.analyse(); r.update(source_capture='a', raw_artifacts={'raw/stdout.log': 'one'})
        b=copy.deepcopy(r); b.update(source_capture='b', raw_artifacts={'raw/stdout.log': 'two'}, binary_sha256='e'*64)
        self.assertEqual(compare.compare_reports([r], [b])['status'], 'diagnostic-only')
        self.assertEqual(compare.compare_reports([r], [r])['status'], 'incomparable')
        for key in ('voxel_scale', 'generator'):
            altered=copy.deepcopy(b); altered['workload']['config'][key]='different'
            self.assertEqual(compare.compare_reports([r],[altered])['status'],'incomparable')
        duplicate=copy.deepcopy(b); duplicate['raw_artifacts']=r['raw_artifacts']
        self.assertEqual(compare.compare_reports([r],[duplicate])['status'],'incomparable')
        b['headless']['output']['checksum']='0'*16
        self.assertEqual(compare.compare_reports([r], [b])['status'], 'incomparable')

    def test_nested_diagnostics(self):
        text=self.text+'\n[slope-build] nodes=100 total_ms=20 inputs_ms=8 passes_ms=10\n[cold-builder] name=roads key=x inclusive_ms=30 unwinding=false\n'
        c=copy.deepcopy(self.c)
        c['instrumentation']={'profile':'builder-timers', 'diagnostic_kinds':['cold-builder','slope-build']}
        r=self.analyse(text,c)
        self.assertEqual(len(r['headless']['diagnostics']), 2)
        self.assertEqual(r['headless']['diagnostics'][0]['durations_ms']['total_ms'],20)
        self.assertNotIn('diagnostic_total',r['headless']['metrics_ms'])
        with self.assertRaises(ValueError): self.analyse(text.replace('total_ms=20','total_ms=NaN'),c)

    def test_instrumentation_identity(self):
        for value in (None, {}, {'profile':' ', 'diagnostic_kinds':[]},
                      {'profile':'p', 'diagnostic_kinds':['unknown']},
                      {'profile':'p', 'diagnostic_kinds':['slope-build','slope-build']}):
            c=copy.deepcopy(self.c); c['instrumentation']=value
            with self.subTest(value=value):
                with self.assertRaises(ValueError): self.analyse(c=c)
        text=self.text+'\n[slope-build] nodes=100 total_ms=20\n'
        with self.assertRaises(ValueError): self.analyse(text)
        c=copy.deepcopy(self.c); c['instrumentation']={'profile':'timers', 'diagnostic_kinds':['slope-build']}
        r=self.analyse(); r.update(source_capture='a',raw_artifacts={'raw/stdout.log':'one'})
        b=self.analyse(text,c); b.update(source_capture='b',raw_artifacts={'raw/stdout.log':'two'})
        self.assertEqual(compare.compare_reports([r],[b])['status'],'incomparable')
        # Same profile but a different observed kind set is also incomparable.
        r=self.analyse(c=c); r.update(source_capture='a',raw_artifacts={'raw/stdout.log':'one'})
        self.assertEqual(compare.compare_reports([r],[b])['status'],'incomparable')
        # Counts are not part of workload identity.
        d=self.analyse(text.replace('nodes=100','nodes=200'),c)
        self.assertEqual(b['workload'],d['workload'])

    def test_invalid_metric_sets_fail_closed(self):
        r=self.analyse(); r.update(source_capture='a',raw_artifacts={'raw/stdout.log':'one'})
        for metrics in (None, {}, {'solve_ms':1}, {'solve_ms':float('nan')}, {'solve_ms':0}, 'bad'):
            b=copy.deepcopy(r); b.update(source_capture='b',raw_artifacts={'raw/stdout.log':'two'})
            b['headless']['metrics_ms']=metrics
            with self.subTest(metrics=metrics):
                self.assertEqual(compare.compare_reports([r],[b])['status'],'incomparable')

    def test_worldgen_without_chunk_claims(self):
        text='\n'.join(l for l in self.text.splitlines() if '[planet-chunk' not in l)
        c=copy.deepcopy(self.c); c['workload_kind']='headless-worldgen'
        c['expected_output']={'world_checksum': c['expected_output']['world_checksum']}
        r=self.analyse(text,c)
        self.assertEqual(set(r['headless']['metrics_ms']), {'solve_ms'})
        self.assertIsNone(r['headless']['spans'][0]['parent'])

    def test_native_analyzer_unchanged(self):
        spec=importlib.util.spec_from_file_location('original_perf', ORIGINAL)
        old=importlib.util.module_from_spec(spec); spec.loader.exec_module(old)
        # Native missing/partial evidence remains native, not a successful headless run.
        from test_frame_bench import REPORT
        for text in ('', self.text, REPORT):
            self.analyse(text=self.text)
            (self.raw/'stdout.log').write_text(text)
            c={'seed':74, 'hardware':{'test':True}, 'build':{}}
            self.assertEqual(perf.analyse(self.raw,c,16), old.analyse(self.raw,c,16))

if __name__=='__main__': unittest.main()
