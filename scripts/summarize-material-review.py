"""Summarize native capture provenance, bottlenecks and matched frame costs."""
import argparse
import csv
import hashlib
import json
import re
import statistics
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('evidence', type=Path)
parser.add_argument('--before-prefix', default='before')
parser.add_argument('--after-prefix', default='after')
args = parser.parse_args()
owners = {
    'gameplay_ms': 'simulation::fixed and world/adventure; inspect builder and pathfinding counters',
    'stream_ms': 'world streaming/meshing; correlate worker_pending, uploads and visible_missing',
    'render_cpu_ms': 'render submission and driver wait; not pure CPU shader execution',
    'terrain_ms': 'world_shader.wgsl and terrain draws; check triangles, material footprint and debug mode',
    'cloud_ms': 'cloud raymarch; compare cloud divisor and sky/weather arguments',
    'horizon_ms': 'CPU horizon build/update; inspect representation and residency',
    'gpu_shadow_ms': 'shadow pass; correlate shadow_draws and lighting configuration',
}


def stats(values):
    values = sorted(values)
    return {'mean': statistics.mean(values), 'p50': statistics.median(values),
            'p95': values[int((len(values)-1)*.95)],
            'p99': values[int((len(values)-1)*.99)], 'max': values[-1]}


def read_run(folder):
    manifest = json.loads((folder/'manifest.json').read_text(encoding='utf-8-sig'))
    stdout = (folder/'stdout.log').read_text(errors='replace') if (folder/'stdout.log').exists() else ''
    stderr = (folder/'stderr.log').read_text(errors='replace') if (folder/'stderr.log').exists() else ''
    record = {'manifest': manifest, 'completed': '[benchmark] avg_fps=' in stdout,
              'summary': [line for line in stdout.splitlines() if '[benchmark]' in line],
              'shader': re.findall(r'world_fnv1a64=([a-f0-9]+)', stdout + stderr),
              'images': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.glob('*.png')},
              'shader_build_match': 'build_match=true' in stdout and 'build_match=false' not in stdout,
              'startup_stages_ms': {name: float(ms) for name, ms in re.findall(
                  r'\[startup-timing\] (?:renderer stage=)?([A-Za-z_]+) ms=([\d.]+)', stdout)},
              'alerts': []}
    if not record['completed']:
        record['alerts'].append('Benchmark incomplete; do not use it as performance evidence.')
    if not record['shader_build_match']:
        record['alerts'].append('Shader provenance missing or mismatched; rebuild before visual attribution.')
    if manifest.get('other_processes'):
        record['alerts'].append('Other native/compiler processes were active at launch; timings may be contaminated.')
    rows = []
    if (folder/'frames.csv').exists():
        with (folder/'frames.csv').open(encoding='utf-8-sig', newline='') as source:
            rows = list(csv.DictReader(source))
    record['frames'] = len(rows)
    metrics = {}
    if rows:
        for name in rows[0]:
            if not any(key in name for key in ('ms', 'draw', 'triang', 'resident', 'missing', 'pending', 'bytes', 'parked')):
                continue
            try:
                values = [float(row[name]) for row in rows if row[name] not in ('', 'NA', 'nan')]
            except ValueError:
                continue
            if values:
                metrics[name] = stats(values)
        record['worst_frames'] = sorted(rows, key=lambda row: float(row['total_ms']), reverse=True)[:8]
    record['metrics'] = metrics
    record['bottleneck_context'] = sorted(
        [{'metric': key, **metrics[key], 'owner_hint': hint} for key, hint in owners.items() if key in metrics],
        key=lambda row: row['mean'], reverse=True)
    for key in ('visible_missing', 'near_missing'):
        if metrics.get(key, {}).get('max', 0) > 0:
            record['alerts'].append(f'{key} was nonzero; inspect coverage before comparing costs.')
    if metrics.get('total_ms', {}).get('p95', 0) > 16.67:
        record['alerts'].append('Frame p95 exceeded 16.67 ms; inspect worst_frames and bottleneck_context.')
    if record['completed'] and not rows:
        record['alerts'].append('No frame CSV; pass costs cannot be independently summarized.')
    return record


output = {folder.name: read_run(folder) for folder in sorted(args.evidence.iterdir())
          if (folder/'manifest.json').exists()}
comparisons = {}
prefix = args.after_prefix + '-'
for name, after in output.items():
    if not name.startswith(prefix):
        continue
    case = name[len(prefix):]
    before = output.get(args.before_prefix + '-' + case)
    if before is None:
        continue
    pair = {'arguments_match': before['manifest']['arguments'] == after['manifest']['arguments'],
            'complete': before['completed'] and after['completed'], 'metrics': {},
            'alerts': list(before['alerts']) + list(after['alerts'])}
    if not pair['arguments_match']:
        pair['alerts'].append('Arguments differ; these are not matched performance samples.')
    for metric in ('total_ms', 'gameplay_ms', 'stream_ms', 'render_cpu_ms', 'gpu_pass_ms',
                   'terrain_ms', 'terrain_triangles', 'terrain_draws', 'cloud_ms'):
        b, a = before['metrics'].get(metric), after['metrics'].get(metric)
        if b and a:
            delta = a['mean'] - b['mean']
            pair['metrics'][metric] = {'before_mean': b['mean'], 'after_mean': a['mean'],
                                      'before_p95': b['p95'], 'after_p95': a['p95'], 'mean_delta': delta}
            if metric in ('terrain_triangles', 'terrain_draws') and b['p50'] != a['p50']:
                pair['alerts'].append(f'{metric} workload differs; inspect geometry/streaming before claiming a speed gain.')
            if metric in ('total_ms', 'terrain_ms', 'gpu_pass_ms') and delta > max(.10, b['mean']*.05):
                pair['alerts'].append(f'{metric} mean rose by {delta:.3f} ms; repeat without competing work and investigate.')
    comparisons[case] = pair
path = args.evidence/'material-review-summary.json'
path.write_text(json.dumps({'timing_note': 'GPU timings cover selected timestamped passes, not total GPU frame time. '
                           'CPU intervals overlap; do not sum them. Process contamination is sampled at launch.',
                           'runs': output, 'comparisons': comparisons}, indent=2))
for name, pair in comparisons.items():
    metric = pair['metrics'].get('terrain_ms', {})
    print(name, 'matched=' + str(pair['arguments_match']), 'terrain_ms=' + str(metric), 'alerts=' + str(pair['alerts']))
print(path)
