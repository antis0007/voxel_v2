// Self-correcting / self-improving layer for the parity swarm. Three small, cheap mechanisms:
//   1. diskGuard   - pauses the swarm before a full disk can truncate source files; resumes itself.
//   2. lessons     - a Haiku (tool-less, ~1k tokens) call distils each blocked/failed/escalated result
//                    into one reusable rule; the newest few are prepended to later prompts.
//   3. checkpoint  - when the tree is quiet, run one cargo check; compiler errors go to a repair agent.
// No dependency on swarm internals: swarm.mjs passes in what is needed.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, statfsSync, writeFileSync } from "node:fs";
import path from "node:path";

// Provider/infra noise is not a codebase lesson; it would crowd out real rules.
const NOISE = /limit|resets? \d|rate.?limit|credit|quota|overloaded|ECONN|ETIMEDOUT|stalled|no result|CLI not found|connection problem|disk/i;
const words = text => new Set(String(text).toLowerCase().match(/[a-z_]{4,}/g) ?? []);
const similar = (a, b) => { const x = words(a), y = words(b); let hit = 0; for (const w of x) if (y.has(w)) hit++; return hit / Math.max(1, Math.min(x.size, y.size)); };

export function createBrain({ dir, root, childEnv, event, claude = process.env.AXIOM_CLAUDE_PATH || "claude", minFreeGb = 2.5 }) {
  const lessonsFile = path.join(dir, "lessons.json");
  let lessons = [];
  try { if (existsSync(lessonsFile)) lessons = JSON.parse(readFileSync(lessonsFile, "utf8")).filter(l => !NOISE.test(l.text)); } catch { lessons = []; }
  const save = () => { try { writeFileSync(lessonsFile, JSON.stringify(lessons)); } catch { /* best effort */ } };

  function freeGb() {
    try { const s = statfsSync(root); return Number(s.bavail) * Number(s.bsize) / 2 ** 30; } catch { return Infinity; }
  }
  // Hysteresis: pause below minFreeGb, resume above minFreeGb + 1.
  let low = false;
  function diskGuard() {
    const gb = freeGb();
    if (!low && gb < minFreeGb) { low = true; event?.(`low disk: ${gb.toFixed(1)} GB free; agents paused until ${(minFreeGb + 1).toFixed(1)} GB`); }
    else if (low && gb >= minFreeGb + 1) { low = false; event?.(`disk recovered: ${gb.toFixed(1)} GB free`); }
    return { low, gb };
  }

  function oneShot(model, system, prompt, timeoutMs = 60_000) {
    return new Promise(resolve => {
      const child = spawn(claude, ["-p", "--model", model, "--effort", "low", "--tools", "", "--system-prompt", system, "--setting-sources", "local",
        "--disable-slash-commands", "--no-session-persistence", "--max-turns", "1", "--max-budget-usd", "0.05"],
      { cwd: root, windowsHide: true, stdio: ["pipe", "pipe", "ignore"], env: { ...childEnv(), CLAUDE_CODE_DISABLE_AUTO_MEMORY: "1", DISABLE_TELEMETRY: "1" } });
      let out = ""; const timer = setTimeout(() => child.kill(), timeoutMs);
      child.stdout.setEncoding("utf8").on("data", c => { out += c; });
      child.on("error", () => { clearTimeout(timer); resolve(""); });
      child.on("close", () => { clearTimeout(timer); resolve(out.trim()); });
      child.stdin.on("error", () => {});
      child.stdin.end(prompt);
    });
  }

  // Learn only from results that cost something: blocked, failed or escalated.
  async function learn(entry) {
    if (!/blocked|failed|escalated/.test(entry.status) || NOISE.test(String(entry.summary))) return;
    const text = await oneShot("haiku",
      "Distil ONE reusable rule (max 22 words, imperative, specific to this Rust voxel codebase) from an agent's failure report so the next agent avoids it or finishes faster. Reply NONE if the report holds no general lesson.",
      `Row ${entry.id}: ${entry.label}\nStatus: ${entry.status}\nReport: ${String(entry.summary).slice(0, 500)}`);
    const line = text.split("\n")[0].replace(/^[-*\d.\s]+/, "").trim();
    if (!line || /^none\b/i.test(line) || line.length > 220) return;
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
