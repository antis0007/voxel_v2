// Uses the panel's revision checks and backups for the native study catalog.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { read, save } from "./workspace.mjs";

const path = "assets/field_studies.json";
const integer = (value, max) => Number.isSafeInteger(value) && value >= 1 && value <= max;
async function run([command = "help", lesson, ...extra]) {
  if (command === "help") return {
    commands: ["inspect [Lesson]", "preview Lesson < JSON", "apply Lesson < JSON"],
    request: { xp: 40, counts: [8, 8, 8] },
    publication: "Apply repeats the preview request with its version and approval. Rebuild the game afterward.",
    scope: "XP and existing supply quantities only; lesson identities and prerequisites remain fixed.",
  };
  if (extra.length || !["inspect", "preview", "apply"].includes(command)) throw Error("Run help for usage");
  const source = read(path);
  const rows = JSON.parse(source.text);
  const row = rows.find(row => row.lesson === lesson);
  if (command === "inspect") {
    if (lesson && !row) throw Error("Unknown lesson");
    return { path, version: source.version, projects: lesson ? [row] : rows };
  }
  if (!row) throw Error("Choose a lesson from inspect");
  const request = JSON.parse(readFileSync(0, "utf8").replace(/^\uFEFF/, ""));
  if (Object.keys(request).some(key => !["xp", "counts", "version", "approval"].includes(key))) throw Error("Unknown request field");
  if (!integer(request.xp, 10000) || !Array.isArray(request.counts)
      || request.counts.length !== row.samples.length || !request.counts.every(n => integer(n, 4096))) {
    throw Error("XP must be 1–10000; provide one supply count (1–4096) per existing ingredient");
  }
  if (request.version !== undefined && request.version !== source.version) throw Error("Catalog changed; preview again");
  const before = structuredClone(row);
  row.xp = request.xp;
  row.samples.forEach((sample, i) => { sample[1] = request.counts[i]; });
  const text = JSON.stringify(rows, null, 2) + "\n";
  const approval = createHash("sha256").update(JSON.stringify([path, source.version, text])).digest("hex");
  if (command === "apply") {
    if (request.version !== source.version || request.approval !== approval) throw Error("Repeat the preview with its version and approval");
    const saved = save(path, { version: source.version, text });
    return { saved: path, version: saved.version, before, after: row, runtime: "Rebuild required" };
  }
  return { path, before, after: row, request: { xp: request.xp, counts: request.counts, version: source.version, approval } };
}
try { process.stdout.write(JSON.stringify(await run(process.argv.slice(2)), null, 2) + "\n"); }
catch (error) { process.stderr.write(JSON.stringify({ error: error.message }) + "\n"); process.exitCode = 1; }
