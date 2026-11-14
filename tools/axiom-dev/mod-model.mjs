import { rotateX, rotateY, rotateZ, radians } from "./model-view.mjs";

const rotations = [rotateX, rotateY, rotateZ];
const axes = ["x", "y", "z"];
const vector = value => Array.isArray(value) && value.length === 3
  && value.every(n => Number.isFinite(n) && Math.abs(n) <= 4096);
const colors = { metal: "8196a2", wood: "9b704c", stone: "636d79", glass: "6fabb2",
  plant: "69894e", light: "ddb866", trim: "b78554" };
const palette = "abcdef";

function material(texture) {
  for (const [pattern, name] of [
    [/glow|lamp|light|magic|energy|lit/, "light"], [/glass|crystal|water/, "glass"],
    [/wood|oak|plank|log/, "wood"], [/leaf|leaves|plant|grass/, "plant"],
    [/iron|steel|metal|copper|machine/, "metal"], [/stone|rock|brick/, "stone"],
  ]) if (pattern.test(texture)) return colors[name];
  return colors.trim;
}

function modelPath(ref, namespace) {
  if (typeof ref !== "string") return null;
  const [owner, resource] = ref.includes(":") ? ref.split(":", 2) : [namespace, ref];
  if (!/^[a-z0-9_.-]+$/.test(owner) || !/^[a-z0-9_./-]+$/.test(resource) ||
      resource.split("/").some(part => !part || part === "." || part === "..")) return null;
  return "assets/" + owner + "/models/" + resource + ".json";
}

function vanilla(model, path, read, notes, visited = new Set()) {
  if (visited.has(path) || visited.size >= 12) throw new Error("Model parent chain is cyclic or too deep");
  visited.add(path);
  const parentPath = modelPath(model.parent, path.split("/")[1]);
  const parent = parentPath && read(parentPath);
  const inherited = parent ? vanilla(parent, parentPath, read, notes, visited) : {};
  if (!parent && parentPath && !model.elements) {
    const type = parentPath.split("/").at(-1);
    if (/^(cube(?:_all|_bottom_top|_column|_column_horizontal)?|orientable(?:_vertical|_with_bottom)?)\.json$/.test(type)) {
      inherited.elements = [{ from: [0, 0, 0], to: [16, 16, 16] }];
      notes.add("Used the built-in cube template");
    } else notes.add("Unresolved parent " + parentPath);
  }
  return { elements: model.elements ?? inherited.elements, textures: { ...inherited.textures, ...model.textures } };
}

function textureName(textures, ref) {
  for (let depth = 0; depth < 8 && typeof ref === "string" && ref.startsWith("#"); depth++) ref = textures[ref.slice(1)];
  return String(ref ?? "");
}

function rotated(point, stack, inverse = false) {
  for (const part of inverse ? [...stack].reverse() : stack) {
    point = point.map((n, i) => n - part.pivot[i]);
    for (const axis of inverse ? [2, 1, 0] : [0, 1, 2]) {
      if (part.angles[axis]) point = rotations[axis](point, radians(part.angles[axis]) * (inverse ? -1 : 1));
    }
    point = point.map((n, i) => n + part.pivot[i]);
  }
  return point;
}

function inverseTransform(stack) {
  const origin = rotated([0, 0, 0], stack, true);
  const basis = [[1, 0, 0], [0, 1, 0], [0, 0, 1]].map(axis =>
    rotated(axis, stack, true).map((n, i) => n - origin[i]));
  return point => origin.map((n, i) => n + basis[0][i] * point[0] + basis[1][i] * point[1] + basis[2][i] * point[2]);
}

function readBoxes(root, path, read, notes) {
  if (Array.isArray(root["minecraft:geometry"])) {
    const bones = root["minecraft:geometry"][0]?.bones ?? [];
    if (!Array.isArray(bones) || bones.length > 128) throw new Error("Geometry has too many bones");
    const byName = new Map(bones.map(bone => [bone.name, bone]));
    return bones.flatMap(bone => {
      const transforms = [];
      const seen = new Set();
      for (let part = bone; part && !seen.has(part) && transforms.length < 24; part = byName.get(part.parent)) {
        seen.add(part);
        transforms.push({ pivot: vector(part.pivot) ? part.pivot : [0, 0, 0],
          angles: vector(part.rotation) ? part.rotation : [0, 0, 0] });
      }
      if (!Array.isArray(bone.cubes)) return [];
      if (bone.cubes.length > 128) throw new Error("Geometry has too many cubes on one bone");
      return bone.cubes.map(cube => {
        const { origin, size } = cube;
        if (!vector(origin) || !vector(size)) return null;
        const inflate = Number.isFinite(cube.inflate) ? Math.max(-32, Math.min(32, cube.inflate)) : 0;
        return { from: origin.map(n => n - inflate), to: origin.map((n, i) => n + size[i] + inflate),
          color: Object.values(colors)[transforms.length % Object.keys(colors).length],
          stack: [{ pivot: vector(cube.pivot) ? cube.pivot : origin,
            angles: vector(cube.rotation) ? cube.rotation : [0, 0, 0] }, ...transforms] };
      });
    }).filter(Boolean);
  }
  const { elements, textures } = vanilla(root, path, read, notes);
  if (!Array.isArray(elements) || elements.length > 256) throw new Error("Expected up to 256 cuboid elements or GeckoLib geometry");
  return elements.flatMap(element => {
    if (!vector(element.from) || !vector(element.to)) return [];
    const axis = axes.indexOf(element.rotation?.axis);
    const pivot = vector(element.rotation?.origin) ? element.rotation.origin : [8, 8, 8];
    const angles = axes.map((_, i) => i === axis && Number.isFinite(element.rotation?.angle) ? element.rotation.angle : 0);
    const ref = element.faces?.up?.texture ?? Object.values(element.faces ?? {})[0]?.texture
      ?? Object.values(textures)[0];
    return [{ from: element.from, to: element.to, color: material(textureName(textures, ref)), stack: [{ pivot, angles }] }];
  });
}

export function voxelPrototype(root, path, read, size = 8) {
  if (size !== 8 && size !== 16) throw new Error("Choose an 8 or 16 voxel grid");
  const notes = new Set(["Static silhouette with approximate materials. UVs, animations and runtime transforms require separate work."]);
  const boxes = readBoxes(root, path, read, notes);
  if (!boxes.length || boxes.length > 256) throw new Error("Model needs 1–256 usable cuboids");
  const low = [Infinity, Infinity, Infinity], high = low.map(n => -n);
  for (const { from, to, stack } of boxes) {
    for (const x of [from[0], to[0]]) for (const y of [from[1], to[1]]) for (const z of [from[2], to[2]]) {
      rotated([x, y, z], stack).forEach((n, i) => { low[i] = Math.min(low[i], n); high[i] = Math.max(high[i], n); });
    }
  }
  const step = Math.max(...high.map((n, i) => n - low[i])) / size;
  if (!(step > 0)) throw new Error("Model has no spatial extent");
  const start = low.map((n, i) => i === 1 ? n : (n + high[i] - size * step) / 2);
  const rows = Array.from({ length: size }, () => Array.from({ length: size }, () => Array(size).fill(".")));
  const swatches = new Map();
  let occupied = 0;
  for (const box of boxes) {
    const inverse = inverseTransform(box.stack);
    if (!swatches.has(box.color)) swatches.set(box.color, (palette + "g")[swatches.size]);
    const ink = swatches.get(box.color);
    for (let y = 0; y < size; y++) for (let z = 0; z < size; z++) for (let x = 0; x < size; x++) {
      const sample = [x, y, z].map((n, i) => start[i] + (n + 0.5) * step);
      const point = inverse(sample);
      if (!point.every((n, i) => {
        const a = Math.min(box.from[i], box.to[i]), b = Math.max(box.from[i], box.to[i]);
        const pad = Math.max(0, (step - (b - a)) / 2);
        return n >= a - pad && n < b + pad;
      })) continue;
      occupied += Number(rows[y][z][x] === ".");
      rows[y][z][x] = ink;
    }
  }
  if (!occupied) throw new Error("No occupied cells at this resolution");
  const text = [`size ${size} ${size} ${size}`, ...[...swatches].map(([color, key]) => `p ${key} ${color} 0.0`),
    ...rows.flatMap((layer, y) => layer.some(row => row.some(ink => ink !== "."))
      ? ["y " + y, ...layer.map(row => row.join(""))] : [])].join("\n") + "\n";
  return { text, boxes: boxes.length, occupied, notes: [...notes] };
}
