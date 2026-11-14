// Headless planet lab: render and measure generated Planet terrain without the game.
// Wraps `cargo run -p engine_worldgen --example planet_lab`; prints the lab's JSON lines.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const exe = join(root, "target/release-fast/examples", process.platform === "win32" ? "planet_lab.exe" : "planet_lab");
const out = "target/planet-lab";
const buildArgs = ["build", "--profile", "release-fast", "-p", "engine_worldgen", "--example", "planet_lab"];

const usage = {
  "build": "compile the lab (release-fast); needed after any worldgen source change",
  "patch|view|globe|stats|where|find|probe ...": "one lab command; options as in crates/worldgen/examples/planet_lab/main.rs",
  "batch <file>": "one lab command per line; the planet is solved once for all of them",
  "review <genA> [genB] [--at x,z|spawn|confluence] [--name label]": "standard evidence set: stats, shaded and detail patches, three eye-level views, globe; A/B when two generators",
  "flags": "--fast before the command sets VOXEL_HISTORY_MYR=20 (short plate history, quick solve, different planet)",
  "docs": "tools/axiom-dev/PLANET_LAB.md",
};

function newestSource(dir) {
  let newest = 0;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    newest = Math.max(newest, entry.isDirectory() ? newestSource(path) : statSync(path).mtimeMs);
  }
  return newest;
}

function lab(args, fast) {
  if (!existsSync(exe)) throw new Error("planet_lab is not built; run: node tools/axiom-dev/planet-lab-cli.mjs build");
  const stale = newestSource(join(root, "crates/worldgen/src")) > statSync(exe).mtimeMs;
  const env = fast ? { ...process.env, VOXEL_HISTORY_MYR: "20" } : process.env;
  const run = spawnSync(exe, args, { cwd: root, env, encoding: "utf8", maxBuffer: 64 << 20 });
  // The solve logs to both streams; keep only the lab's own lines.
  const lines = (run.stdout ?? "").split(/\r?\n/).filter(line => line.startsWith("{"));
  const solve = (run.stderr ?? "").split(/\r?\n/).filter(line => line.startsWith("[planet-lab]"));
  for (const line of lines) console.log(line);
  if (run.status !== 0) {
    console.error((run.stderr ?? "").split(/\r?\n/).filter(line => /error|Error|panicked/.test(line)).slice(-6).join("\n"));
    process.exitCode = run.status ?? 1;
  }
  console.log(JSON.stringify({ solves: solve.length, stale: stale ? "exe older than crates/worldgen/src; run build" : undefined, fast: fast || undefined }));
}

function review(rest, fast) {
  const gens = rest.filter(word => /^\d+$/.test(word)).slice(0, 2);
  if (!gens.length) throw new Error("review <genA> [genB]");
  const option = name => { const i = rest.indexOf("--" + name); return i >= 0 ? rest[i + 1] : undefined; };
  const at = option("at") ?? "spawn";
  const name = option("name") ?? `review-${gens.join("-vs-")}`;
  const common = `--gen ${gens.join(",")} --at ${at}`;
  const lines = [
    `stats ${common} --side 512 --spacing 1 --out ${out}/${name}-stats`,
    `patch ${common} --side 640 --px 640 --out ${out}/${name}-patch`,
    `patch ${common} --side 640 --px 640 --layer detail --out ${out}/${name}-detail`,
    `patch ${common} --side 160 --px 640 --out ${out}/${name}-close`,
    `view ${common} --alt 2 --yaw 20 --pitch -4 --size 800x500 --out ${out}/${name}-view-a`,
    `view ${common} --alt 2 --yaw 200 --pitch -4 --size 800x500 --out ${out}/${name}-view-b`,
    `view ${common} --alt 120 --yaw 110 --pitch -22 --size 800x500 --out ${out}/${name}-aerial`,
    `globe ${common} --size 720 --out ${out}/${name}-globe`,
  ];
  mkdirSync(join(root, out), { recursive: true });
  const file = join(root, out, `${name}.txt`);
  writeFileSync(file, lines.join("\n") + "\n");
  lab(["batch", file], fast);
}

try {
  let args = process.argv.slice(2);
  const fast = args[0] === "--fast";
  if (fast) args = args.slice(1);
  const [command = "help", ...rest] = args;
  if (command === "help") console.log(JSON.stringify({ commands: usage, exe: existsSync(exe) ? exe : "not built" }));
  else if (command === "build") {
    const run = spawnSync("cargo", buildArgs, { cwd: root, stdio: ["ignore", "ignore", "inherit"] });
    process.exitCode = run.status ?? 1;
    if (run.status === 0) console.log(JSON.stringify({ exe }));
  }
  else if (command === "review") review(rest, fast);
  else lab([command, ...rest], fast);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
