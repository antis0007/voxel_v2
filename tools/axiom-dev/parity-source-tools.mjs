// Stdio MCP transport shared by Codex and Claude Code. No tool can write, run a
// project command, or read a path outside the prepared snapshot.
import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { appendFile, readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { promisify } from "node:util";

const root = await realpath(process.argv[2]);
const readJournal = process.argv[4];
const exec = promisify(execFile);
const allowed = /^(?:crates|assets|docs|tools\/axiom-dev\/scenarios)\/(?:[\w .-]+\/)*[\w .-]+\.(?:rs|ron|toml|md|json|wgsl)$/i;
const specificationText = /^docs\/specs\/(?:[\w .-]+\/)*[\w .-]+\.txt$/i;
const areas = ["crates", "assets", "docs", "tools/axiom-dev/scenarios"];
const scopePath = /^(?:crates|assets|docs|tools\/axiom-dev\/scenarios)(?:\/[\w][\w .-]*)*$/;
const rg = ["--no-config", "--hidden", "--no-ignore", "--glob", "!.*", "--glob", "*.{rs,ron,toml,md,json,wgsl}", "--glob", "docs/specs/**/*.txt"];
const tools = [
  { name: "find_source_paths", description: "Find files by relative path fragments, including module names, even if their names do not appear in source text. All fragments must match.",
    inputSchema: { type: "object", required: ["fragments"], properties: { fragments: { type: "array", items: { type: "string" } },
      within: { type: "string" } } } },
  { name: "search_source", description: "Find literal symbols or text in the snapshot. Supply an exact relative file or directory as within to trace local definitions and callers.",
    inputSchema: { type: "object", required: ["text"], properties: { text: { type: "string" },
      area: { type: "string", enum: areas }, within: { type: "string" } } } },
  { name: "read_source", description: "Read numbered lines from a snapshot file. Use exact file paths from search results.",
    inputSchema: { type: "object", required: ["file"], properties: { file: { type: "string" }, line: { type: "integer" },
      count: { type: "integer" } } } },
].map(tool => ({ ...tool, annotations: { readOnlyHint: true, openWorldHint: false } }));
let calls = 0;
const budget = Math.min(70, Math.max(1, Number(process.argv[3]) || 70));
const visiblePath = file => (allowed.test(file) || specificationText.test(file)) && !file.split("/").some(part => part.startsWith("."));
const validText = (text, limit) => typeof text === "string" && text.trim() && text.length <= limit && !/[\r\n\0]/.test(text);

async function sourceRoots(within, area) {
  if (within === undefined) return (area ? [area] : areas).filter(directory => existsSync(path.join(root, directory)));
  if (typeof within !== "string" || !scopePath.test(within) || within.split("/").some(part => part.endsWith(".") || part.endsWith(" "))) {
    throw Error("Provide an exact relative snapshot file or directory.");
  }
  const resolved = await realpath(path.join(root, within)).catch(error => {
    if (error.code !== "ENOENT") throw error;
  });
  if (!resolved) return [];
  if (path.relative(root, resolved).split(path.sep).join("/") !== within) throw Error("Scope is outside the snapshot.");
  const info = await stat(resolved);
  return info.isDirectory() || (info.isFile() && visiblePath(within)) ? [within] : [];
}

async function runRg(args) {
  try {
    const { stdout } = await exec("rg", [...rg, ...args], {
      cwd: root, encoding: "utf8", maxBuffer: 2_000_000, timeout: 8000, windowsHide: true,
    });
    return stdout.trimEnd().split(/\r?\n/).filter(Boolean);
  } catch (error) {
    if (error.code === 1) return [];
    throw Error(error.code === "ENOENT" ? "Source search needs rg installed." : "Search is too broad or unavailable.");
  }
}

async function call(name, args) {
  if (++calls > budget) throw Error("Source lookup budget reached. Complete the response using evidence gathered so far.");
  if (name === "find_source_paths") {
    const { fragments, within } = args;
    if (!Array.isArray(fragments) || !fragments.length || fragments.length > 6 || fragments.some(part => !validText(part, 80))) {
      throw Error("Provide one to six path fragments of up to 80 characters.");
    }
    const roots = await sourceRoots(within);
    if (!roots.length) return "No matching paths.";
    const terms = fragments.map(part => part.trim().toLowerCase());
    const matches = (await runRg(["--files", "--", ...roots])).map(file => file.replaceAll("\\", "/"))
      .filter(file => visiblePath(file) && terms.every(term => file.toLowerCase().includes(term))).sort();
    return matches.slice(0, 45).join("\n") + (matches.length > 45 ? "\nNarrow the path fragments or scope." : "") || "No matching paths.";
  }
  if (name === "search_source") {
    const { text, area, within } = args;
    if (!validText(text, 180) ||
        (area !== undefined && !areas.includes(area))) throw Error("Provide a literal query of 1 to 180 characters.");
    const roots = await sourceRoots(within, area);
    if (!roots.length) return "No matches.";
    const lines = (await runRg(["--with-filename", "-n", "-i", "-F", "--max-columns", "240",
      "--max-count", within ? "12" : "3", "--", text, ...roots]))
      .map(line => line.replace(/^[^:]+/, file => file.replaceAll("\\", "/")))
      .filter(line => visiblePath(line.split(":")[0]));
    return lines.slice(0, 45).join("\n") + (lines.length > 45 ? "\nNarrow the query or select one area." : "") || "No matches.";
  }
  if (name !== "read_source") throw Error("Unknown snapshot tool.");
  const { file, line = 1, count = 75 } = args;
  if (typeof file !== "string" || !visiblePath(file)
      || !Number.isSafeInteger(line) || line < 1 || !Number.isSafeInteger(count) || count < 1 || count > 140) {
    throw Error("Provide a snapshot source path and up to 140 lines.");
  }
  const target = await realpath(path.join(root, file));
  if (path.relative(root, target).split(path.sep).join("/") !== file || (await stat(target)).size > 4_194_304) {
    throw Error("Source is outside the snapshot or exceeds the read limit.");
  }
  const lines = (await readFile(target, "utf8")).split(/\r?\n/);
  const excerpt = lines.slice(line - 1, line - 1 + count).map((text, i) => (line + i) + " | " + text)
    .join("\n").slice(0, 16_000);
  if (readJournal && excerpt) {
    await appendFile(readJournal, JSON.stringify({ file, line, count }) + "\n");
  }
  return excerpt || "No lines at this offset.";
}

const input = createInterface({ input: process.stdin });
for await (const line of input) {
  let request;
  try { request = JSON.parse(line); } catch { continue; }
  if (request.id === undefined) continue;
  let result, error;
  switch (request.method) {
    case "initialize":
      result = { protocolVersion: request.params.protocolVersion, capabilities: { tools: {} },
        serverInfo: { name: "parity-snapshot", version: "1.0.0" } };
      break;
    case "ping": result = {}; break;
    case "tools/list": result = { tools }; break;
    case "tools/call":
      try { result = { content: [{ type: "text", text: await call(request.params.name, request.params.arguments ?? {}) }] }; }
      catch (failure) { result = { isError: true, content: [{ type: "text", text: failure.message }] }; }
      break;
    default: error = { code: -32601, message: "Method not found" };
  }
  process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: request.id, result, error }) + "\n");
}
