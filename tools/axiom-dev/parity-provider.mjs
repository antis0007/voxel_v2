import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

// The one test for "this provider is out of quota". Claude: "You've hit your 5-hour limit · resets 5:30am (Etc/GMT+7)".
// Codex: "usage limit ... try again at ...". Every caller imports this, so a new wording is added in one place.
export const providerLimitText = /\b(?:rate limit(?:ed)?|usage limit|weekly limit|\d+-hour limit|hit your [\w -]{0,20}limit|limit resets|out of credits|credit balance|HTTP 429)\b/i;

const sourceTools = fileURLToPath(new URL("./parity-source-tools.mjs", import.meta.url));
const string = { type: "string" };
const object = properties => ({ type: "object", properties, required: Object.keys(properties), additionalProperties: false });
const schema = object({
  summary: string,
  checks: { type: "array", items: string },
  edits: { type: "array", items: object({ file: string, before: string, after: string, reason: string }) },
});

export async function runParityProvider({ provider, model, directory, snapshot, prompt, signal, onProgress, onUsage,
  responseSchema = schema, schemaFile, reasoning = "medium", timeoutMs = 8 * 60_000, lookupBudget = 70,
  readJournal, textReview = false }) {
  const started = Date.now();
  await mkdir(directory, { recursive: true });
  const config = snapshot && { command: process.execPath, args: [sourceTools, snapshot, String(lookupBudget),
    ...(readJournal ? [readJournal] : [])] };
  const schemaPath = schemaFile ?? path.join(directory, "schema.json");
  const output = path.join(directory, "proposal.json");
  const claude = provider === "claude";
  const executable = claude ? process.env.AXIOM_CLAUDE_PATH || "claude" : process.env.AXIOM_CODEX_PATH || "codex";
  let args;
  if (claude) {
    const mcp = path.join(directory, "mcp.json");
    await writeFile(mcp, JSON.stringify({ mcpServers: config ? { parity: config } : {} }));
    args = ["--print", "--output-format", "stream-json", "--verbose",
      "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config", "--mcp-config", mcp,
      "--tools", textReview ? "" : "StructuredOutput", "--permission-mode", "dontAsk", "--allowedTools",
      [!textReview && "StructuredOutput", config && "mcp__parity__find_source_paths,mcp__parity__search_source,mcp__parity__read_source"].filter(Boolean).join(","),
      "--effort", reasoning, "--max-turns", textReview ? "8" : config ? "35" : "5"];
    if (!textReview) args.push("--json-schema", JSON.stringify(responseSchema));
  } else {
    if (!schemaFile) await writeFile(schemaPath, JSON.stringify(responseSchema));
    args = ["--ask-for-approval", "never", "exec", "--sandbox", "read-only", "--ephemeral", "--ignore-user-config",
      "--ignore-rules", "--skip-git-repo-check", "--disable", "shell_tool", "--disable", "unified_exec", "--disable", "hooks",
      "--disable", "apps", "--disable", "multi_agent", "--disable", "code_mode", "--disable", "plugins",
      "-c", 'web_search="disabled"', "--json", "--output-schema", schemaPath,
      "--output-last-message", output];
    if (config) args.push("-c", "mcp_servers.parity.command=" + JSON.stringify(config.command),
      "-c", "mcp_servers.parity.args=" + JSON.stringify(config.args),
      "-c", 'mcp_servers.parity.default_tools_approval_mode="writes"');
    else args.push("-c", "mcp_servers={}");
    if (provider === "codex") args.push("-c", "model_reasoning_effort=" + JSON.stringify(reasoning));
    else args.push("--oss", "--local-provider", provider);
  }
  if (model) args.push("--model", model);
  if (!claude) args.push("-");
  const usage = { input: 0, output: 0, promptBytes: Buffer.byteLength(prompt) };
  let diagnostic = "", stderr = "", buffer = "", result, lookups = 0;
  const pendingLookups = new Set();
  function consume(line) {
    let event;
    try { event = JSON.parse(line); } catch { return; }
    if (event.type === "result") result = event;
    if (event.type === "turn.completed" || event.type === "result") {
      usage.input += (event.usage?.input_tokens ?? 0) + (event.usage?.cache_read_input_tokens ?? 0)
        + (event.usage?.cache_creation_input_tokens ?? 0);
      usage.output += event.usage?.output_tokens ?? 0;
    }
    if (event.type === "error" || event.type === "turn.failed") diagnostic = event.error?.message ?? event.message;
    const oldCount = lookups;
    const item = event.item;
    if (event.type === "item.completed" && item?.type === "mcp_tool_call" && item.server === "parity"
        && item.status === "completed" && !item.error) lookups++;
    const content = event.message?.content;
    for (const block of Array.isArray(content) ? content : []) {
      if (event.type === "assistant" && block.type === "tool_use" && block.name?.startsWith("mcp__parity__")) pendingLookups.add(block.id);
      if (event.type === "user" && block.type === "tool_result" && pendingLookups.delete(block.tool_use_id) && !block.is_error) lookups++;
    }
    if (oldCount !== lookups) onProgress?.(lookups);
  }
  const timeout = AbortSignal.timeout(timeoutMs);
  const exit = await new Promise((resolve, reject) => {
    let spawnError;
    const child = spawn(executable, args, { cwd: directory, windowsHide: true,
      env: claude ? { ...process.env, ENABLE_TOOL_SEARCH: "false", MAX_STRUCTURED_OUTPUT_RETRIES: "2" } : process.env,
      signal: AbortSignal.any([signal, timeout]), stdio: ["pipe", "pipe", "pipe"] });
    child.once("error", error => { spawnError = error; });
    child.once("close", code => {
      if (buffer) consume(buffer);
      if (spawnError) reject(spawnError);
      else resolve(code);
    });
    child.stdin.on("error", () => {});
    child.stdin.end(prompt);
    child.stderr.setEncoding("utf8").on("data", chunk => { stderr = (stderr + chunk).slice(-4000); });
    child.stdout.setEncoding("utf8").on("data", chunk => {
      buffer += chunk;
      for (let end; (end = buffer.indexOf("\n")) >= 0;) {
        consume(buffer.slice(0, end));
        buffer = buffer.slice(end + 1);
      }
      if (buffer.length > 2 * 1024 * 1024) buffer = "";
    });
  }).catch(error => {
    if (timeout.aborted) throw Error(provider + " exceeded its time limit.");
    if (error.code === "ENOENT") throw Error(provider + " CLI was not found. Check PATH or the configured executable.");
    throw error;
  }).finally(() => {
    onUsage?.(usage);
  });
  if (exit !== 0 || result?.is_error) {
    throw Error(provider + " exited with " + exit + " · " + String(diagnostic || result?.result || result?.errors?.join(" ") || stderr).trim().slice(-700));
  }
  const proposal = claude
    ? (textReview && typeof result?.result === "string" && result.result.trim()
      ? { summary: result.result.trim().slice(0, 5500), checks: [], edits: [] } : result?.structured_output)
    : JSON.parse(await readFile(output, "utf8"));
  if (!proposal) throw Error(provider + " returned no structured proposal.");
  return { proposal, usage, lookups, elapsedMs: Date.now() - started };
}
