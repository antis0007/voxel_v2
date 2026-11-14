const pairs = { "(": ")", "[": "]", "{": "}" };
const word = /(?:r#)?[\p{L}_][\p{L}\p{N}_]*/uy;
const number = /[+-]?(?:0[xX][a-fA-F0-9_]+|0[bB][01_]+|0[oO][0-7_]+|(?:[0-9][0-9_]*(?:\.[0-9_]*)?|\.[0-9][0-9_]*)(?:[eE][+-]?[0-9][0-9_]*)?)(?:[uif](?:8|16|32|64|128|size))?/y;

export function parse(source, format) {
  if (format === "json") JSON.parse(source);
  let pos = 0;
  let token;
  let count = 0;

  function error(message, offset = token.start) {
    const preceding = source.slice(0, offset);
    throw new Error(message + " at line " + preceding.split("\n").length
      + ", column " + (offset - preceding.lastIndexOf("\n")));
  }

  function next() {
    while (pos < source.length) {
      if (/\s/.test(source[pos])) { pos++; continue; }
      if (source.startsWith("//", pos)) {
        const end = source.indexOf("\n", pos);
        pos = end < 0 ? source.length : end + 1;
        continue;
      }
      if (source.startsWith("/*", pos)) {
        const start = pos;
        let depth = 1;
        pos += 2;
        while (depth && pos < source.length) {
          if (source.startsWith("/*", pos)) { depth++; pos += 2; }
          else if (source.startsWith("*/", pos)) { depth--; pos += 2; }
          else pos++;
        }
        if (depth) error("Unclosed comment", start);
        continue;
      }
      break;
    }

    const start = pos;
    const c = source[pos];
    if (c === undefined) return token = { kind: "end", start, end: pos, raw: "" };
    let kind;
    if ("()[]{}:,#!".includes(c)) {
      kind = c;
      pos++;
    } else {
      const rawString = /(?:br|r)(#*)"/y;
      rawString.lastIndex = pos;
      const raw = rawString.exec(source);
      if (raw) {
        const closer = '"' + raw[1];
        const end = source.indexOf(closer, rawString.lastIndex);
        if (end < 0) error("Unclosed raw string", start);
        pos = end + closer.length;
        kind = raw[0].startsWith("b") ? "literal" : "string";
      } else if (c === '"' || c === "'" || (c === "b" && /['"]/.test(source[pos + 1] ?? ""))) {
        const quote = c === "b" ? source[++pos] : c;
        pos++;
        while (pos < source.length && source[pos] !== quote) pos += source[pos] === "\\" ? 2 : 1;
        if (source[pos] !== quote) error("Unclosed string or character", start);
        pos++;
        kind = c === '"' ? "string" : "literal";
      } else {
        word.lastIndex = number.lastIndex = pos;
        const match = number.exec(source) ?? word.exec(source);
        if (!match && !source.startsWith("-inf", pos) && !source.startsWith("+inf", pos)) {
          error("Unexpected character " + JSON.stringify(c), start);
        }
        pos += match?.[0].length ?? 4;
        kind = number.lastIndex > start ? "number" : "symbol";
      }
    }
    return token = { kind, raw: source.slice(start, pos), start, end: pos };
  }

  function expect(kind) {
    if (token.kind !== kind) error("Expected " + kind);
    const found = token;
    next();
    return found;
  }

  function value(depth) {
    if (++count > 150000 || depth > 96) error("Document exceeds the visual editor limit");
    const first = token;
    if (!pairs[first.kind] && !["number", "symbol", "literal", "string"].includes(first.kind)) error("Expected a value");
    next();
    const prefix = first.kind === "symbol" && token.kind === "(" ? first.raw : "";
    const opener = prefix ? token : first;
    if (prefix) next();
    if (!pairs[opener.kind]) {
      return { ...first, kind: ["true", "false"].includes(first.raw) ? "boolean" : first.kind };
    }
    const node = { kind: "group", start: first.start, prefix, opener: opener.kind, open: opener.end, children: [] };
    const closer = pairs[opener.kind];
    while (token.kind !== closer) {
      const entry = value(depth + 1);
      let child = entry;
      if (token.kind === ":") {
        if (opener.kind === "[" || (opener.kind === "(" && !["symbol", "string"].includes(entry.kind))) {
          error("Unexpected field or map key");
        }
        next();
        child = value(depth + 1);
        child.key = entry.raw === undefined ? source.slice(entry.start, entry.end) : decode(entry.raw);
        child.keyStart = entry.start;
      }
      child.entry = entry.start;
      node.children.push(child);
      if (token.kind !== ",") break;
      child.comma = expect(",").start;
    }
    const close = expect(closer);
    node.close = close.start;
    node.end = close.end;
    return node;
  }

  next();
  while (token.kind === "#") {
    next();
    expect("!");
    expect("[");
    let depth = 1;
    while (depth && token.kind !== "end") {
      if (token.kind === "[") depth++;
      if (token.kind === "]") depth--;
      next();
    }
    if (depth) error("Unclosed attribute");
  }
  const root = value(0);
  expect("end");
  return root;
}

export function decode(raw) {
  const match = /^r(#*)"/.exec(raw);
  if (match) return raw.slice(match[0].length, -(match[1].length + 1));
  if (!raw.startsWith('"')) return raw.replace(/^r#/, "");
  try { return JSON.parse(raw); } catch {
    return raw.slice(1, -1).replace(/\\(?:u\{([0-9a-f]+)\}|x([0-9a-f]{2})|(['"\\0]))/gi, (escape, unicode, byte, simple) => {
      if (simple) return simple === "0" ? "\0" : simple;
      const code = Number.parseInt(unicode ?? byte, 16);
      return code <= 0x10ffff ? String.fromCodePoint(code) : escape;
    });
  }
}

export function quoted(value, format) {
  const json = JSON.stringify(value);
  return format === "json" ? json : json.replace(/\\\\|\\u([0-9a-f]{4})/g,
    (escape, digits) => digits ? "\\u{" + digits + "}" : escape);
}

export function title(node, index) {
  if (node.key !== undefined) return node.key;
  if (!node.children) return decode(node.raw).slice(0, 80) || '""';
  const record = node.children[1]?.children && !node.children[0]?.children ? node.children[1] : node;
  const label = ["name", "display_name", "title", "id", "kind", "key", "species", "why"]
    .map(key => record.children.find(child => child.key === key && child.raw)).find(Boolean);
  return label ? decode(label.raw).slice(0, 80) : node.prefix || "[" + index + "]";
}

export function at(root, path) {
  return path.reduce((node, index) => node?.children?.[index], root);
}

export function patch(source, start, end, replacement) {
  return source.slice(0, start) + replacement + source.slice(end);
}

export function deleteEntry(source, parent, index) {
  const node = parent.children[index];
  if (node.comma !== undefined) {
    return patch(patch(source, node.comma, node.comma + 1, ""), node.entry, node.end, "");
  }
  const prior = parent.children[index - 1]?.comma;
  const removed = patch(source, node.entry, node.end, "");
  return prior === undefined ? removed : patch(removed, prior, prior + 1, "");
}

export function addEntry(source, parent, entry) {
  const last = parent.children.at(-1);
  const multi = source.slice(parent.open, parent.close).includes("\n");
  const indentAt = offset => /^[ \t]*/.exec(source.slice(source.lastIndexOf("\n", offset - 1) + 1))[0];
  const indent = parent.children.length ? indentAt(parent.children[0].entry) : indentAt(parent.close) + "    ";
  const formatted = multi ? "\n" + indent + entry : (last ? " " : "") + entry;
  if (!last) return patch(source, parent.open, parent.open, formatted);
  if (last.comma !== undefined) return patch(source, last.comma + 1, last.comma + 1, formatted + ",");
  return patch(source, last.end, last.end, "," + formatted);
}

