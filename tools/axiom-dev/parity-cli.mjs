// Agent and human CLI over the mod-parity ledger. Output is minimal JSON:
// tables are {cols, rows} arrays, defaults are omitted, rules print once in help.
import { loadScenarios, runScenarios, staleContent } from "./factory-lab.mjs";
import { FAMILIES, INDUSTRIAL, RULES, STATUSES, brief, filterRows, loadLedger, nextRows, statusPath, writeStatus } from "./parity-data.mjs";

const usage = {
  "next [--family f] [--limit 5]": "open rows whose owners have code, earliest milestone first",
  "describe <ID>": "work order: mapping, owners [code|design], likely files, absent keywords",
  "list [--family f] [--status s] [--milestone M4] [--ready|--blocked] [--query t] [--limit 40]": "matching rows",
  "summary [--family f]": "counts per status and family",
  "verify <ID> [--status verified|playable] [--note t]": "run the headless scenarios tagged with ID; on pass record status with them as evidence",
  "set <ID> --status s [--evidence path#line|commit:sha]... [--note t] --rev r": "record progress; rev from any read",
};

function options(args) {
  const out = { _: [] };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (!arg.startsWith("--")) { out._.push(arg); continue; }
    const key = arg.slice(2);
    if (key === "ready" || key === "blocked") { out.ready = key === "ready"; continue; }
    const value = args[++i];
    if (value === undefined) throw new Error("Missing value for --" + key);
    if (key === "evidence") (out.evidence ??= []).push(value); else out[key === "revision" ? "rev" : key] = value;
  }
  return out;
}

// [id, label, status, milestone, owners] where owners is "ok" or the design-only owner ids.
const ROW_COLS = "id label status ms owners";
const tuple = row => [row.id, row.label, row.status, row.milestone, row.readiness.ready ? "ok" : row.readiness.missing.join(",")];

async function run() {
  const [command = "help", ...rest] = process.argv.slice(2);
  if (command === "help") return { commands: usage, statuses: STATUSES.join(" "), families: Object.keys(FAMILIES).join(" ") + " industrial",
    rules: RULES, ledger: statusPath };
  const opts = options(rest);
  const ledger = loadLedger();
  const rev = ledger.revision;
  if (command === "summary") {
    const rows = filterRows(ledger.rows, { family: opts.family });
    const status = {};
    for (const row of rows) status[row.status] = (status[row.status] ?? 0) + 1;
    const families = {};
    for (const key of Object.keys(FAMILIES)) {
      const own = rows.filter(row => row.families.includes(key));
      if (own.length) families[key] = `${own.filter(r => r.status === "verified").length}/${own.length} verified, ${own.filter(r => r.readiness.ready).length} ready`;
    }
    return { rev, rows: rows.length, ready: rows.filter(row => row.readiness.ready).length, status, families };
  }
  if (command === "list" || command === "next") {
    const limit = Math.min(Number(opts.limit ?? (command === "next" ? 5 : 40)), 500);
    const matched = command === "next"
      ? nextRows(filterRows(ledger.rows, { family: opts.family ?? INDUSTRIAL.join(",") }), limit)
      : filterRows(ledger.rows, opts);
    const out = { rev, cols: ROW_COLS, rows: matched.slice(0, limit).map(tuple) };
    if (matched.length > limit) out.total = matched.length;
    return out;
  }
  if (command === "describe") {
    const row = ledger.rows.find(entry => entry.id === opts._[0]?.toUpperCase());
    if (!row) throw new Error("Unknown ID; use list");
    return { rev, ...brief(row, ledger.systems) };
  }
  if (command === "verify") {
    const id = opts._[0]?.toUpperCase();
    if (!ledger.rows.some(row => row.id === id)) throw new Error("Unknown ID; use list");
    const scenarios = loadScenarios(s => (s.parity ?? []).some(entry => entry.toUpperCase() === id));
    if (!scenarios.length) throw new Error(`No scenario lists ${id} in "parity"; add tools/axiom-dev/scenarios/<name>.json`);
    const stale = staleContent();
    if (stale.length) throw new Error(`Executable older than ${stale.join(", ")}; run factory-lab-cli build`);
    const failed = (await runScenarios(scenarios)).filter(result => !result.pass);
    if (failed.length) { process.exitCode = 1; return { id, cols: "step path test want got", failures: Object.fromEntries(failed.map(r => [r.id, r.fail])) }; }
    const change = { status: opts.status ?? "verified", evidence: scenarios.map(s => s.file) };
    if (opts.note !== undefined) change.note = opts.note;
    const result = writeStatus(id, change, rev, new Set(ledger.rows.map(row => row.id)));
    return { id, status: result.after.status, scenarios: scenarios.length, rev: result.revision };
  }
  if (command === "set") {
    const id = opts._[0]?.toUpperCase();
    if (!id || !opts.rev) throw new Error("set <ID> --status s --rev r [--evidence …] [--note …]");
    const change = {};
    if (opts.status) change.status = opts.status;
    if (opts.evidence) change.evidence = opts.evidence;
    if (opts.note !== undefined) change.note = opts.note;
    const result = writeStatus(id, change, opts.rev, new Set(ledger.rows.map(row => row.id)));
    return { id, status: result.after.status, rev: result.revision };
  }
  throw new Error("Unknown command; use help");
}

run().then(out => console.log(JSON.stringify(out))).catch(error => { console.error(error.message); process.exitCode = 1; });
