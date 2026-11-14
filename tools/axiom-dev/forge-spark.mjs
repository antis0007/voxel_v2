import { parse, patch } from "./syntax.mjs";

export { spellPath } from "./spell-wiring.mjs";
const number = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i;

export function sparkFields(text) {
  const root = parse(text, "ron");
  const rows = root.children?.filter(row => row.children?.some(node => node.key === "id" && node.raw === "0"));
  const row = rows?.length === 1 ? rows[0] : null;
  const find = key => row?.children.find(node => node.key === key);
  const speed = find("speed");
  if (root.opener !== "[" || find("name")?.raw !== '"Spark"' || find("class")?.raw !== "Projectile"
    || !speed || !number.test(speed.raw)) throw new Error("Could not identify the Spark projectile definition");
  return { speed, cost: find("cost")?.raw, damage: find("damage")?.raw, cooldown: find("cooldown")?.raw };
}

export function checkedSpeed(value) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 1 || value > 120) {
    throw new Error("Spark speed must be between 1 and 120 m/s for this fixture.");
  }
  return value;
}

export function editSparkSpeed(text, value) {
  const { speed } = sparkFields(text);
  checkedSpeed(value);
  if (Number(speed.raw) === value) return text;
  const replacement = Number.isInteger(value) ? value.toFixed(1) : String(value);
  return patch(text, speed.start, speed.end, replacement);
}

export function sparkPreview(source, { version, speed }) {
  if (source.version !== version) throw new Error("Spell table changed on disk. Reload it before previewing.");
  sparkFields(source.text);
  return { version, speed: checkedSpeed(speed) };
}
