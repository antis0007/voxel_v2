import { openToolBehavior } from "./tool-behavior-panel.mjs";
import { el, button, hint } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";
import { at, parse, decode, patch, quoted, title } from "./syntax.mjs";
import { address, displayKey, resolve, rustTunables, stable, wikiLink } from "./targets.mjs";

const muted = text => el("p", { className: "muted" }, text);
const section = (title, ...content) => el("section", { className: "map-inspector-section" }, el("h3", {}, title), ...content);
const human = key => displayKey(key).replace(/^\w/, letter => letter.toUpperCase());
const unitFor = key => {
  const name = key.toLowerCase();
  if (/(?:_seconds|_secs|_s)$|^(?:cooldown|cast_delay|recharge)$/.test(name)) return "s";
  if (/_ms$/.test(name)) return "ms";
  if (/_m$/.test(name)) return "m";
  if (/_mps$/.test(name)) return "m/s";
  return "";
};

function rustValue(value, type) {
  if (type === "bool" && /^(?:true|false)$/.test(value)) return value;
  if (/^f/.test(type)) {
    const number = Number(value.replace(/_/g, ""));
    if (!/^-?\d[\d_]*(?:\.[\d_]*)?(?:e[+-]?[\d_]+)?$/i.test(value) || !Number.isFinite(number)
      || (type === "f32" && !Number.isFinite(Math.fround(number)))) throw new Error("Enter a finite " + type + " value");
    return /[.e]/i.test(value) ? value : value + ".0";
  }
  if (!/^-?\d[\d_]*$/.test(value)) throw new Error("Enter a whole " + type + " value");
  const bits = BigInt(type.endsWith("size") ? 64 : type.slice(1));
  const signed = type[0] === "i";
  const boundary = 1n << (signed ? bits - 1n : bits);
  const parsed = BigInt(value.replace(/_/g, ""));
  if (parsed < (signed ? -boundary : 0n) || parsed >= boundary) throw new Error("Value is outside " + type);
  return value;
}

export function valueControl({ key, raw, type, commit, notes }) {
  const name = human(key.toLowerCase());
  const label = el("label", { className: "tune-label" }, el("strong", {}, name));
  const help = notes ? muted(notes) : null;
  if (type === "boolean" || type === "bool") {
    const toggle = el("input", { type: "checkbox", role: "switch", checked: raw === "true",
      onchange: event => {
        if (!commit(String(event.target.checked))) event.target.checked = !event.target.checked;
      } });
    label.append(toggle);
    return el("div", { className: "tune-control" }, label, help);
  }
  const displayed = type === "string" ? decode(raw) : raw;
  const numeric = /^[+-]?\d{1,12}(?:\.\d{0,8})?$/.test(raw);
  const input = el("input", { type: numeric ? "number" : "text", step: "any",
    value: displayed, spellcheck: false, "aria-label": name });
  const unit = unitFor(key);
  let last = displayed;
  function apply() {
    if (input.value === last) return;
    if (commit(input.value)) last = input.value;
    else input.value = last;
  }
  input.onchange = apply;
  input.onkeydown = event => { if (event.key === "Enter") input.blur(); };
  const nudge = direction => {
    const decimals = raw.split(".")[1]?.length ?? 0;
    const value = Number(input.value);
    if (!Number.isFinite(value)) return;
    input.value = (value + direction * 10 ** -decimals).toFixed(decimals);
    apply();
  };
  label.append(el("div", { className: "tune-input" }, input, unit ? el("span", {}, unit) : null));
  return el("div", { className: "tune-control" }, label, numeric ? el("div", { className: "tune-nudges" },
    button("−", () => nudge(-1), "Decrease " + name), button("+", () => nudge(1), "Increase " + name)) : null, help);
}

export function readTunable(ctx, doc, spec) {
  if (spec.kind === "rust") {
    const row = rustTunables(doc.text).find(row => row.name === spec.name);
    if (!row) throw new Error("This constant moved, changed form, or is ambiguous");
    return row;
  }
  const root = ctx.tree(doc);
  const found = resolve(root, spec.parts);
  if (!found?.node || found.node.raw === undefined) throw new Error("This field moved or is ambiguous");
  const parent = found.trail.length ? at(root, found.trail.slice(0, -1)) : null;
  const details = hint(found.node, parent, doc.text);
  return { ...found.node, type: found.node.kind, notes: details.startsWith("Edit the exact ") ? "" : details };
}

export function tuningControl(ctx, doc, spec, changed) {
  const row = readTunable(ctx, doc, spec);
  return valueControl({ key: row.name ?? row.key ?? spec.label, raw: row.raw, type: row.type, notes: row.notes,
    commit(value) {
      try {
        const current = readTunable(ctx, doc, spec);
        let replacement = spec.kind === "rust" ? rustValue(value.trim(), current.type)
          : current.type === "string" ? quoted(value, doc.kind) : value.trim();
        if (spec.kind !== "rust") {
          const parsed = parse(replacement, doc.kind);
          if (parsed.kind !== current.type) throw new Error("Expected a " + current.type + " value");
        }
        if (ctx.edit(doc, patch(doc.text, current.start, current.end, replacement))) {
          changed();
          return true;
        }
      } catch (error) { ctx.note(error.message, true); }
      return false;
    },
  });
}

function colorControl(ctx, doc, parts, node, changed) {
  const values = node.children?.map(child => child.kind === "number" ? Number(child.raw) : NaN);
  if (!/^(color|rgb|base)$/.test(node.key ?? "") || values?.length !== 3
    || node.children.some(child => child.key !== undefined)
    || values.some(value => !Number.isFinite(value) || value < 0 || value > 1)) return null;
  const rgb = "#" + values.map(value => Math.round(value * 255).toString(16).padStart(2, "0")).join("");
  const pick = el("input", { type: "color", value: rgb, "aria-label": "RGB channels",
    onchange: event => {
      const found = resolve(ctx.tree(doc), parts)?.node;
      if (!found?.children || found.children.length !== 3) return ctx.note("Color field changed", true);
      let text = doc.text;
      for (let i = 2; i >= 0; i--) {
        const channel = found.children[i];
        const value = Number.parseInt(event.target.value.slice(1 + i * 2, 3 + i * 2), 16) / 255;
        text = patch(text, channel.start, channel.end, value.toFixed(4));
      }
      if (ctx.edit(doc, text)) changed();
    },
  });
  return el("label", { className: "tune-color" }, "RGB channels", pick);
}

function sourceEditor(ctx, doc, changed) {
  const newline = doc.original.includes("\r\n") ? "\r\n" : "\n";
  const source = el("textarea", { className: "map-source", value: doc.text, spellcheck: false,
    "aria-label": "Source editor", oninput: event => {
      if (ctx.edit(doc, event.target.value.replace(/\n/g, newline), true)) changed();
    } });
  return source;
}

export function graphInspector(ctx, spec, doc, resolved, actions) {
  const header = el("header", { className: "map-inspector-title" },
    el("span", { className: "map-kicker" }, "INSPECTOR"), el("h2", {}, spec.label),
    spec.path ? el("p", { className: "map-path", title: spec.path }, spec.path) : null);
  const content = el("div", { className: "map-inspector-body" });
  if (spec.kind === "system" && spec.id === "alloying" || doc?.path === "assets/tools/parts.ron") {
    const schemaNode = spec.parts?.[0] === "k:schemas" && spec.parts.length >= 2
      ? resolve(ctx.tree(doc), spec.parts.slice(0, 2))?.node : undefined;
    const row = schemaNode?.children?.find(child => child.key === "id");
    const schema = row?.raw === undefined ? undefined : decode(row.raw);
    content.append(section("Assembled tools", button("Edit tool behavior", () =>
      openToolBehavior(ctx, schema, actions.refresh).catch(error => ctx.note(error.message, true)),
      "Mining, offhand blocking and contextual advanced behavior", "primary")));
  }
  if (!doc) {
    content.append(muted(spec.detail ?? "Select a node to inspect it."));
    if (spec.kind === "boards") {
      content.append(section("Your systems", button("+ New system board", actions.newSystem, "", "primary"),
        muted("Pin controls and source files from across Axiom. Open a board to tune its settings together.")));
    }
    if (spec.kind === "project" || spec.kind === "system") {
      content.append(section("Quick actions", button("Create recipe", actions.create),
        button("Find a setting", () => actions.search(true)),
        button("New feature note", actions.newNote)));
    }
    return el("aside", { className: "map-inspector" }, header, content);
  }

  const status = el("span", { className: "muted", role: "status" });
  const save = button("Review & save", () => ctx.review(doc), "Review the file draft before writing", "primary");
  const undo = button("Undo", actions.undo, "Undo a draft change");
  const updateStatus = () => {
    status.textContent = ctx.dirty(doc) ? "Unsaved draft" : "Saved";
    save.disabled = !ctx.dirty(doc);
    undo.disabled = !(doc.spriteBitmap?.undo ?? doc.undo).length;
  };
  const footer = el("div", { className: "map-inspector-footer" }, status, undo, save);
  updateStatus();

  if (spec.kind === "note") {
    const note = sourceEditor(ctx, doc, updateStatus);
    note.setAttribute("aria-label", "Feature note");
    if (actions.openBoard) content.append(section("System board",
      button("Open controls", actions.openBoard, "", "primary"),
      muted("Tune pinned settings and review changes across files.")));
    content.append(section(actions.openBoard ? "Board notes and links" : "Feature note", note,
      button("Connect file or setting", actions.connectNote, "Insert a wiki link into this note"),
      button("Update links", actions.refresh, "Refresh the graph from the note draft")));
    if (actions.linkedValues.length) {
      const board = section("Linked settings");
      for (const target of actions.linkedValues) {
        const row = el("div", { className: "tune-linked" }, muted("Loading " + target.label + "…"));
        board.append(row);
        ctx.load(target.path).then(other => {
          const review = button("Review file", () => ctx.review(other), target.path);
          const status = () => { review.disabled = !ctx.dirty(other); };
          const edit = tuningControl(ctx, other, target, status);
          status();
          row.replaceChildren(el("small", { className: "muted" }, target.path), edit, review);
        }).catch(error => { row.replaceChildren(muted(target.label + " · " + error.message)); });
      }
      content.prepend(board);
    }
  } else if (doc.kind === "asset") {
    content.append(section("Asset", muted(doc.size.toLocaleString() + " bytes"),
      button(ctx.editableSprite(doc.path) ? "Open sprite editor" : "Open asset", actions.openFile)));
  } else {
    if (actions.previewRecipe) {
      content.append(section("Recipe tools", button("New recipe", actions.create),
        button("Playtest recipe draft", actions.previewRecipe, "Launch the existing panel build with this recipe draft in a separate save folder")));
    }
    if (spec.kind === "rust" || resolved?.node.raw !== undefined) {
      content.append(section("Setting", tuningControl(ctx, doc, spec, updateStatus)));
    } else if (resolved?.node.children) {
      const group = section("Fields", colorControl(ctx, doc, spec.parts, resolved.node, updateStatus));
      let shown = 0;
      resolved.node.children.forEach((child, index) => {
        if (child.raw === undefined || child.key === undefined || shown >= 12) return;
        const trail = [...resolved.trail, index];
        const target = { kind: "field", path: doc.path, parts: address(ctx.tree(doc), trail),
          label: child.key ?? title(child, index) };
        group.append(tuningControl(ctx, doc, target, updateStatus));
        shown++;
      });
      if (shown || group.children.length > 1) content.append(group);
    }

    const structured = ["ron", "json"].includes(doc.kind);
    const actionsRow = el("div", { className: "map-inspector-actions" },
      button(ctx.editableSprite(doc.path) ? "Open sprite editor" : "Open in workspace", actions.openFile));
    for (const action of actions.coding ?? []) actionsRow.append(button(action.label, action.run));
    if (spec.kind === "rust" || (spec.parts?.length && stable(spec.parts))) {
      actionsRow.append(button("Copy note link", () => {
        const parts = spec.kind === "rust" ? ["rust:" + spec.name] : spec.parts;
        copyText(wikiLink(doc.path, parts, spec.label))
          .then(() => ctx.note("Note link copied")).catch(error => ctx.note(error.message, true));
      }));
    }
    content.append(actionsRow);
    const source = sourceEditor(ctx, doc, updateStatus);
    if (structured || spec.kind === "rust" || actions.hasTunables) {
      content.append(el("details", { className: "map-source-details" }, el("summary", {}, "Source"), source));
    } else {
      content.append(section("Source", source));
    }
  }
  return el("aside", { className: "map-inspector" }, header, content, footer);
}

