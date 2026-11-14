import test from "node:test";
import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

test("nearby regression evidence survives overlapping implementation excerpts", async () => {
  const base = mkdtempSync(path.join(tmpdir(), "forge-relevance-"));
  try {
    const toolPath = path.join(base, "tools", "axiom-dev");
    mkdirSync(toolPath, { recursive: true });
    for (const entry of readdirSync(fileURLToPath(new URL(".", import.meta.url)))) {
      if (entry.endsWith(".mjs")) copyFileSync(new URL(entry, import.meta.url), path.join(toolPath, entry));
    }
    for (const folder of ["crates", "assets", "docs"]) mkdirSync(path.join(base, folder));
    const owner = "crates/neutral.rs";
    writeFileSync(path.join(base, owner), "// Shipping inventory capacity\n"
      + "fn transfer_inventory(stock: u32, capacity: u32) -> bool { stock < capacity }\n"
      + "#[cfg(test)]\nmod tests {\n#[test]\n"
      + "fn inventory_capacity() { assert!(!super::transfer_inventory(4, 4)); }\n}\n");
    const { collectContext } = await import(pathToFileURL(path.join(toolPath, "agent-context.mjs")).href);
    const result = await collectContext({ task: "Inspect shipping inventory capacity before consumption", paths: [owner] });
    assert(result.evidence.some(row => row.path === owner && row.kind !== "Existing test source"), "retain implementation evidence");
    assert(result.evidence.some(row => row.path === owner && row.kind === "Existing test source"), "retain nearby tests as explicit counterevidence");
    assert(result.evidence.length <= 12, "preserve context budget");
  } finally {
    assert(path.resolve(base).startsWith(path.resolve(tmpdir()) + path.sep));
    rmSync(base, { recursive: true, force: true });
  }
});
