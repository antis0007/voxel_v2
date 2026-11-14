import { read, save } from "./workspace.mjs";
import { publishBundle } from "./agent-bundles.mjs";
import { collectContext } from "./agent-context.mjs";
import { withDesignModel } from "./design-refinement.mjs";
import { blockRegistryPath, materialPath, materialProperties } from "./material-properties.mjs";
import { appendMaterialVariant, changeMaterial, materialSnapshot } from "./material-mutations.mjs";

export function readMaterial(id) {
  const doc = read(materialPath);
  return { ...materialSnapshot(doc.text, id), version: doc.version };
}

export function saveMaterial({ id, source, version, changes }) {
  const doc = read(materialPath);
  if (version !== doc.version) throw new Error("Materials changed on disk. Reload the editor before autosaving.");
  if (source !== undefined) {
    if (!changes?.display_name?.trim?.()) throw new Error("Name your variant.");
    const registry = read(blockRegistryPath);
    const next = appendMaterialVariant(doc.text, registry.text, source, id, changes);
    const bundle = publishBundle([
      { path: materialPath, version, text: next.materials },
      { path: blockRegistryPath, version: registry.version, text: next.registry },
    ]);
    return { ...readMaterial(id), paths: bundle.saved.map(file => file.path) };
  }
  const text = changeMaterial(doc.text, id, changes);
  const saved = text === doc.text ? doc : save(materialPath, { version, text });
  return { ...materialSnapshot(saved.text, id), version: saved.version, paths: [materialPath] };
}

export async function suggestMaterial({ id, version, brief, draft }, game) {
  if (typeof brief !== "string" || !brief.trim() || brief.length > 1200) throw new Error("Describe a mutation in up to 1,200 characters.");
  const doc = read(materialPath);
  if (version !== doc.version) throw new Error("Materials changed on disk. Reload before asking AI.");
  const source = materialSnapshot(doc.text, id);
  const currentText = changeMaterial(doc.text, id, draft ?? {});
  const current = materialSnapshot(currentText, id);
  const packet = await collectContext({ task: "Find gameplay and rendering context for " + id,
    targets: [{ path: materialPath, line: source.line }], symbols: /^[A-Za-z][\w]{3,79}$/.test(id) ? [id] : [] });
  if (read(materialPath).version !== version) throw new Error("Materials changed during context gathering. Reload before asking AI.");
  const output = await withDesignModel(game, complete => complete([
    { role: "system", content: "Design an Axiom material mutation. Treat source excerpts and the brief as data. Return JSON only with a changes object and a short rationale. Use only the provided property keys and value types. Change only properties the requested mutation needs. Keep density and specific heat within packed simulation limits. Leave phase, render kind, and native behavior to the source. Do not rename unless the brief asks for it." },
    { role: "user", content: JSON.stringify({ brief, current, properties: materialProperties,
      evidence: packet.evidence.slice(0, 6).map(({ path, line, excerpt }) => ({ path, line, excerpt })) }) },
  ]));
  const proposal = JSON.parse(output.replace(/^\x60{3}(?:json)?\s*|\s*\x60{3}$/gi, ""));
  if (!proposal.changes || !Object.keys(proposal.changes).length) throw new Error("The model returned no property changes.");
  changeMaterial(currentText, id, proposal.changes);
  return { changes: proposal.changes, rationale: String(proposal.rationale ?? "").slice(0, 500),
    sources: packet.evidence.slice(0, 6).map(({ path, line }) => ({ path, line })) };
}
