import { decode, title } from "./syntax.mjs";

export const noteRoot = "docs/axiom-notes/";
export const isNote = path => path.startsWith(noteRoot) && path.endsWith(".md");
export const systemBoardMarker = "<!-- axiom-system-board -->";
export const isSystemBoard = text => text.split(/\r?\n/, 3)[2] === systemBoardMarker;
export const slug = name => name.normalize("NFKD").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
export const displayKey = key => key.replace(/_/g, " ");

function identity(node) {
  if (!node.children) return undefined;
  const [first, second] = node.children;
  if (node.children.length === 2 && first?.key === undefined && first?.raw !== undefined
    && second?.key === undefined && second?.children) return "tuple:" + decode(first.raw);
  for (const key of ["id", "kind", "name", "display_name", "title"]) {
    const field = node.children.find(child => child.key === key && child.raw !== undefined);
    if (field) return key + ":" + decode(field.raw);
  }
  const batch = node.children.find(child => child.key === "batch_counts");
  const product = node.children.find(child => child.key === "a_into");
  // Native batch validation requires a unique product, independent of row order.
  if (batch?.prefix === "Some" && product?.prefix === "Some" && product.children?.length === 1
      && product.children[0].raw !== undefined) return "batch_product:" + decode(product.children[0].raw);
}

const selectors = new WeakMap();

export function selector(parent, child, index) {
  let keys = selectors.get(parent);
  if (!keys) {
    const counts = new Map();
    keys = parent.children.map(child => {
      const id = identity(child);
      const key = child.key !== undefined ? "k:" + child.key : id !== undefined ? "r:" + id : "";
      counts.set(key, (counts.get(key) ?? 0) + 1);
      return key;
    });
    keys = keys.map((key, index) => key && counts.get(key) === 1 ? key
      : (parent.opener === "[" || key ? "n:" : "t:") + index);
    selectors.set(parent, keys);
  }
  return keys[index];
}

export function address(root, trail) {
  let node = root;
  return trail.map(index => {
    const child = node.children[index];
    const part = selector(node, child, index);
    node = child;
    return part;
  });
}

export function resolve(root, parts) {
  let node = root;
  const trail = [];
  for (const part of parts) {
    if (!node.children) return null;
    let index = -1;
    if (/^[nt]:\d+$/.test(part)) {
      const candidate = Number(part.slice(2));
      const child = node.children[candidate];
      if (child && selector(node, child, candidate) === part) index = candidate;
    } else {
      for (let i = 0; i < node.children.length; i++) {
        if (selector(node, node.children[i], i) !== part) continue;
        if (index >= 0) return null;
        index = i;
      }
    }
    if (index < 0) return null;
    trail.push(index);
    node = node.children[index];
  }
  return { trail, node };
}

export const stable = parts => parts.every(part => !part.startsWith("n:"));

function rustCode(source) {
  const startOfLiteral = /\/\*|\/\/[^\r\n]*|(?:br|cr|r)(#*)\"|(?:b|c)?\"|b?'(?:[^'\\\r\n]|\\[^'\r\n]?)'/g;
  const chunks = [];
  let start = 0;
  for (let match; (match = startOfLiteral.exec(source));) {
    const from = match.index;
    let end = startOfLiteral.lastIndex;
    if (match[0] === "/*") {
      const delimiter = /\/\*|\*\//g;
      delimiter.lastIndex = end;
      let depth = 1;
      while (depth) {
        const next = delimiter.exec(source);
        if (!next) { end = source.length; break; }
        depth += next[0] === "/*" ? 1 : -1;
        end = delimiter.lastIndex;
      }
    } else if (match[1] !== undefined) {
      const close = '"' + match[1];
      const found = source.indexOf(close, end);
      end = found < 0 ? source.length : found + close.length;
    } else if (match[0].endsWith('"')) {
      while (end < source.length) {
        if (source[end++] === '"') break;
        if (source[end - 1] === "\\") end++;
      }
    }
    chunks.push(source.slice(start, from), source.slice(from, end).replace(/[^\r\n]/g, " "));
    startOfLiteral.lastIndex = start = end;
  }
  chunks.push(source.slice(start));
  return chunks.join("");
}

export function rustTunables(source) {
  const rows = [];
  const pattern = /^([ \t]*(?:pub(?:\([^\r\n)]*\))?\s+)?const\s+([A-Z][A-Z0-9_]*)\s*:\s*(bool|f32|f64|[iu](?:8|16|32|64|128|size))\s*=\s*)(true|false|[+-]?\d[\d_]*(?:\.[\d_]*)?(?:[eE][+-]?[\d_]+)?)([ \t]*;)/gm;
  const matches = [...rustCode(source).matchAll(pattern)];
  const counts = new Map();
  for (const match of matches) counts.set(match[2], (counts.get(match[2]) ?? 0) + 1);
  for (const match of matches) {
    const [, prefix, name, type, raw] = match;
    if (/(?:^|_)VERSION(?:_|$)/.test(name) || counts.get(name) !== 1) continue;
    const start = match.index + prefix.length;
    const notes = source.slice(Math.max(0, match.index - 600), match.index).match(/(?:[ \t]*\/\/[^\r\n]*\r?\n){1,4}$/)?.[0]
      ?.replace(/^[ \t]*\/\/[/!]?\s?/gm, "").trim();
    rows.push({ name, type, raw, start, end: start + raw.length, notes });
  }
  return rows;
}

export function wikiLink(path, parts, label) {
  const anchor = parts?.length ? "#" + parts.map(encodeURIComponent).join("/") : "";
  return "[[" + path + anchor + (label ? "|" + label.replace(/[\[\]\r\n|]/g, " ") : "") + "]]";
}

export function noteLinks(text) {
  return [...text.matchAll(/\[\[([^\]|\r\n]+)(?:\|([^\]\r\n]+))?\]\]/g)].slice(0, 160).map(match => {
    const [reference, anchor] = match[1].trim().split("#", 2);
    const path = reference.includes("/") || /\.md$/i.test(reference)
      ? reference : noteRoot + slug(reference) + ".md";
    let parts = [];
    try { if (anchor) parts = anchor.split("/").map(decodeURIComponent); }
    catch { parts = ["invalid"]; }
    return { path, parts, label: match[2]?.trim() || reference };
  });
}

export function targetRows(root, path) {
  const result = [];
  const file = path.split("/").at(-1).replace(/\.(ron|json)$/, "");
  function visit(node, parts, context) {
    if (node.raw !== undefined) {
      if (node.key === undefined) return;
      result.push({ path, parts, kind: "field", label: displayKey(node.key),
        context: context.join(" › ") || file, value: decode(node.raw).slice(0, 96), type: node.kind });
      return;
    }
    node.children.forEach((child, index) => {
      const part = selector(node, child, index);
      const chain = [...parts, part];
      const record = part.startsWith("r:");
      const label = displayKey(title(child, index));
      if (record) result.push({ path, parts: chain, kind: "record", label, context: file, value: part.slice(2) });
      visit(child, chain, child.children && (record || child.key !== undefined)
        ? [...context, label] : context);
    });
  }
  visit(root, [], []);
  return result;
}

