import copy
import unittest
import test_foundry_perf_headless as base
import foundry_perf_headless as h


def receipt(**updates):
    c = dict.fromkeys(h.REGIONAL_FIELDS, 0)
    c.update(updates)
    return '[relief-cache-result] ReliefCacheStats { ' + ', '.join(f'{k}: {v}' for k,v in c.items()) + ' }'


class RegionalTests(unittest.TestCase):
    setUp = base.HeadlessTests.setUp
    analyse = base.HeadlessTests.analyse
    def test_states_and_backward_compatibility(self):
        cases = [('bypass-only', dict(bypassed=489)), ('hit-only', dict(attempts=2,hits=2,bytes_read=100)),
                 ('mixed', dict(attempts=2,hits=1,misses=1,stored=1,bytes_read=100,bytes_written=50)),
                 ('miss-only', dict(attempts=1,misses=1,write_skipped=1)), ('inactive', {})]
        self.assertEqual(self.analyse()['workload']['regional_cache']['observation'], 'unobserved')
        for state, values in cases:
            c=copy.deepcopy(self.c);c['instrumentation']['diagnostic_kinds']=['relief-cache-result']
            report=self.analyse(self.text+'\n'+receipt(**values),c)
            self.assertEqual(report['workload']['regional_cache']['observation'],state)
            self.assertIsNone(report['telemetry'])
            self.assertEqual(report['headless']['diagnostics'][-1]['counts']['attempts'],values.get('attempts',0))

    def test_malformed_counts(self):
        good=receipt(bypassed=3)
        bad=[good+ '\n'+good, good.replace('hits: 0','hits: -1'),good.replace('hits: 0','hits: NaN'),
             good.replace('hits: 0','hits: 0, hits: 0'),good.replace('hits: 0, ','') ,
             good.replace('hits: 0','hits: 18446744073709551616'),good+' junk',
             receipt(attempts=1),receipt(misses=1),receipt(invalid=1),receipt(write_errors=1),
             receipt(bytes_read=1),receipt(bytes_written=1),receipt(attempts=1,hits=1),
             receipt(attempts=1,hits=1,bytes_read=524289)]
        for text in bad:
            with self.subTest(text=text):
                with self.assertRaises(ValueError):h.regional_cache(text)
        oversized=receipt(attempts=1,misses=1,invalid=1,write_skipped=1,bytes_read=450*1024)
        self.assertEqual(h.regional_cache(oversized)['observation'],'miss-only')
        # A digest-valid empty payload may fail semantic decode with zero bytes read.
        self.assertEqual(h.regional_cache(receipt(attempts=1,misses=1,invalid=1,write_skipped=1))['observation'],'miss-only')

    def test_cold_warm_comparison_identity_and_declared_instrumentation(self):
        c=copy.deepcopy(self.c);c['instrumentation']['diagnostic_kinds']=['relief-cache-result']
        cold=self.analyse(self.text+'\n'+receipt(attempts=1,misses=1,stored=1,bytes_written=50),c)
        warm=self.analyse(self.text+'\n'+receipt(attempts=1,hits=1,bytes_read=50),c)
        self.assertNotEqual(cold['workload'],warm['workload'])
        result=h.compare([cold],[warm],0.05)
        self.assertEqual(result['status'],'incomparable')
        with self.assertRaises(ValueError):self.analyse(self.text+'\n'+receipt(bypassed=1))

if __name__=='__main__':unittest.main()
