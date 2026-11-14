import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, lstatSync, existsSync, realpathSync, writeFileSync, renameSync, rmSync } from "node:fs";
import path from "node:path";
import { workshopPath } from "./workshop-process.mjs";

function directory(root, relative) {
  let current = root;
  for (const part of relative.split("/")) {
    current = path.join(current, part);
    if (!existsSync(current)) mkdirSync(current);
    if (!lstatSync(current).isDirectory() || lstatSync(current).isSymbolicLink() || realpathSync(current) !== current) {
      throw new Error("Workshop staging directory redirects elsewhere.");
    }
  }
  return current;
}

export function stageContent(root, plan, snapshot) {
  const parent = directory(root, "target/axiom-dev/workshop");
  const pending = mkdtempSync(path.join(parent, ".pending-"));
  const id = "content-" + path.basename(pending).slice(".pending-".length);
  const destination = path.join(parent, id);
  let moved = false;
  try {
    for (const [name, text] of [["baseline", plan.baseline], ["candidate", plan.candidate]]) {
      const assets = directory(pending, name + "/assets");
      writeFileSync(path.join(assets, "fabrication.ron"), text, { flag: "wx" });
    }
    const manifest = { schema: 1, id, kind: "content-snapshot", at: new Date().toISOString(),
      sourceVersion: plan.version, approval: plan.approval, dependencies: plan.dependencies,
      source: workshopPath, changes: plan.changes, production: plan.production, planningScope: plan.planningScope, evidence: plan.evidence,
      contentRevisions: { baseline: createHash("sha256").update(plan.baseline).digest("hex"),
        candidate: createHash("sha256").update(plan.candidate).digest("hex") },
      files: { baseline: "baseline/assets/fabrication.ron", candidate: "candidate/assets/fabrication.ron" },
      worldCopied: false, originalModified: false,
      activation: "No native runtime loading, world copy or installation occurred. This is an authoring snapshot, not a playable pack." };
    writeFileSync(path.join(pending, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
    if (snapshot().version !== plan.version) throw new Error("Workshop dependencies changed during staging. Candidate discarded.");
    renameSync(pending, destination);
    moved = true;
    return { status: "content-staged", directory: destination, manifest };
  } finally {
    if (!moved && path.dirname(pending) === parent && path.basename(pending).startsWith(".pending-")) {
      rmSync(pending, { recursive: true, force: true });
    }
  }
}

