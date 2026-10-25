"""Player-visible text check for crates/app/src/egui_ui/UI_RULES.md.

Scans string literals in UI code and the world's chat lines (`say(...)`).
Errors: banned words, second person, exclamation marks, dev text.
Warnings: instructional openings and rule-of-three lists.

    python -I scripts/ui_text_lint.py               report every file
    python -I scripts/ui_text_lint.py --strict F..  exit 1 on errors in F
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCAN = ["crates/app/src/egui_ui", "crates/app/src/world"]

BANNED = (
    "amazing powerful ultimate epic seamless seamlessly unleash journey immersive "
    "explore harness mysterious ancient forgotten echo echoes essence robust "
    "leverage delve elevate comprehensive crucial ensure streamline elegant"
).split()
# Literal material and item names are allowed to contain a banned word.
NAMES = ["Ancient circuit", "Wild essence", "Resonant crystal", "Arcane core", "Echo Spindle", "Cinder echo", "Echo step"]

LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')
WORD = re.compile(r"\b(" + "|".join(BANNED) + r")\b", re.I)
SECOND = re.compile(r"\b(you|your|you're|yourself|let's)\b", re.I)
DEV = re.compile(r"\b(TODO|FIXME|Coming soon)\b|::|\b[a-z]+_[a-z_]+\b")
INSTRUCT = re.compile(r"^(Click|Press|Use|Drag|Tap|Hold|Try|Make sure)\b")
TRICOLON = re.compile(r"\b\w+, \w+,? and \w+\b")

# Literals that are identifiers, not player text.
IGNORE_CALL = re.compile(r"(println!|eprintln!|print!|log::|info!|warn!\(|debug!|trace!|error!\(|bail!|anyhow!|ensure!|context\(|Id::new|id_salt|from_id_salt|get_temp|insert_temp|env::var|include_str|module\(|port\(|module ==|\.module\b|Grid::new|#\[path|path =|expect\(|panic!|format!\(\"\{)")


def player_text(s):
    return re.search(r"[A-Za-z]{2,}", s) and " " in s.strip() or re.match(r"^[A-Z][a-z]+$", s)


def scan(path):
    errors, warnings = [], []
    in_test = False
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        code = line.split("//", 1)[0] if '"' not in line.split("//", 1)[0][:0] else line
        if "#[cfg(test)]" in line:
            in_test = True
        if in_test or line.lstrip().startswith("//") or IGNORE_CALL.search(line):
            continue
        for lit in LITERAL.findall(code):
            if not player_text(lit):
                continue
            text = lit
            for name in NAMES:
                text = re.sub(re.escape(name), "", text, flags=re.I)
            where = f"{path.relative_to(ROOT)}:{n}"
            if m := WORD.search(text):
                errors.append(f"{where}: banned word '{m.group(1)}': \"{lit}\"")
            if m := SECOND.search(text):
                errors.append(f"{where}: second person '{m.group(1)}': \"{lit}\"")
            if "!" in text.replace("!=", ""):
                errors.append(f"{where}: exclamation: \"{lit}\"")
            if (m := DEV.search(re.sub(r"\{[^}]*\}", "", text))) and not re.search(r"\.(rs|ron|png|wgsl)\b", text):
                errors.append(f"{where}: dev text '{m.group(0)}': \"{lit}\"")
            if INSTRUCT.search(text):
                warnings.append(f"{where}: instruction: \"{lit}\"")
            if TRICOLON.search(text):
                warnings.append(f"{where}: rule of three: \"{lit}\"")
    return errors, warnings


def main(argv):
    sys.stdout.reconfigure(encoding="utf-8")
    strict = [ROOT / p for p in argv[argv.index("--strict") + 1:]] if "--strict" in argv else []
    files = strict or [p for d in SCAN for p in sorted((ROOT / d).rglob("*.rs"))]
    total_e, total_w, failed = 0, 0, False
    per_file = []
    for f in files:
        e, w = scan(f)
        total_e += len(e)
        total_w += len(w)
        if e or w:
            per_file.append((len(e), len(w), f, e, w))
        failed |= bool(strict and e)
    per_file.sort(key=lambda r: (-r[0], -r[1]))
    for ne, nw, f, e, w in per_file:
        print(f"{f.relative_to(ROOT)}: {ne} errors, {nw} warnings")
        for line in (e + w)[: (None if strict else 6)]:
            print("  " + line)
    print(f"total: {total_e} errors, {total_w} warnings in {len(files)} files")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
