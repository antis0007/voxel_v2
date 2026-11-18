// Self-correcting / self-improving layer for the parity swarm. Three small, cheap mechanisms:
//   1. diskGuard   - pauses the swarm before a full disk can truncate source files; resumes itself.
//   2. lessons     - a Haiku (tool-less, ~1k tokens) call distils each blocked/failed/escalated result
//                    into one reusable rule; the newest few are prepended to later prompts.
//   3. checkpoint  - when the tree is quiet, run one cargo check; compiler errors go to a repair agent.
// No dependency on swarm internals: swarm.mjs passes in what is needed.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, readdirSync, rmSync, statSync, statfsSync, writeFileSync } from "node:fs";
import path from "node:path";
import { settleOnce } from "./swarm-check.mjs";

// Provider/infra noise is not a codebase lesson; it would crowd out real rules.
const NOISE = /limit|resets? \d|rate.?limit|credit|quota|overloaded|ECONN|ETIMEDOUT|stalled|no result|CLI not found|connection problem|disk/i;
// Lessons that tell agents to stop, escalate or "grep first" contradict OWNER POLICY (build the missing owner)
// or repeat the tool rules already in every prompt; 12 of 12 live lessons were this shape. Row-specific
// keyword hints ("grep for altar") help no other row either.
const UNHELPFUL = /\bescalat|report (partial|blocked|the blocker)|\bblocked\b|cap (blind )?exploration|within (five|5|a few) (turns|searches)|before (reading|broad|exploring)|\b(grep|rg|search)\b[^.]{0,60}\bfirst\b|\bearly\b/i;
const words = text => new Set(String(text).toLowerCase().match(/[a-z_]{4,}/g) ?? []);
const similar = (a, b) => { const x = words(a), y = words(b); let hit = 0; for (const w of x) if (y.has(w)) hit++; return hit / Math.max(1, Math.min(x.size, y.size)); };

export function createBrain({ dir, root, childEnv, event, claude = process.env.AXIOM_CLAUDE_PATH || "claude", minFreeGb = 2.5 }) {
  const lessonsFile = path.join(dir, "lessons.json");
  let lessons = [];
  try { if (existsSync(lessonsFile)) lessons = JSON.parse(readFileSync(lessonsFile, "utf8")).filter(l => !NOISE.test(l.text) && !UNHELPFUL.test(l.text)); } catch { lessons = []; }
  const save = () => { try { writeFileSync(lessonsFile, JSON.stringify(lessons)); } catch { /* best effort */ } };

  function freeGb() {
    try { const s = statfsSync(root); return Number(s.bavail) * Number(s.bsize) / 2 ** 30; } catch { return Infinity; }
  }
  // Self-heal before pausing: stale cargo incremental session dirs (safe to delete; cargo rebuilds them) and
  // agent logs older than three days. Sessions touched in the last 20 minutes are skipped so a running
  // cargo (the swarm's or a peer's) keeps its own. Without this the swarm sat paused until someone cleaned up.
  function dirBytes(p) {
    let n = 0;
    try { for (const e of readdirSync(p, { withFileTypes: true })) { const f = path.join(p, e.name); n += e.isDirectory() ? dirBytes(f) : (statSync(f).size || 0); } } catch { /* vanished */ }
    return n;
  }
  function reclaim() {
    const cutoff = Date.now() - 20 * 60_000, logCutoff = Date.now() - 3 * 86_400_000;
    let bytes = 0;
    const target = path.join(root, "target");
    let profiles = [];
    // target/<profile>/incremental plus the swarm's proof target (target/proof/<profile>/incremental).
    for (const base of [target, path.join(target, "proof")]) {
      try { profiles.push(...readdirSync(base, { withFileTypes: true }).filter(e => e.isDirectory()).map(e => path.join(base, e.name, "incremental"))); } catch { /* none */ }
    }
    for (const inc of profiles) {
      let crates = [];
      try { crates = readdirSync(inc).map(c => path.join(inc, c)); } catch { continue; }
      for (const crate of crates) {
        let sessions = [];
        try { sessions = readdirSync(crate).filter(s => s.startsWith("s-") && !s.endsWith(".lock")).map(s => path.join(crate, s)); } catch { continue; }
        for (const s of sessions) {
          try { if (statSync(s).mtimeMs > cutoff) continue; const b = dirBytes(s); rmSync(s, { recursive: true, force: true }); rmSync(s + ".lock", { force: true }); bytes += b; } catch { /* in use */ }
        }
      }
    }
    try {
      for (const f of readdirSync(path.join(dir, "logs"))) {
        const p = path.join(dir, "logs", f);
        try { const st = statSync(p); if (st.mtimeMs < logCutoff) { rmSync(p, { force: true }); bytes += st.size; } } catch { /* gone */ }
      }
    } catch { /* no logs */ }
    return bytes / 2 ** 30;
  }
  // Hysteresis: pause below minFreeGb, resume above minFreeGb + 1. Reclaim at most every 5 minutes.
  let low = false, reclaimedAt = 0;
  function diskGuard() {
    let gb = freeGb();
    if (gb < minFreeGb + 1 && Date.now() - reclaimedAt > 5 * 60_000) {
      reclaimedAt = Date.now();
      const freed = reclaim();
      if (freed > 0.05) { event?.(`disk: reclaimed ${freed.toFixed(1)} GB of stale cargo incremental caches and old logs`); gb = freeGb(); }
    }
    if (!low && gb < minFreeGb) { low = true; event?.(`low disk: ${gb.toFixed(1)} GB free; agents paused until ${(minFreeGb + 1).toFixed(1)} GB`); }
    else if (low && gb >= minFreeGb + 1) { low = false; event?.(`disk recovered: ${gb.toFixed(1)} GB free`); }
    return { low, gb };
  }

  function oneShot(model, system, prompt, timeoutMs = 60_000) {
    return new Promise(resolve => {
      const child = spawn(claude, ["-p", "--model", model, "--effort", "low", "--tools", "", "--system-prompt", system, "--setting-sources", "local",
        "--disable-slash-commands", "--no-session-persistence", "--strict-mcp-config", "--max-turns", "1", "--max-budget-usd", "0.05"],
      { cwd: root, windowsHide: true, stdio: ["pipe", "pipe", "ignore"], env: { ...childEnv(), FORCE_PROMPT_CACHING_5M: "1", ENABLE_CLAUDEAI_MCP_SERVERS: "false", CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1", DISABLE_TELEMETRY: "1" } });
      let out = "";
      child.stdout.setEncoding("utf8").on("data", c => { out += c; });
      settleOnce(child, () => resolve(out.trim()), timeoutMs);
      child.stdin.on("error", () => {});
      child.stdin.end(prompt);
    });
  }

  // Learn only from results that cost something: blocked, failed or escalated.
  async function learn(entry) {
    if (!/blocked|failed|escalated/.test(entry.status) || NOISE.test(String(entry.summary)) || /explored \d+ turns without an edit|^scout:/.test(String(entry.summary))) return;
    const text = await oneShot("haiku",
      "Distil ONE reusable rule (max 22 words, imperative) from an agent's failure report that would help agents working on OTHER rows of this Rust voxel codebase: a concrete fact such as where a tick, table or save field lives, or a compile trap. Agents already search with Grep and must build missing owners rather than stop, so never advise searching first, stopping, escalating or reporting blocked. Reply NONE if no such fact exists.",
      `Row ${entry.id}: ${entry.label}\nStatus: ${entry.status}\nReport: ${String(entry.summary).slice(0, 500)}`);
    const line = text.split("\n")[0].replace(/^[-*\d.\s]+/, "").trim();
    if (!line || /^none\b/i.test(line) || line.length > 220 || UNHELPFUL.test(line)) return;
    const dup = lessons.find(l => similar(l.text, line) > 0.7);
    if (dup) { dup.n++; dup.at = Date.now(); } else lessons.push({ text: line, n: 1, at: Date.now() });
    lessons.sort((a, b) => b.n - a.n || b.at - a.at);
    lessons = lessons.slice(0, 12);
    save(); event?.("lesson: " + line);
  }
  const note = () => lessons.length ? "Lessons from earlier rows: " + lessons.slice(0, 6).map(l => l.text).join(" | ") : "";

  // One cargo check; returns { ok, errors: ["path:line: message", ...] }.
  function compile(timeoutMs = 20 * 60_000) {
    return new Promise(resolve => {
      const child = spawn("cargo", ["check", "-p", "minecraft_style_voxel_prototype", "--message-format", "short"], { cwd: root, windowsHide: true, env: childEnv(), stdio: ["ignore", "pipe", "pipe"] });
      let out = ""; const timer = setTimeout(() => child.kill(), timeoutMs);
      const take = c => { out = (out + c).slice(-200_000); };
      child.stdout.setEncoding("utf8").on("data", take); child.stderr.setEncoding("utf8").on("data", take);
      child.on("error", e => { clearTimeout(timer); resolve({ ok: false, errors: [], note: e.message }); });
      child.on("close", code => {
        clearTimeout(timer);
        const errors = [...new Set(out.split(/\r?\n/).filter(l => /: error(\[E\d+\])?: /.test(l)).map(l => l.trim()))].slice(0, 40);
        resolve({ ok: code === 0, errors });
      });
    });
  }

  return { diskGuard, freeGb, learn, note, compile, lessons: () => lessons };
}
