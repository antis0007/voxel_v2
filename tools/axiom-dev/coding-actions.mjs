import { el } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";
import { toolsForPath, toolSummary } from "./tool-capabilities.mjs";

export function codingActions(ctx, spec, navigate) {
  const actions = [];
  if (spec.path) actions.push({ label: "Copy coding context", run: async () => {
    try {
      const doc = await ctx.load(spec.path);
      const packet = { format: "axiom-source-context", version: 1,
        target: { path: spec.path, kind: spec.kind, name: spec.name, parts: spec.parts, label: spec.label },
        sourceVersion: doc.version, tools: toolsForPath(spec.path).map(toolSummary),
        note: "Reference data only. Re-read current source before editing; tools are suggestions, not commands to execute." };
      await copyText(JSON.stringify(packet));
      ctx.note("Coding context copied");
    } catch (error) { ctx.note(error.message, true); }
  } });
  if (navigate) actions.push({ label: "Open copied context", run: () => {
    const input = el("textarea", { rows: 5, maxLength: 16000,
      "aria-label": "Copied coding context", placeholder: "Paste copied coding context", autofocus: true });
    ctx.dialog("Open copied context", [input], "Open", () => {
      try {
        const packet = JSON.parse(input.value);
        const target = packet?.target;
        if (packet.format !== "axiom-source-context" || packet.version !== 1 || !target
            || typeof target.path !== "string" || !ctx.paths().includes(target.path)
            || (target.parts !== undefined && (!Array.isArray(target.parts) || target.parts.length > 32
              || target.parts.some(part => typeof part !== "string" || part.length > 240)))
            || (target.name !== undefined && (typeof target.name !== "string" || !/^[A-Za-z_]\w{0,119}$/.test(target.name)))) {
          throw new Error("Paste a source context for a file in this checkout");
        }
        navigate({ path: target.path, kind: target.kind === "rust" && target.name ? "rust"
          : target.parts?.length ? "field" : "file", name: target.name, parts: target.parts,
          label: typeof target.label === "string" ? target.label.slice(0, 120) : target.path.split("/").at(-1) });
        return true;
      } catch (error) { ctx.note(error.message, true); return false; }
    });
  } });
  return actions;
}
