"""Fill missing native UI sprites with Claude Haiku. Use --apply to update sheets."""

import argparse
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
WORLD = ROOT / 'crates/app/src/world'
SHEETS = ROOT / 'assets/icons'
HEADER = re.compile(r'sprite (\d+) (\w+)')
COLOR = re.compile(r'([a-z]) ([0-9a-fA-F]{6})')
MACHINE_REFERENCES = (0, 5, 14, 15)
MACHINE_BRIEFS = {
    'exhaust_duct': 'Short flanged steel exhaust pipe with a soot-dark bore.',
    'chimney': 'Tall capped steel flue with vents and a dark opening.',
    'dust_filter': 'Steel filter box with a visibly pleated ochre cartridge.',
    'wet_scrubber': 'Water scrubber tower with a cyan sight glass and inlet pipe.',
    'gas_tank': 'Chemical tank with a domed cylinder, valve and muted green sight gauge.',
    'gas_generator': 'Industrial engine casing with an amber power symbol and side vent.',
}


def snake(name):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).lower()


def enum_ids(path, name):
    source = (WORLD / path).read_text(encoding='utf-8')
    body = re.search(rf'pub enum {name} \{{([^}}]*)\}}', source)[1]
    ids = {}
    value = 0
    for variant in re.sub(r'//[^\n]*', '', body).split(','):
        if not variant.strip():
            continue
        label, number = re.fullmatch(r'\s*(\w+)\s*(?:=\s*(\d+)\s*)?', variant).groups()
        if number is not None:
            value = int(number)
        ids[value] = snake(label)
        value += 1
    return ids


def catalog():
    expedition = (WORLD / 'expedition.rs').read_text(encoding='utf-8')
    names = re.search(r'pub const NAMES:[^=]+?= \[(.*?)\];', expedition, re.S)[1]
    resources = {i: re.sub(r'\W+', '_', name.lower()) for i, name in enumerate(re.findall(r'"([^"]+)"', names))}
    items = dict(enumerate(('bucket_empty', 'bucket_full', 'torch', 'shears', 'fishing_rod',
                            'boat', 'wooden_pickaxe', 'stone_pickaxe', 'iron_pickaxe')))
    items.update(enumerate(('spell_frame', 'ladder', 'vines', 'sapling', 'bore', 'gold_pan'), 36))
    items.update({20 + slot * 4 + tier: f'{material}_{piece}'
                  for slot, piece in enumerate(('helm', 'cuirass', 'greaves', 'boots'))
                  for tier, material in enumerate(('copper', 'steel', 'lumen', 'kinetic'))})
    for i, name in enum_ids('inventory.rs', 'FoodKind').items():
        items[10 + i if i < 10 else 50 + i] = name
    effects = enum_ids('progression/effects.rs', 'Kind')
    sprites = ROOT / 'crates/app/src/egui_ui/icons/sprites16.rs'
    poison_id = int(re.search(r'const MANA_POISONING: usize = (\d+);', sprites.read_text(encoding='utf-8'))[1])
    if poison_id in effects:
        raise ValueError('Mana poisoning ID overlaps an effect')
    effects[poison_id] = 'mana_poisoning'
    return {'resources': resources, 'items': items, 'machines': enum_ids('factory.rs', 'Kind'), 'effects': effects}


def validate(body, generated=False):
    palette = {'o'}
    rows = []
    for line in body:
        if color := COLOR.fullmatch(line):
            key = color[1]
            if rows or key in palette:
                raise ValueError('late or duplicate palette key')
            palette.add(key)
        elif len(line) == 16 and set(line) <= palette | {'.'}:
            rows.append(line)
        else:
            raise ValueError(f'invalid sprite line {line!r}')
    if len(rows) != 16:
        raise ValueError('expected 16 rows')
    pixels = ''.join(rows)
    if generated and (pixels.count('.') < 16 or pixels.count('o') < 8 or
                      256 - pixels.count('.') < 40 or len(set(pixels) - {'.', 'o'}) < 3):
        raise ValueError('sprite needs a transparent background, outline and shaded silhouette')


def parse_sheet(source):
    lines = [line for line in source.splitlines() if line and not line.startswith('#')]
    if not lines or not re.fullmatch(r'outline [0-9a-fA-F]{6}', lines[0]):
        raise ValueError('missing sheet outline')
    sprites = {}
    start = 1
    while start < len(lines):
        match = HEADER.fullmatch(lines[start])
        if not match:
            raise ValueError(f'invalid header {lines[start]!r}')
        end = lines.index('end', start + 1)
        sprite_id = int(match[1])
        if sprite_id in sprites:
            raise ValueError(f'duplicate sprite ID {sprite_id}')
        validate(lines[start + 1:end])
        sprites[sprite_id] = '\n'.join(lines[start:end + 1])
        start = end + 1
    return lines[0], sprites


def generate(claude, sheet, sprite_id, name, outline, references):
    header = f'sprite {sprite_id} {name}'
    subject = MACHINE_BRIEFS.get(name, name.replace('_', ' '))
    prompt = f'''Author one native 16x16 inventory pixel-art sprite for the {sheet} sheet.
Subject: {subject}
Return exactly one plain-text block starting with `{header}`. Palette lines
contain only a single letter, a space, and six hex digits, for example `a 3c4046`.
Then write exactly 16 rows of 16 characters and a final line containing only `end`.
Use `.` for transparency and `o` for the shared outline ({outline}). Do not define `o`.
Use a tight silhouette, crisp clusters of pixels, and three or four shades per
material lit from the upper left. Read clearly at 16px. Keep at least a one-pixel
transparent margin. Match the reference sprites' proportions, materials and contrast.
For industrial machines use shared casing a=3c4046 b=5c646c c=8a949c d=c8a050
and interior i=1c1c22 where appropriate. Give the machine a distinct functional
silhouette or a bold central symbol. No letters, gradients or antialiasing.

Existing references:
{references}
'''
    command = [claude, '-p', '--model', 'haiku', '--tools', '', '--setting-sources', '',
               '--strict-mcp-config', '--no-session-persistence', prompt]
    with tempfile.TemporaryDirectory(prefix='voxel-sprite-haiku-', ignore_cleanup_errors=True) as scratch:
        result = subprocess.run(command, stdin=subprocess.DEVNULL, cwd=scratch, capture_output=True,
                                encoding='utf-8', timeout=180)
    if result.returncode:
        raise RuntimeError((result.stderr or result.stdout).strip()[:600])
    lines = [line.strip() for line in result.stdout.splitlines()]
    if lines.count(header) != 1 or 'end' not in lines[lines.index(header) + 1:]:
        raise ValueError(f'Haiku did not return the requested sprite block: {result.stdout.strip()[:240]}')
    start = lines.index(header) + 1
    body = []
    for line in lines[start:lines.index('end', start)]:
        if not line:
            continue
        line = line.removeprefix('key ')
        # Repair one-pixel width errors without removing isolated details.
        if len(line) == 15 and line.startswith('.'):
            line += '.'
        elif len(line) == 17 and line.startswith('.') and line.endswith('.'):
            runs = list(re.finditer(r'(.)\1{2,}', line))
            if runs:
                run = max(runs, key=lambda part: len(part[0]))
                line = line[:run.start()] + line[run.start() + 1:]
        body.append(line)
    first_row = next((i for i, line in enumerate(body) if len(line) == 16), len(body))
    if len(body) - first_row == 15 and body[-1] == '.' * 16:
        body.append('.' * 16)
    elif len(body) - first_row == 17 and body[-2:] == ['.' * 16] * 2:
        body.pop()
    validate(body, generated=True)
    return '\n'.join([header, *body, 'end'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--apply', action='store_true', help='delegate all missing sprites and append validated results')
    mode.add_argument('--check', action='store_true', help='exit nonzero if a mapped sprite is missing')
    args = parser.parse_args()
    batches = []
    for sheet, expected in catalog().items():
        path = SHEETS / f'{sheet}16.txt'
        original = path.read_bytes()
        outline, existing = parse_sheet(original.decode('utf-8'))
        missing = sorted(expected.keys() - existing.keys())
        if missing:
            refs = MACHINE_REFERENCES if sheet == 'machines' else tuple(existing)[:4]
            references = '\n\n'.join(existing[i] for i in refs if i in existing)
            jobs = [(sheet, i, expected[i], outline, references) for i in missing]
            batches.append((path, original, jobs))
            for _, i, name, _, _ in jobs:
                print(f'{path.name} {i} {name}', flush=True)
    if not batches:
        print('All mapped UI sprites are present.')
        return 0
    if not args.apply:
        return int(args.check)

    claude = shutil.which('claude')
    if not claude:
        parser.error('Claude Code is required for --apply')
    failed = False
    with ThreadPoolExecutor(max_workers=3) as pool:
        pending = [(path, original, [(job, pool.submit(generate, claude, *job)) for job in jobs])
                   for path, original, jobs in batches]
        for path, original, jobs in pending:
            additions = []
            for job, future in jobs:
                try:
                    additions.append(future.result())
                except (ValueError, RuntimeError, OSError, subprocess.TimeoutExpired) as error:
                    failed = True
                    print(f'{path.name} {job[1]} failed: {error}', flush=True)
            if additions:
                if path.read_bytes() != original:
                    print(f'{path.name} changed during generation. Rerun to use the latest sheet.')
                    failed = True
                    continue
                newline = '\r\n' if b'\r\n' in original else '\n'
                suffix = '\n\n'.join(additions) + '\n'
                separator = '\n' if original.endswith(b'\n') else '\n\n'
                with path.open('ab') as out:
                    out.write((separator + suffix).replace('\n', newline).encode('utf-8'))
                print(f'Added {len(additions)} sprites to {path.name}.')
    return int(failed)


if __name__ == '__main__':
    raise SystemExit(main())
