import { parse } from "./syntax.mjs";
import { isSystemBoard, noteLinks, resolve, rustTunables, stable } from "./targets.mjs";

export function boardTargets(text) {
  if (!isSystemBoard(text)) return [];
  return noteLinks(text).filter(link => stable(link.parts)).slice(0, 12).map(({ path, parts }) =>
    parts[0]?.startsWith("rust:") ? { path, name: parts[0].slice(5) } : { path, parts });
}

export function focusLine(doc, target) {
  let start;
  if (target.name && doc.path.endsWith(".rs")) {
    start = rustTunables(doc.text).find(row => row.name === target.name)?.start;
  } else if (target.parts?.length && ["ron", "json"].includes(doc.kind)) {
    start = resolve(parse(doc.text, doc.kind), target.parts)?.node.start;
  } else return target.line ?? 1;
  return start === undefined ? undefined : doc.text.slice(0, start).split("\n").length;
}

export function centeredExcerpt(text, line, limit) {
  const lines = text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  const index = line - 1;
  if (index < 0 || index >= lines.length) return { start: 0, text: "" };
  const leading = lines.slice(Math.max(0, index - 10), index).join("").slice(-Math.floor(limit / 3));
  const start = lines.slice(0, index).reduce((size, row) => size + row.length, 0) - leading.length;
  return { start, text: (leading + lines.slice(index, index + 30).join("")).slice(0, limit) };
}
