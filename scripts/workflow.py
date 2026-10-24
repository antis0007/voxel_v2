#!/usr/bin/env python3
"""Task verification controller: derive required checks from the real diff, run them, store evidence.

    python -I scripts/workflow.py status          required checks and what is satisfied
    python -I scripts/workflow.py verify [CHECK...]   run missing/stale automatic checks (or only the named ones)
    python -I scripts/workflow.py attest ID --note TEXT [--evidence PATH]   record a manual check
    python -I scripts/workflow.py reset           start a fresh task from the current tree
    python -I scripts/workflow.py claim PATH... --note TEXT   take ownership of files written outside the hooks
    python -I scripts/workflow.py adopt-ownership   switch a legacy session to ownership-based attribution
    python -I scripts/workflow.py owners PATH...  which sessions own a path (who else is editing it)
    python -I scripts/workflow.py disown PATH... --note TEXT   drop a peer's write a hook attributed to you (audited)
    python -I scripts/workflow.py hook prompt|stop|pre|post   Claude Code hook entry points

Policy: .claude/verify/policy.json. State, logs and evidence: target/workflow/ (untracked).
One task per Claude Code session (target/workflow/sessions/<CLAUDE_CODE_SESSION_ID>.json).
Evidence is bound to the content of every changed file, so any later edit makes it stale.

Ownership: a session's task is the set of files it wrote. The pre/post hooks bracket each writing
tool call and record which paths changed in between; `claim` adds files written by other means.
Owned files stay in the task for its whole life, even when a peer edits them later; the one exit is
`disown` for a peer's write that landed inside one of your calls, which is logged and announced at Stop. Sessions
created before ownership (no "owned" key) keep the old rule until `adopt-ownership` runs once.
"""
import fnmatch
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / ".claude" / "verify" / "policy.json"
STATE_DIR = ROOT / "target" / "workflow"
SESSION_DIR = STATE_DIR / "sessions"
IGNORED = ("target/", ".code-advisor/", ".git/")


def policy():
    return json.loads(POLICY_PATH.read_text(encoding="utf-8"))


def git(*args):
    """Fails closed: a git error must never read as 'no changes'."""
    out = subprocess.run(["git", "--no-pager", "-c", "core.quotepath=off", *args], cwd=ROOT, capture_output=True,
                         encoding="utf-8", errors="replace")
    if out.returncode != 0:
        raise RuntimeError(f"git {args[0]} failed: {out.stderr.strip()[:200]}")
    return out.stdout


def file_sha(rel):
    path = ROOT / rel
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else "deleted"


def dirty_paths(base):
    names = set(git("diff", "--name-only", "--no-renames", "-z", base).split("\0"))
    names |= set(git("ls-files", "--others", "--exclude-standard", "-z").split("\0"))
    return sorted(n for n in names if n and not n.startswith(IGNORED))


def cached_sha(rel, cache):
    """file_sha, reused while (mtime_ns, size) are unchanged. Keeps per-tool-call hooks well under 300 ms."""
    path = ROOT / rel
    if not path.is_file():
        return "deleted"
    st = path.stat()
    hit = cache.get(rel)
    if hit and hit[:2] == [st.st_mtime_ns, st.st_size]:
        return hit[2]
    sha = file_sha(rel)
    cache[rel] = [st.st_mtime_ns, st.st_size, sha]
    return sha


def dirty_snapshot(base, cache=None):
    """{path: sha} of everything dirty against base. Deleted files hash to 'deleted'.
    With a cache dict, entries are read and refreshed in place."""
    if cache is None:
        return {p: file_sha(p) for p in dirty_paths(base)}
    snap = {p: cached_sha(p, cache) for p in dirty_paths(base)}
    for stale in set(cache) - set(snap):  # forget paths that are no longer dirty
        del cache[stale]
    return snap


def repo_rel(path):
    """Repo-relative POSIX path for a tool's file_path, or None when it lies outside the checkout."""
    p = Path(path)
    p = p if p.is_absolute() else ROOT / p
    try:
        return p.resolve().relative_to(ROOT.resolve()).as_posix()
    except (OSError, ValueError):
        return None


def session_key(session=""):
    """One task per Claude Code session, so parallel sessions in one checkout never reset each other."""
    raw = session or os.environ.get("CLAUDE_CODE_SESSION_ID", "")
    return re.sub(r"[^A-Za-z0-9_-]", "_", raw) or "default"


def state_path(key):
    return SESSION_DIR / f"{key}.json"


def load_state(session=""):
    try:
        return json.loads(state_path(session_key(session)).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def save_state(state):
    SESSION_DIR.mkdir(parents=True, exist_ok=True)
    path = state_path(state["session"])
    tmp = path.with_suffix(f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(state, indent=1), encoding="utf-8")
    tmp.replace(path)


def prune_sessions(keep, days=14):
    cutoff = time.time() - days * 86400
    for path in SESSION_DIR.glob("*.json"):
        if path.stem != keep and path.stat().st_mtime < cutoff:
            path.unlink(missing_ok=True)


def new_task(session=""):
    key = session_key(session)
    base = git("rev-parse", "HEAD").strip()
    pre = dirty_snapshot(base)
    prune_sessions(key)
    return {"session": key, "base": base, "pre": pre, "started": time.time(), "evidence": {}, "failed": {},
            "failures": {}, "continuations": 0, "failed_runs": 0, "owned": [], "claims": []}


def ensure_task(session=""):
    state = load_state(session)
    if state is None:
        state = new_task(session)
        save_state(state)
    return state


def current_head():
    return git("rev-parse", "HEAD").strip()


def changed_files(state, head=None):
    """The session's task. Legacy sessions: anything dirty that changed since the first prompt.
    Ownership sessions: only files this session wrote, judged the same way, so peers' files drop out.
    Dirty means against the current HEAD, not the task's base: committed work leaves every task. (Hook
    snapshots keep the fixed base, so a peer's commit during a tool call never reads as a write.)"""
    pre = state["pre"]
    dirty = dirty_paths(head or current_head())
    if "owned" not in state:
        return [p for p in dirty if file_sha(p) != pre.get(p)]
    owned = owned_set(state)
    return [p for p in dirty if p in owned and file_sha(p) != pre.get(p)]


SHELL_TOOLS = ("Bash", "PowerShell")


def cache_path(key):
    """Sha cache for hooks. Its own file, so hook saves never rewrite the session state object."""
    return SESSION_DIR / f"{key}.shas.json"


def load_cache(key):
    try:
        return json.loads(cache_path(key).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def save_cache(key, cache):
    SESSION_DIR.mkdir(parents=True, exist_ok=True)
    tmp = cache_path(key).with_suffix(f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(cache), encoding="utf-8")
    tmp.replace(cache_path(key))


def owned_path(key):
    """Append-only ownership log. Hooks only ever append here, so concurrent calls cannot overwrite each other."""
    return SESSION_DIR / f"{key}.owned.txt"


def owned_entries(state):
    """(path, source) per attribution, in log order. Sources: `explicit` (a tool named the file), `window`
    (a shell call's before/after diff, which can include a peer's concurrent write), `claim`, and `legacy`
    for older lines that recorded none."""
    try:
        lines = owned_path(state["session"]).read_text(encoding="utf-8").splitlines()
    except OSError:
        return []
    out = []
    for line in lines:
        fields = line.split("\t")
        if fields[0]:
            out.append((fields[0], fields[2] if len(fields) > 2 else "legacy"))
    return out


def disowned_since(entry):
    """Log position a disown took effect at. A malformed entry returns -1, so the path stays owned: the gate
    errs toward requiring checks."""
    try:
        return int(entry["seq"])
    except (TypeError, KeyError, ValueError):
        return -1


def owned_set(state):
    """Claimed paths plus attributed paths, minus paths disowned after their last attribution. Ordering is by
    position in the append-only log, never by wall clock, so a clock step cannot lose a later re-own."""
    disowned = state.get("disowned") if isinstance(state.get("disowned"), dict) else {}
    owned = {p for p in state.get("owned", []) if p not in disowned or disowned_since(disowned[p]) < 0}
    last = {}
    for index, (path, _) in enumerate(owned_entries(state)):
        last[path] = index
    owned |= {p for p, i in last.items() if p not in disowned or i >= disowned_since(disowned[p])}
    return owned


def record_ownership(state, before, after, explicit=(), explicit_source="explicit"):
    """Appends every path whose sha differs between two snapshots (source `window`) plus explicit paths
    (source `explicit_source`) to the owned log."""
    named = {p for p in explicit if p}
    window = {p for p in set(before) | set(after) if before.get(p) != after.get(p)} - named
    if named or window:
        SESSION_DIR.mkdir(parents=True, exist_ok=True)
        now = time.time()
        rows = [(p, "window") for p in window] + [(p, explicit_source) for p in named]
        with owned_path(state["session"]).open("a", encoding="utf-8") as fh:
            fh.write("".join(f"{p}\t{now:.3f}\t{src}\n" for p, src in sorted(rows)))
    return sorted(named | window)


def file_diffs(state, files, head=None):
    """Per file: added/removed lines of tracked changes, or the whole content of an untracked source."""
    out = {f: "" for f in files}
    # Anchored: a diff line that happens to contain the marker (this file has one) must not start a block.
    for block in re.split(r"^diff --git ", git("diff", "-U0", "--no-renames", head or current_head()), flags=re.M)[1:]:
        header = block.split("\n", 1)[0]
        for f in files:
            if header.endswith(" b/" + f):
                out[f] += "\n".join(ln for ln in block.splitlines() if ln[:1] in "+-" and ln[:3] not in ("+++", "---")) + "\n"
    untracked = set(git("ls-files", "--others", "--exclude-standard", "-z").split("\0"))
    for f in set(files) & untracked:
        path = ROOT / f
        if path.is_file() and path.suffix in (".rs", ".wgsl", ".toml", ".py", ".ron"):
            out[f] += "".join("+" + line for line in path.read_text(encoding="utf-8", errors="replace").splitlines(True))
    return out


def diff_text(state, files):
    """Added/removed lines of the task's tracked changes plus the whole content of untracked sources."""
    return "".join(file_diffs(state, files).values())


def fingerprint(state, files, pol):
    parts = [(f, file_sha(f)) for f in files]
    for name in ("Cargo.lock", "rust-toolchain.toml", "rust-toolchain"):
        parts.append((name, file_sha(name)))
    parts.append(("policy", str(pol["version"])))
    return hashlib.sha256(json.dumps(parts).encode()).hexdigest()[:16]


def matches(path, patterns):
    return any(fnmatch.fnmatch(path, pat) for pat in patterns)


def classifier_extras(state, fp):
    """Review checks from a classifier label, valid only for the exact diff it was given."""
    done = state.get("classified")
    return done["checks"] if done and done.get("fp") == fp else []


def required_checks(state, files, pol, extra=(), head=None):
    """Rules + observed failures + classifier extras. Nothing here can remove a rule-required check.
    Also returns each check's scope: the files whose routes require it (all files for failures and
    classifier extras), which is what a manual check's evidence is bound to."""
    if not files:
        return {}, [], {}
    diffs = file_diffs(state, files, head)
    why, routes, scope = {}, [], {}
    for route in pol["routes"]:
        hit = set()
        if "only_paths" in route and all(matches(f, route["only_paths"]) for f in files):
            hit = set(files)
        if "paths" in route:
            hit |= {f for f in files if matches(f, route["paths"])}
        if "diff" in route:
            hit |= {f for f in files if re.search(route["diff"], diffs[f], re.M)}
        if hit:
            routes.append(route["id"])
            for check in route["checks"]:
                why.setdefault(check, []).append(route["id"])
                scope.setdefault(check, set()).update(hit)
    for check in state["failed"]:
        if check in pol["checks"]:
            why.setdefault(check, []).append("observed_failure")
            scope[check] = set(files)
    for check in extra:
        if check in pol["checks"]:
            why.setdefault(check, []).append("classifier")
            scope[check] = set(files)
    # A required covering check (a full test run) makes its covered, cheaper checks redundant.
    for check in list(why):
        for covered in pol["checks"][check].get("covers", []):
            why.pop(covered, None)
            scope.pop(covered, None)
    return why, routes, {c: sorted(s) for c, s in scope.items()}


def doc_patterns(pol):
    """Paths that cannot change what the game does or draws: the policy's `inert_paths` (docs, agent config,
    the gate's own scripts), else the docs-only route's patterns. A test checks the game embeds none of them."""
    return pol.get("inert_paths") or next((r["only_paths"] for r in pol["routes"] if r.get("id") == "docs_only"), [])


def check_fingerprints(state, files, fp, why, scope, pol):
    """Automatic checks (builds, test runs) are bound to the whole task. A manual check (captures, reviews) is
    bound to the files that require it plus every non-doc file in the task: any code, shader or content edit
    can change what a capture or review saw, so only doc edits leave that evidence standing."""
    docs = doc_patterns(pol)
    code = {f for f in files if not matches(f, docs)}
    return {c: fp if pol["checks"][c]["kind"] == "auto" else fingerprint(state, sorted(set(scope.get(c, files)) | code), pol)
            for c in why}


def satisfied(state, check, fp):
    ev = state["evidence"].get(check)
    return bool(ev) and ev["fp"] == fp and ev["exit"] == 0


def assess(state, pol):
    """(files, task fingerprint, why, routes, missing, per-check fingerprints). HEAD is read once, so a peer's
    commit mid-assessment cannot leave a task file without its diff."""
    head = current_head()
    files = changed_files(state, head)
    fp = fingerprint(state, files, pol)
    why, routes, scope = required_checks(state, files, pol, classifier_extras(state, fp), head)
    fps = check_fingerprints(state, files, fp, why, scope, pol)
    missing = [c for c in why if not satisfied(state, c, fps[c])]
    return files, fp, why, routes, missing, fps


def run_check(check, spec, pol):
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    log = STATE_DIR / "logs" / f"{check}-{time.time_ns()}.log"
    log.parent.mkdir(exist_ok=True)
    started = time.time()
    with log.open("wb") as out:
        proc = subprocess.Popen(spec["argv"], cwd=ROOT, stdout=out, stderr=subprocess.STDOUT)
        try:
            code = proc.wait(timeout=pol["limits"]["check_timeout_s"])
        except subprocess.TimeoutExpired:
            subprocess.run(["taskkill", "/F", "/T", "/PID", str(proc.pid)], capture_output=True)
            code = 124
    return code, log, time.time() - started


def cmd_verify(args):
    pol = policy()
    state = ensure_task()
    files, fp, why, routes, missing, fps = assess(state, pol)
    if not files:
        print("no task changes; nothing to verify")
        return 0
    if state["failed_runs"] > pol["limits"]["repair_attempts"]:
        print(f"BLOCKED: repair budget used ({state['failed_runs']} failing runs). Report the failures; `reset` starts over.")
        return 3
    print(f"changes={len(files)} routes={','.join(routes)} fingerprint={fp}")
    only = getattr(args, "checks", None) or []
    unknown = [c for c in only if c not in why or pol["checks"][c]["kind"] != "auto"]
    if unknown:
        print(f"not a required automatic check for this diff: {', '.join(unknown)}")
        return 2
    failed = False
    # Named checks run on their own, so an unrelated failure (say, a peer's snapshot breaking the full test
    # run) does not stop independent checks from being recorded. Nothing is skipped: the rest stay missing.
    for check in [c for c in missing if pol["checks"][c]["kind"] == "auto" and (not only or c in only)]:
        code, log, secs = run_check(check, pol["checks"][check], pol)
        state["evidence"][check] = {"fp": fps[check], "exit": code, "argv": pol["checks"][check]["argv"],
                                    "t": time.time(), "secs": round(secs, 1), "log": str(log.relative_to(ROOT))}
        print(f"{'PASS' if code == 0 else 'FAIL'} {check} ({secs:.0f}s) log={state['evidence'][check]['log']}")
        save_state(state)
        if code != 0:
            failed = True
            state["failed"][check] = True
            lines = [ln for ln in log.read_text(encoding="utf-8", errors="replace").splitlines() if ln.strip()][:30]
            sig = hashlib.sha256("\n".join(lines).encode()).hexdigest()[:12]
            if state["failures"].get(check) == sig:
                print("  same failure as the previous run (not repeated); see log")
            else:
                print("\n".join("  " + ln for ln in lines))
            state["failures"][check] = sig
            break  # later checks would only bury the first failure
        state["failed"].pop(check, None)
    if failed:
        state["failed_runs"] += 1
    elif not [c for c in missing if pol["checks"][c]["kind"] == "auto" and c in state["failed"]]:
        state["failed_runs"] = 0  # a clean run restores the repair budget
    save_state(state)
    pending = [c for c in missing if pol["checks"][c]["kind"] == "manual"]
    for check in pending:
        print(f"PENDING manual {check}: {pol['checks'][check]['what']}")
    rest = [c for c in missing if pol["checks"][c]["kind"] == "auto" and only and c not in only]
    for check in rest:
        print(f"PENDING auto {check}: not run (not named)")
    # A named run is never "verified": exit 5 while any other required automatic check is still missing.
    return 1 if failed else (5 if rest else (4 if pending else 0))


def cmd_status(args):
    pol = policy()
    state = ensure_task()
    files, fp, why, routes, missing, fps = assess(state, pol)
    owned = "legacy (every changed file)" if "owned" not in state else "ownership"
    print(f"base={state['base'][:8]} changes={len(files)} fingerprint={fp} routes={','.join(routes) or '-'} attribution={owned}")
    for check, reasons in why.items():
        state_word = "MISSING/STALE" if check in missing else "ok"
        bound = "" if fps[check] == fp else f" (bound to {fps[check]})"
        print(f"{state_word:14} {check} <- {','.join(reasons)}{bound}")
    for line in disowned_report(state):
        print(line)
    return 0


def cmd_attest(args):
    pol = policy()
    state = ensure_task()
    check = args.check
    if pol["checks"].get(check, {}).get("kind") != "manual":
        print(f"{check} is not a manual check")
        return 2
    if not args.evidence or not (ROOT / args.evidence).exists():
        print("--evidence <path> is required: a directory of before/after captures, or the saved review text")
        return 2
    target = ROOT / args.evidence
    fresh = [f for f in ([target] if target.is_file() else target.rglob("*")) if f.is_file() and f.stat().st_mtime >= state["started"]]
    images = [f for f in fresh if f.suffix.lower() in (".png", ".jpg", ".jpeg", ".mp4", ".webm")]
    if check in ("capture_ab", "seam_route") and len(images) < 2:
        print("capture checks need >= 2 image/video files created since the task started (before and after)")
        return 2
    if check.startswith("review") and not any(f.stat().st_size >= 200 for f in fresh):
        print("review checks need a saved review text (>= 200 bytes) created since the task started")
        return 2
    files, fp, why, routes, missing, fps = assess(state, pol)
    bound = fps.get(check, fp)  # a check no route requires right now is bound to the whole task
    state["evidence"][check] = {"fp": bound, "exit": 0, "t": time.time(), "note": args.note, "evidence": args.evidence}
    state["failed"].pop(check, None)
    save_state(state)
    print(f"recorded {check} for fingerprint {bound}")
    return 0


def write_packet(state, pol):
    """Compact evidence for the review classifier: paths, routes, required checks and a truncated diff."""
    files, fp, why, routes, missing, fps = assess(state, pol)
    diff = diff_text(state, files)
    changed = sum(1 for ln in diff.splitlines() if ln[:1] in "+-")
    text = "\n".join([
        f"fingerprint: {fp}",
        f"changed_lines: {changed}",
        f"files ({len(files)}): " + ", ".join(files[:40]),
        f"routes: {', '.join(routes) or '-'}",
        f"required_checks: {', '.join(why) or '-'}",
        "--- diff (added and removed lines, truncated) ---",
        diff[:6000],
    ])
    path = STATE_DIR / "packets" / f"{state['session']}-{fp}.txt"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path, changed, fp


def cmd_packet(args):
    pol = policy()
    path, changed, fp = write_packet(ensure_task(), pol)
    print(f"{path}  changed_lines={changed} fingerprint={fp}")
    return 0


def cmd_classify(args):
    pol = policy()
    cfg = pol["review_classifier"]
    state = ensure_task()
    files, fp, *_ = assess(state, pol)
    if not files:
        print("no task changes; nothing to classify")
        return 0
    try:
        out = json.loads(Path(args.result).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        print(f"unreadable classifier result: {exc}")
        return 2
    label = out.get("review") if isinstance(out, dict) else None
    if label not in cfg["labels"]:
        print(f"rejected label {label!r}; allowed: {', '.join(cfg['labels'])}")
        return 2
    # The label is only about the packet it was made from. A tree that moved since then gets no label.
    try:
        packet_fp = Path(args.packet).read_text(encoding="utf-8").splitlines()[0].split(":", 1)[1].strip()
    except (OSError, IndexError, ValueError) as exc:
        print(f"unreadable packet {args.packet}: {exc}")
        return 2
    if packet_fp != fp:
        print(f"packet is for fingerprint {packet_fp}, current diff is {fp}: packet stale, run `packet` again")
        return 2
    reason = str(out.get("reason", ""))[:160].replace("\n", " ")
    checks = list(cfg["labels"][label])
    baseline, _, _ = required_checks(state, files, pol)
    added = [c for c in checks if c not in baseline]
    state["classified"] = {"fp": fp, "label": label, "checks": checks, "reason": reason, "t": time.time()}
    save_state(state)
    # Log for later evaluation against real review outcomes; the classifier's output is never trusted beyond this.
    with (STATE_DIR / "review_classifier.log").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps({"t": time.time(), "fp": fp, "label": label, "added": added, "reason": reason}) + "\n")
    print(f"classified {label}: adds {added or 'nothing'} for fingerprint {fp}")
    return 0


def cmd_claim(args):
    """Adds paths to the task. Never removes anything. For edits made before the hooks existed,
    or by an external process the session started."""
    state = ensure_task()
    paths = [repo_rel(p) for p in args.paths]
    if None in paths:
        print("claim paths must lie inside the checkout")
        return 2
    paths = sorted(set(paths))
    state.setdefault("claims", []).append({"paths": paths, "note": args.note, "t": time.time()})
    if "owned" in state:
        state["owned"] = sorted(set(state["owned"]) | set(paths))
        for path in paths:
            state.get("disowned", {}).pop(path, None)
        record_ownership(state, {}, {}, paths, "claim")  # the log is what hooks read, so claims survive their saves
    save_state(state)
    print(f"claimed {len(paths)} path(s){'' if 'owned' in state else ' (legacy session: run adopt-ownership to apply)'}: {args.note}")
    return 0


def cmd_adopt(args):
    """One-time switch of a legacy session to ownership: owned becomes exactly the claimed paths.
    Files the legacy rule counted that were never claimed are logged as dropped from the task."""
    state = ensure_task()
    if "owned" in state:
        print("already ownership-based; nothing to adopt")
        return 0
    claimed = sorted({p for c in state.get("claims", []) for p in c["paths"]})
    if not claimed:
        print("no claims recorded; claim the files this session wrote first")
        return 2
    legacy = changed_files(state)
    dropped = sorted(set(legacy) - set(claimed))
    state["owned"] = claimed
    owned_path(state["session"]).unlink(missing_ok=True)
    state["adoption"] = {"t": time.time(), "kept": sorted(set(legacy) & set(claimed)), "dropped": dropped}
    save_state(state)
    print(f"adopted {len(claimed)} owned path(s); {len(dropped)} dropped from the task:")
    for path in dropped:
        print(f"  dropped {path}")
    return 0


def disowned_report(state):
    """Every path this session disowned, flagging ones whose content changed since (a later edit may be ours)."""
    disowned = state.get("disowned") if isinstance(state.get("disowned"), dict) else {}
    owned = owned_set(state) if disowned else set()
    lines = []
    for path, entry in sorted(disowned.items()):
        if path in owned:
            continue  # re-owned by a later write or claim
        note = entry.get("note", "") if isinstance(entry, dict) else ""
        moved = isinstance(entry, dict) and entry.get("sha") not in (None, file_sha(path))
        lines.append(f"disowned       {path}{' (CHANGED since disown)' if moved else ''}: {note}")
    return lines


def cmd_disown(args):
    """Removes paths a hook attributed to this session although a peer wrote them (a peer write landing inside one
    of your tool calls). Audited: logged, kept in the state, and announced at Stop. A later write or claim by this
    session owns the path again."""
    state = ensure_task()
    if "owned" not in state:
        print("legacy session: ownership is not tracked, so there is nothing to disown")
        return 2
    paths = [repo_rel(p) for p in args.paths]
    if None in paths:
        print("paths must lie inside the checkout")
        return 2
    owned = owned_set(state)
    unknown = [p for p in paths if p not in owned]
    if unknown:
        print(f"not owned by this session: {', '.join(unknown)}")
        return 2
    entries = owned_entries(state)
    # Only `window` attributions can be a peer's write; `legacy` lines predate sources and might be named writes.
    named = {p for p, src in entries if src != "window"} | set(state.get("owned", []))
    refused = [p for p in paths if p in named]
    if refused:
        # Only a shell call's diff window can catch a peer's write. A file this session's tool named, or that
        # it claimed, is its own, and dropping it would drop its required checks.
        print(f"written or claimed by this session, cannot be disowned: {', '.join(refused)}")
        return 2
    now = time.time()
    disowned = state.setdefault("disowned", {})
    for path in paths:
        disowned[path] = {"seq": len(entries), "t": now, "sha": file_sha(path), "note": args.note, "announced": False}
    save_state(state)
    log_line = json.dumps({"t": now, "session": state["session"], "event": f"disowned {', '.join(paths)}: {args.note}"})
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    with (STATE_DIR / "ownership.log").open("a", encoding="utf-8") as fh:
        fh.write(log_line + "\n")
    print(f"disowned {len(paths)} path(s); announced at the next Stop: {args.note}")
    return 0


def cmd_owners(args):
    """Which sessions own a path, for coordinating with whoever else is editing it. Legacy sessions record no
    ownership, so a path changed only by them shows no owner."""
    rels = [repo_rel(p) for p in args.paths]
    if None in rels:
        print("paths must lie inside the checkout")
        return 2
    for rel in rels:
        owners = []
        for path in sorted(SESSION_DIR.glob("*.json")):
            if path.name.endswith(".shas.json"):
                continue
            try:
                state = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                continue
            if "owned" in state and rel in owned_set(state):
                owners.append((path.stat().st_mtime, state["session"]))
        listed = ", ".join(f"{key} (active {time.strftime('%m-%d %H:%M', time.localtime(t))})" for t, key in sorted(owners, reverse=True))
        print(f"{rel}: {listed or 'no ownership session (a legacy session or nobody)'}")
    return 0


def cmd_reset(args):
    """Clears evidence and budgets only. Requirements are never re-baselined from here."""
    state = ensure_task()
    state.update(evidence={}, failed={}, failures={}, continuations=0, failed_runs=0)
    save_state(state)
    print("evidence and repair budget cleared; task base unchanged")
    return 0


def main():
    import argparse
    sys.stdout.reconfigure(errors="replace")
    sys.stdin.reconfigure(encoding="utf-8")  # cargo output may hold non-cp1252 characters
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("status", "reset", "adopt-ownership"):
        sub.add_parser(name)
    ver = sub.add_parser("verify")
    ver.add_argument("checks", nargs="*", help="run only these required automatic checks")
    att = sub.add_parser("attest")
    att.add_argument("check")
    att.add_argument("--note", required=True)
    att.add_argument("--evidence")
    clm = sub.add_parser("claim")
    clm.add_argument("paths", nargs="+")
    clm.add_argument("--note", required=True)
    own = sub.add_parser("owners")
    own.add_argument("paths", nargs="+")
    dis = sub.add_parser("disown")
    dis.add_argument("paths", nargs="+")
    dis.add_argument("--note", required=True)
    sub.add_parser("packet")
    cls = sub.add_parser("classify")
    cls.add_argument("result", help="JSON file written from the change-classifier subagent output")
    cls.add_argument("--packet", required=True, help="the packet file the label was made from")
    hook = sub.add_parser("hook")
    # "dirty" is a retired PostToolUse event: still accepted (no-op) so a session with a stale hook config is never blocked.
    hook.add_argument("event", choices=["prompt", "stop", "pre", "post", "dirty"])
    args = parser.parse_args()
    if args.cmd == "hook":
        import workflow_hooks
        return workflow_hooks.run(args.event)
    commands = {"status": cmd_status, "verify": cmd_verify, "attest": cmd_attest, "reset": cmd_reset,
                "packet": cmd_packet, "classify": cmd_classify, "claim": cmd_claim,
                "adopt-ownership": cmd_adopt, "owners": cmd_owners, "disown": cmd_disown}
    return commands[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
