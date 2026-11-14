import fabricationMetadata from "./fabrication-contract.mjs";
import * as fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { parse, decode } from "./syntax.mjs";
import { checkPortalLayout, portalSources } from "./portal-validator.mjs";

import { catalogContracts, catalogContract, nativeCatalogs } from "./catalog-contracts.mjs";
export { nativeCatalogs } from "./catalog-contracts.mjs";

export const validatorInstruction = "Run node tools/axiom-dev/build-validators.mjs, then save again.";
export const validatorDirectory = root => path.join(root, "target", "axiom-dev", "native-validation");
export const binaryName = name => name + (process.platform === "win32" ? ".exe" : "");
export const hashBytes = bytes => createHash("sha256").update(bytes).digest("hex");

// Part/material/schema positions are persisted by inventory and workshop IDs.
// Keep those identities in place; native parsing owns the definition's types.
export function validateStableCatalogIds(catalog, before, after) {
  const sections = catalogContract(catalog)?.stableIdSections ?? [];
  if (!sections.length) return;
  const oldRoot = parse(before, "ron"), newRoot = parse(after, "ron");
  const ids = (root, section) => (section ? root.children.find(node => node.key === section)?.children : root.children)
    ?.map(row => decode(row.children.find(node => node.key === "id")?.raw ?? "")) ?? [];
  for (const section of sections) {
    const previous = ids(oldRoot, section), next = ids(newRoot, section);
    if (next.length < previous.length || previous.some((id, index) => id !== next[index])) {
      throw new Error("Keep existing " + (section || catalogContract(catalog)?.label || "catalog") + " IDs and row positions. Saved equipment refers to them; append new IDs instead.");
    }
  }
}

// Include the packed material table used by reaction validation. Editable spell,
// tool and reaction drafts do not invalidate their own checker binaries.
export function nativeSourcePaths(root) {
  const files = ["Cargo.toml", "Cargo.lock", "assets/materials.ron", "tools/axiom-dev/catalog-contracts.mjs"];
  if (fs.existsSync(path.join(root, "rust-toolchain.toml"))) files.push("rust-toolchain.toml");
  for (const crate of ["core", "content", "audio", "tools"]) {
    files.push(`crates/${crate}/Cargo.toml`);
    const walk = relative => {
      for (const entry of fs.readdirSync(path.join(root, relative), { withFileTypes: true })) {
        if (entry.isSymbolicLink()) throw new Error("Native validator source redirects elsewhere");
        const next = relative + "/" + entry.name;
        if (entry.isDirectory()) walk(next);
        else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(next);
      }
    };
    walk(`crates/${crate}/src`);
    if (fs.existsSync(path.join(root, `crates/${crate}/examples`))) walk(`crates/${crate}/examples`);
  }
  return files.sort();
}

export function nativeSourceFingerprint(root) {
  const digest = createHash("sha256").update("axiom-native-validation-v2\0").update(JSON.stringify(catalogContracts));
  for (const file of nativeSourcePaths(root)) {
    digest.update(file + "\0").update(hashBytes(fs.readFileSync(path.join(root, file))));
  }
  return digest.digest("hex");
}

export function validateNativeBundle(root, plans) {
  const changed = plans.filter(plan => !plan.original.equals(plan.next));
  if (changed.some(plan => plan.relative === "assets/portal_layout.ron") && changed.some(plan => portalSources.includes(plan.relative))) {
    throw new Error("Save and rebuild the portal schema before publishing a layout.");
  }
  if (!changed.some(plan => Object.hasOwn(nativeCatalogs, plan.relative))) return;
  const dependencies = new Set([...nativeSourcePaths(root), "tools/axiom-dev/fabrication-contract.mjs"]);
  if (changed.some(plan => dependencies.has(plan.relative))) {
    throw new Error("A catalog cannot be checked against unpublished native schema changes. Save and rebuild the native schema first, then review the catalog bundle.");
  }
}

export function checkedValidator(root, catalog) {
  const example = nativeCatalogs[catalog];
  if (!example) return null;
  const directory = validatorDirectory(root);
  try {
    const receiptPath = path.join(directory, "receipt.json");
    if (fs.realpathSync(receiptPath) !== receiptPath || fs.statSync(receiptPath).size > 64 * 1024) throw new Error();
    const receipt = JSON.parse(fs.readFileSync(receiptPath, "utf8"));
    if (receipt.version !== 1 || receipt.source !== nativeSourceFingerprint(root)) throw new Error();
    if (catalog === "assets/fabrication.ron") {
      const metadataPath = path.join(root, "tools/axiom-dev/fabrication-contract.mjs");
      if (fs.realpathSync(metadataPath) !== metadataPath
          || receipt.fabricationMetadata?.file !== hashBytes(fs.readFileSync(metadataPath))
          || receipt.fabricationMetadata?.data !== hashBytes(JSON.stringify(fabricationMetadata))) throw new Error();
    }
    const binary = path.join(directory, binaryName(example));
    if (fs.realpathSync(binary) !== binary || receipt.binaries?.[example] !== hashBytes(fs.readFileSync(binary))) throw new Error();
    return binary;
  } catch {
    throw new Error("Native content validators are missing or stale. " + validatorInstruction);
  }
}

export function validateNativeContent(root, catalog, text) {
  if (catalog === "assets/portal_layout.ron") { checkPortalLayout(root, text); return; }
  const binary = checkedValidator(root, catalog);
  if (!binary) return;
  if (typeof text !== "string" || Buffer.byteLength(text) > 4 * 1024 * 1024) {
    throw new Error("Native content draft exceeds the 4 MiB text limit");
  }
  const result = spawnSync(binary, ["-"], {
    cwd: root, input: text, encoding: "utf8", windowsHide: true,
    timeout: 10000, maxBuffer: 1024 * 1024,
  });
  if (result.error || result.signal) throw new Error("Native content validation could not complete. " + validatorInstruction);
  let report;
  try { report = JSON.parse(result.stdout); }
  catch { throw new Error("Native content validator returned an invalid receipt. " + validatorInstruction); }
  if (result.status !== 0 || report.valid !== true) {
    const errors = (report.diagnostics ?? []).slice(0, 20).map(entry => typeof entry === "string" ? entry
      : `${entry.id == null ? "Catalog" : "Spell " + entry.id}.${entry.field}: ${entry.message}`);
    throw new Error("Native validation rejected " + catalog + ": " + (errors.join("; ") || report.error || "invalid catalog"));
  }
  return report;
}
