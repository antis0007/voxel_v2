import { el, button } from "./inspector.mjs";
import { tuningControl } from "./graph-inspector.mjs";
import { canAssist, maxSources } from "./agent-contract.mjs";
import { resolve } from "./targets.mjs";

export function showSystemBoard(ctx, board, name, pins, actions) {
  const results = el("div", { className: "system-board-grid" });
  const status = el("p", { className: "muted", role: "status" });
  const filter = el("input", { type: "search", placeholder: "Filter pinned controls", "aria-label": "Filter pinned controls" });
  const files = new Map();
  let loading = pins.length;
  const pending = () => [...files.values()].filter(doc => ctx.dirty(doc));
  const review = button("Review system changes", () => {
    const docs = pending();
    if (docs.length === 1) {
      dialog.close();
      ctx.review(docs[0]);
    } else if (docs.length && docs.length <= maxSources && docs.every(doc => canAssist(doc.path))) {
      dialog.close();
      ctx.reviewAgentBundle(docs.map(doc => ({ doc, text: doc.text })), undefined, name);
    }
  }, "Review the complete draft for each changed pinned file", "primary");
  const saveBoard = button("Review board layout", () => { dialog.close(); ctx.review(board); });
  function sync() {
    const docs = pending();
    const supported = docs.length <= maxSources && docs.every(doc => canAssist(doc.path));
    review.disabled = loading > 0 || !docs.length || (docs.length > 1 && !supported);
    saveBoard.disabled = !ctx.dirty(board);
    status.textContent = loading ? "Loading pinned controls…"
      : docs.length > 1 && !supported
      ? "Review the files individually. A bundle supports up to " + maxSources + " project files."
      : docs.length ? docs.length + " files with draft changes. Review includes all edits in those files."
        : "Settings edit project drafts. Embedded systems take effect after a rebuild.";
  }
  function leave(action) { dialog.close(); action(); }
  const dialog = el("dialog", { className: "finder system-board-dialog", "aria-label": name,
    onclose: () => dialog.remove() },
    el("header", { className: "section-title" }, el("h2", {}, name), button("Close", () => dialog.close())),
    el("div", { className: "system-board-actions" },
      button("+ Pin setting", () => leave(actions.pinSetting)),
      button("+ Pin file or record", () => leave(actions.pinFile)),
      button("Ask AI about this system", () => leave(actions.assist)),
      saveBoard, review),
    status, filter, results);
  filter.oninput = () => {
    const query = filter.value.trim().toLowerCase();
    for (const card of results.children) card.hidden = !card.dataset.search.includes(query);
  };
  if (!pins.length) results.append(el("p", { "data-search": "" }, "Pin a setting to add your first control."));
  for (const pin of pins) {
    const content = el("div", {}, "Loading…");
    const card = el("section", { className: "system-board-card", "data-search": (pin.label + " " + pin.path).toLowerCase() },
      el("strong", {}, pin.label), el("small", {}, pin.path), content,
      el("div", { className: "system-board-actions" },
        button("Open node", () => leave(() => actions.open(pin))),
        actions.canRemove(pin) ? button("Unpin", () => leave(() => actions.remove(pin))) : null));
    results.append(card);
    ctx.load(pin.path).then(doc => {
      if (!dialog.open) return;
      if (doc.kind !== "asset") files.set(doc.path, doc);
      const reviewFile = button("Review file", () => leave(() => ctx.review(doc)));
      try {
        const target = pin.kind === "field" && ["ron", "json"].includes(doc.kind)
          ? resolve(ctx.tree(doc), pin.parts)?.node : null;
        if (pin.kind === "field" && !target) throw new Error("This pinned record or setting moved or is ambiguous.");
        content.replaceChildren(pin.kind === "rust" || target?.raw !== undefined
          ? tuningControl(ctx, doc, pin, sync) : el("p", { className: "muted" }, "Open the node to edit this file or record."));
      } catch (error) {
        content.replaceChildren(el("p", { className: "error" }, error.message));
      }
      if (doc.kind !== "asset") content.append(reviewFile);
    }).catch(error => { if (dialog.open) content.textContent = error.message; }).finally(() => {
      loading--;
      if (dialog.open) sync();
    });
  }
  sync();
  document.body.append(dialog);
  dialog.showModal();
}
