import fs from "node:fs";
import path from "node:path";
import http from "node:http";
import { createHash, randomBytes } from "node:crypto";
import { fileURLToPath } from "node:url";

const assets = new Map([["/foundry", ["foundry.html", "text/html"]],
  ["/foundry-ui.mjs", ["foundry-ui.mjs", "text/javascript"]],
  ["/foundry.css", ["foundry.css", "text/css"]], ["/foundry-link.mjs", ["foundry-link.mjs", "text/javascript"]],
  ["/panel-organizer.mjs", ["panel-organizer.mjs", "text/javascript"]],
  ["/command-palette.mjs", ["command-palette.mjs", "text/javascript"]],
  ["/devtools-qol.css", ["devtools-qol.css", "text/css"]]]);
export function serveFoundryAsset(route, response) {
  if (!assets.has(route)) return false;
  const [file, type] = assets.get(route);
  response.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" });
  response.end(fs.readFileSync(new URL(file, import.meta.url)));
  return true;
}

export function createFoundry(root) {
  const base = fs.realpathSync(root);
  function read(relative, max = 4 * 1024 * 1024) {
    const full = fs.realpathSync(path.resolve(base, relative));
    const rel = path.relative(base, full);
    if (path.isAbsolute(rel) || rel === ".." || rel.startsWith(".." + path.sep)) throw new Error("Evidence path escapes root");
    const info = fs.statSync(full);
    if (!info.isFile() || info.size > max) throw new Error("Evidence exceeds read limit");
    return fs.readFileSync(full);
  }
  const json = relative => JSON.parse(read(relative).toString("utf8"));
  function receipts() {
    return fs.readdirSync(base, { withFileTypes: true }).filter(e => e.isDirectory() && !e.isSymbolicLink())
      .flatMap(entry => {
        const file = entry.name + "/receipt.json";
        if (!fs.existsSync(path.join(base, file))) return [];
        try {
          const r = json(file);
          if (r.schema !== "axiom.foundry.receipt.v1") return [];
          return [{ id: entry.name, created: r.created_utc, source: r.source?.commit,
            build: r.build?.provenance, hardwareComplete: r.hardware?.complete,
            conclusion: r.conclusion ?? "Collection in progress", mergeAuthorized: false,
            runs: r.runs ?? [], images: Object.keys(r.artifacts ?? {}).filter(p => p.endsWith(".png")) }];
        } catch (error) { return [{ id: entry.name, error: error.message }]; }
      });
  }
  function optional(file) { return fs.existsSync(path.join(base, file)) ? json(file) : null; }
  function performance() {
    return fs.readdirSync(base, { withFileTypes: true }).filter(e => e.isDirectory() && !e.isSymbolicLink())
      .flatMap(entry => {
        const file = entry.name + "/performance.json";
        if (!fs.existsSync(path.join(base, file))) return [];
        try { const report = json(file); return report.schema === "axiom.foundry.performance.v1" ? [{ ...report, id: entry.name, merge_authorized: false }] : []; }
        catch (error) { return [{ id: entry.name, status: "unavailable", error: error.message }]; }
      });
  }
  return {
    snapshot() {
      let research = [];
      const ledger = "foundry-research/research.jsonl";
      if (fs.existsSync(path.join(base, ledger))) research = read(ledger).toString("utf8").trim().split("\n").filter(Boolean)
        .map(line => { const e = JSON.parse(line); return { hypothesis: e.experiment?.hypothesis,
          status: e.status, conclusion: e.conclusion, verification: e.verification }; });
      return { updated: new Date().toISOString(), receipts: receipts(), performance: performance(), research,
        comparisons: ["foundry-comparison.json", "foundry-repeatability-comparison.json"].flatMap(file => {
          const value = optional(file); return value ? [{ file, ...value }] : [];
        }).concat(fs.readdirSync(base, { withFileTypes: true }).filter(e => e.isFile() && e.name.endsWith("-comparison.json"))
          .flatMap(e => { const value = optional(e.name); return value?.schema === "axiom.foundry.performance-comparison.v1" ? [{ ...value, file: e.name, merge_authorized: false }] : []; })),
        references: JSON.parse(fs.readFileSync(new URL("foundry-references.json", import.meta.url), "utf8")),
        mergeAuthorized: false, interpretation: "Reported evidence only. Artifact integrity and acceptance require the Python verifier." };
    },
    artifact(id, relative) {
      if (!/^[a-zA-Z0-9_-]+$/.test(id) || typeof relative !== "string" || relative.includes("\\")
        || relative.startsWith("/") || relative.split("/").includes("..")) throw new Error("Invalid evidence ID");
      const r = fs.existsSync(path.join(base, id, "receipt.json")) ? json(id + "/receipt.json") : json(id + "/performance.json");
      const manifest = r.artifacts ?? r.raw_artifacts ?? {};
      if (!Object.hasOwn(manifest, relative) || !/\.(png|csv|log|json)$/.test(relative)) throw new Error("Artifact not listed in receipt");
      const bytes = read(id + "/" + relative, 32 * 1024 * 1024);
      if (createHash("sha256").update(bytes).digest("hex") !== manifest[relative]) throw new Error("Artifact hash mismatch");
      return { bytes,
        type: relative.endsWith(".png") ? "image/png" : "text/plain; charset=utf-8" };
    },
  };
}

export function serveFoundryApi(url, response, root = process.env.AXIOM_FOUNDRY_ROOT) {
  if (!["/api/foundry", "/api/foundry/artifact"].includes(url.pathname)) return false;
  try {
    if (!root) throw new Error("AXIOM_FOUNDRY_ROOT is not configured");
    const service = createFoundry(root);
    if (url.pathname.endsWith("/artifact")) {
      const a = service.artifact(url.searchParams.get("receipt"), url.searchParams.get("path"));
      response.writeHead(200, { "Content-Type": a.type, "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" });
      response.end(a.bytes);
    } else {
      response.writeHead(200, { "Content-Type": "application/json", "Cache-Control": "no-store" });
      response.end(JSON.stringify(service.snapshot()));
    }
  } catch (error) {
    response.writeHead(400, { "Content-Type": "application/json" });
    response.end(JSON.stringify({ error: error.message }));
  }
  return true;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const root = process.argv[2];
  if (!root) throw new Error("Usage: node foundry-service.mjs <evidence-root>");
  const token = randomBytes(24).toString("hex");
  let origin;
  const server = http.createServer((request, response) => {
    const url = new URL(request.url, origin);
    if (request.headers.host !== new URL(origin).host || request.headers.origin && request.headers.origin !== origin) {
      response.writeHead(403).end(); return;
    }
    if (request.method === "GET" && serveFoundryAsset(url.pathname, response)) return;
    if (request.method !== "GET" || request.headers["x-axiom-session"] !== token) { response.writeHead(403).end(); return; }
    if (!serveFoundryApi(url, response, root)) response.writeHead(404).end();
  });
  server.listen(0, "127.0.0.1", () => {
    origin = "http://127.0.0.1:" + server.address().port;
    console.log(origin + "/foundry#" + token);
  });
}
