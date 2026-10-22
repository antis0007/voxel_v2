#!/usr/bin/env python3
"""Run a command (or read stdin) and print a short, agent-friendly diagnostic summary.

Usage:
    python -I scripts/filter.py -- <command...>     run the command, filter its combined output
    <command> 2>&1 | python -I scripts/filter.py     filter stdin

Output rules: errors first (deduplicated, in order), then warnings collapsed to
per-file counts, then any other non-empty lines (deduplicated), capped at
MAX_LINES in total. Exit status is the command's (or 0 when reading stdin).
Expects `--message-format=short`, which puts each diagnostic on one line.
"""
import re
import subprocess
import sys
from collections import Counter

MAX_LINES = 60
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
DIAG = re.compile(r"^(?P<loc>\S+?:\d+:\d+): (?P<kind>error|warning)(\[[^\]]*\])?: (?P<msg>.*)$")
NOISE = (
    "= note:", "= help:", "note:", "help:",
    "warning: `minecraft_style_voxel_prototype` (bin",
    "warning: build failed", "For more information",
    "Some errors have detailed explanations",
    "Validated ", "Compiling ", "Checking ", "Finished ", "Running ", "Blocking ",
)
SUMMARY = re.compile(r"^(warning|error): .* generated \d+ warnings?")


HEADER = re.compile(r"^(?P<kind>error|warning)(\[[^\]]*\])?: (?P<msg>.*)$")
ARROW = re.compile(r"^\s*--> (?P<loc>\S+?:\d+:\d+)$")
GUTTER = re.compile(r"^\s*(\d+\s*\|.*|\|.*|\.\.\.|= (note|help):.*|\^+.*)$")


def filter_lines(raw: str) -> list[str]:
    """Collapse rustc diagnostics in either `--message-format=short` (one line)
    or the default long format (header, `-->` location, gutter block)."""
    errors, others = [], []
    warnings: Counter[str] = Counter()
    seen = set()
    pending = None  # (kind, msg) of a long-format header awaiting its `-->`

    def flush(loc: str) -> None:
        nonlocal pending
        if pending is None:
            return
        kind, msg = pending
        pending = None
        if kind == "error":
            errors.append(f"{loc}: {msg}")
        else:
            warnings[loc.split(":")[0]] += 1

    for line in raw.splitlines():
        line = ANSI.sub("", line).rstrip()
        stripped = line.strip()
        if not stripped:
            continue
        m = ARROW.match(line)
        if m:
            flush(m["loc"])
            continue
        if GUTTER.match(line):
            continue
        flush("<no location>")
        if line in seen:
            continue
        seen.add(line)
        m = DIAG.match(line)
        if m:
            if m["kind"] == "error":
                errors.append(f"{m['loc']}: {m['msg']}")
            else:
                warnings[m["loc"].split(":")[0]] += 1
            continue
        m = HEADER.match(line)
        if m and not SUMMARY.match(stripped) and not stripped.startswith(NOISE):
            pending = (m["kind"], m["msg"])
            continue
        if SUMMARY.match(stripped) or stripped.startswith(NOISE):
            continue
        others.append(line)
    flush("<no location>")
    out = []
    if errors:
        out.append(f"{len(errors)} error(s):")
        out += errors
    if warnings:
        total = sum(warnings.values())
        top = ", ".join(f"{f} {n}" for f, n in warnings.most_common(5))
        out.append(f"{total} warning(s) in {len(warnings)} file(s); top: {top}")
    out += others
    if len(out) > MAX_LINES:
        hidden = len(out) - (MAX_LINES - 1)
        out = out[: MAX_LINES - 1] + [f"... {hidden} more line(s) hidden; rerun the raw command to see them"]
    return out


def main() -> int:
    # Windows consoles default to cp1252; nextest prints box-drawing characters.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    argv = sys.argv[1:]
    if argv and argv[0] == "--":
        argv = argv[1:]
    if argv:
        proc = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
        raw, code = proc.stdout, proc.returncode
    else:
        raw, code = sys.stdin.read(), 0
    lines = filter_lines(raw)
    if lines:
        print("\n".join(lines))
    elif code == 0:
        print("ok")
    return code


if __name__ == "__main__":
    sys.exit(main())
