#!/usr/bin/env python3
"""Find tracked files that need untracked files to build or run.

    python -I scripts/tracked_deps.py check        # exit 1 on a build dependency
    python -I scripts/tracked_deps.py inventory    # markdown commit sets on stdout
    python -I scripts/tracked_deps.py --rev HEAD   # the commit alone, as a clone sees it

A clean clone holds only tracked files, so every edge below that ends on an
untracked or missing file is a build the clone cannot reproduce:
`mod` declarations (with `#[path]`), `include_bytes!`/`include_str!`, Cargo
`path =` dependencies, and crates the `crates/*` workspace glob picks up.
String literals naming `assets/...` are runtime loads and only warn.
"""
import re
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path, PurePosixPath

MOD_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(r#)?([A-Za-z_][A-Za-z0-9_]*)\s*;")
PATH_ATTR_RE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]')
CFG_RE = re.compile(r"#\[cfg\((.*)\)\]")
INCLUDE_RE = re.compile(r'include_(?:bytes|str)!\(\s*"([^"]+)"\s*\)')
ASSET_RE = re.compile(r'"((?:\.\./)*assets/[^"{}*\s]+)"')
CARGO_PATH_RE = re.compile(r'path\s*=\s*"([^"]+)"')
CRATE_PATH_RE = re.compile(r"\bcrate((?:::[a-z_][a-z0-9_]*)+)")

BUILD_KINDS = ("mod", "include", "cargo", "workspace")
SET_LISTING = 25


def git_files(root, *args):
    out = subprocess.run(["git", "-C", str(root), "ls-files", "-z", *args],
                         check=True, capture_output=True).stdout
    return {p.decode("utf-8") for p in out.split(b"\0") if p}


def norm(path):
    parts = []
    for part in PurePosixPath(path).parts:
        if part == "..":
            if parts:
                parts.pop()
        elif part != ".":
            parts.append(part)
    return "/".join(parts)


def mod_candidates(src, name, path_attr):
    here = PurePosixPath(src).parent
    if path_attr:
        return [norm(here / path_attr)]
    stem = PurePosixPath(src).stem
    base = here if stem in ("main", "lib", "mod") or src.endswith("build.rs") else here / stem
    return [norm(base / f"{name}.rs"), norm(base / name / "mod.rs")]


def attrs_above(lines, index):
    """Attribute lines directly above `index`, nearest last."""
    attrs = []
    i = index - 1
    while i >= 0 and lines[i].strip().startswith(("#[", "//")):
        attrs.append(lines[i].strip())
        i -= 1
    return list(reversed(attrs))


def scan_rust(src, text):
    lines = text.splitlines()
    for n, line in enumerate(lines, 1):
        if line.lstrip().startswith("//"):
            continue
        m = MOD_RE.match(line)
        if m:
            attrs = attrs_above(lines, n - 1)
            path_attr = next((PATH_ATTR_RE.search(a).group(1) for a in attrs if PATH_ATTR_RE.search(a)), None)
            cfg = next((CFG_RE.search(a).group(1) for a in attrs if CFG_RE.search(a)), "")
            yield n, "mod", mod_candidates(src, m.group(2), path_attr), cfg
        for inc in INCLUDE_RE.findall(line):
            yield n, "include", [norm(PurePosixPath(src).parent / inc)], ""
        for lit in ASSET_RE.findall(line):
            yield n, "runtime", [lit[lit.index("assets/"):].rstrip("/")], ""
        root = crate_src_root(src)
        if root:
            for path in set(CRATE_PATH_RE.findall(line)):
                segments = path.strip(":").split("::")
                # Longest module prefix wins; shorter ones are its parents and already linked by `mod`.
                candidates = []
                for k in range(len(segments), 0, -1):
                    stem = "/".join([root, *segments[:k]])
                    candidates += [stem + ".rs", stem + "/mod.rs"]
                yield n, "use", candidates, ""


def crate_src_root(src):
    parts = src.split("/")
    if len(parts) > 3 and parts[0] == "crates" and parts[2] == "src":
        return "/".join(parts[:3])
    return None


def scan_cargo(src, text):
    for n, line in enumerate(text.splitlines(), 1):
        for dep in CARGO_PATH_RE.findall(line):
            yield n, "cargo", [norm(PurePosixPath(src).parent / dep)], ""


def tree_files(root, rev):
    out = subprocess.run(["git", "-C", str(root), "ls-tree", "-r", "-z", "--name-only", rev],
                         check=True, capture_output=True).stdout
    return {p.decode("utf-8") for p in out.split(b"\0") if p}


def tree_texts(root, rev, paths):
    """Blob text for `paths` at `rev` through one `git cat-file --batch`."""
    request = "".join(f"{rev}:{p}\n" for p in paths).encode("utf-8")
    out = subprocess.run(["git", "-C", str(root), "cat-file", "--batch"], input=request,
                         check=True, capture_output=True).stdout
    texts, pos = {}, 0
    for path in paths:
        end = out.index(b"\n", pos)
        header = out[pos:end].split()
        pos = end + 1
        if len(header) < 3 or header[1] != b"blob":
            continue
        size = int(header[2])
        texts[path] = out[pos:pos + size].decode("utf-8", errors="replace")
        pos += size + 1
    return texts


class Repo:
    def __init__(self, root, rev=None):
        self.root = Path(root)
        self.rev = rev
        if rev:
            # A commit holds exactly its tree: nothing is untracked, everything else is missing.
            self.tracked = tree_files(root, rev)
            self.untracked = set()
            wanted = sorted(p for p in self.tracked if p.endswith((".rs", "Cargo.toml")))
            self.texts = tree_texts(root, rev, wanted)
        else:
            self.tracked = git_files(root)
            self.untracked = git_files(root, "--others", "--exclude-standard")
            self.texts = None
        self.tracked_dirs = set()
        for p in self.tracked:
            parent = PurePosixPath(p).parent
            while str(parent) != ".":
                self.tracked_dirs.add(str(parent))
                parent = parent.parent

    def state(self, path):
        if path in self.tracked or path in self.tracked_dirs:
            return "tracked"
        if not self.rev and (self.root / path).exists():
            return "untracked"
        return "missing"

    def untracked_under(self, path):
        if path in self.untracked:
            return [path]
        prefix = path + "/"
        return sorted(p for p in self.untracked if p.startswith(prefix))

    def edges(self):
        """(referrer, line, kind, target, state, cfg) for every non-tracked target."""
        found = []
        for src in sorted(self.tracked | self.untracked):
            if src.startswith("target/"):
                continue
            if src.endswith(".rs"):
                scanner = scan_rust
            elif src.endswith("Cargo.toml"):
                scanner = scan_cargo
            else:
                continue
            if self.texts is not None:
                text = self.texts.get(src)
                if text is None:
                    continue
            else:
                try:
                    text = (self.root / src).read_text(encoding="utf-8", errors="replace")
                except OSError:
                    continue
            for line, kind, candidates, cfg in scanner(src, text):
                states = [(c, self.state(c)) for c in candidates]
                if kind == "use":
                    hit = next(((c, s) for c, s in states if s != "missing"), None)
                    if hit is None or hit[1] == "tracked" or hit[0] == src:
                        continue
                    target, state = hit
                elif any(s == "tracked" for _, s in states):
                    continue
                else:
                    target, state = next(((c, s) for c, s in states if s == "untracked"), states[0])
                if kind == "runtime" and state == "missing":
                    continue  # a literal prefix or a path built at runtime
                found.append((src, line, kind, target, state, cfg))
        if "Cargo.toml" in self.tracked:
            for manifest in sorted(self.untracked):
                parts = manifest.split("/")
                if len(parts) == 3 and parts[0] == "crates" and parts[2] == "Cargo.toml":
                    found.append(("Cargo.toml", 0, "workspace", f"crates/{parts[1]}", "untracked", ""))
        return found


def build_blockers(repo, edges):
    return [e for e in edges if e[0] in repo.tracked and e[2] in BUILD_KINDS]


def commit_sets(repo, edges):
    """Group untracked files that must be committed together with their tracked referrers."""
    parent = {}

    def find(x):
        parent.setdefault(x, x)
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    def union(a, b):
        parent[find(a)] = find(b)

    referrers = defaultdict(set)
    for src, _, kind, target, state, _ in edges:
        if state != "untracked" or kind == "runtime":
            continue
        files = repo.untracked_under(target)
        if not files:
            continue
        for f in files:
            find(f)
            union(f, files[0])
        if src in repo.untracked:
            union(src, files[0])
        else:
            referrers[files[0]].add(src)
    groups = defaultdict(lambda: (set(), set()))
    for f in list(parent):
        files, refs = groups[find(f)]
        files.add(f)
    for head, refs in referrers.items():
        groups[find(head)][1].update(refs)
    return sorted(groups.values(), key=lambda g: (-len(g[1]), sorted(g[0])[0]))


def cmd_check(repo):
    edges = repo.edges()
    blockers = build_blockers(repo, edges)
    for src, line, kind, target, state, cfg in blockers:
        gate = f" [cfg({cfg})]" if cfg else ""
        print(f"{src}:{line}: {kind} -> {target} ({state}){gate}")
    runtime = [e for e in edges if e[0] in repo.tracked and e[2] == "runtime"]
    print(f"{len(blockers)} build dependencies on untracked or missing files; "
          f"{len(runtime)} runtime asset references (warning)")
    return 1 if blockers else 0


def cmd_inventory(repo):
    edges = repo.edges()
    sets = commit_sets(repo, edges)
    reached = set().union(*(files for files, _ in sets)) if sets else set()
    runtime = {e[3] for e in edges if e[2] == "runtime" and e[4] == "untracked"}
    print(f"# Untracked inventory\n\nGenerated {time.strftime('%Y-%m-%d %H:%M')} by "
          "`python -I scripts/tracked_deps.py inventory`; regenerate rather than edit.\n")
    print(f"{len(repo.untracked)} untracked files; {len(build_blockers(repo, edges))} build "
          f"edges from tracked files; {len(sets)} commit sets.\n")
    print("## Commit sets\n\nEach set must land in one commit with its tracked referrers, "
          "or the referrers' commit does not build.\n")
    for i, (files, refs) in enumerate(sets, 1):
        shown = sorted(files)
        print(f"### Set {i}: {len(files)} files, {len(refs)} tracked referrers\n")
        print("Referrers: " + (", ".join(f"`{r}`" for r in sorted(refs)) or "none (reached only from untracked files)") + "\n")
        for f in shown[:SET_LISTING]:
            print(f"- `{f}`")
        if len(shown) > SET_LISTING:
            print(f"- ... {len(shown) - SET_LISTING} more")
        print()
    print("## Runtime-only\n\nUntracked assets named by a string literal; the build passes, the load fails.\n")
    for path in sorted(runtime - reached):
        print(f"- `{path}`")
    print("\n## No referrer\n\nNot reached from any code edge: generated, stale, documents, or work not yet wired.\n")
    by_area = defaultdict(int)
    for path in sorted(repo.untracked - reached):
        if any(path == r or path.startswith(r + "/") for r in runtime):
            continue
        by_area["/".join(path.split("/")[:2])] += 1
    for area, count in sorted(by_area.items(), key=lambda kv: -kv[1]):
        print(f"- `{area}`: {count}")
    return 0


def main(argv):
    args = argv[1:]
    rev = None
    if "--rev" in args:
        i = args.index("--rev")
        rev = args[i + 1] if i + 1 < len(args) else "HEAD"
        del args[i:i + 2]
    action = args[0] if args else "check"
    root = Path(args[1]) if len(args) > 1 else Path(__file__).resolve().parent.parent
    repo = Repo(root, rev)
    if action == "check":
        return cmd_check(repo)
    if action == "inventory":
        return cmd_inventory(repo)
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
