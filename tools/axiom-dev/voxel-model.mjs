export function parseVoxel(text) {
  const newline = text.includes("\r\n") ? "\r\n" : "\n";
  const lines = text.split(/\r?\n/);
  const palette = new Map(), layers = new Map(), cells = new Map();
  let size, layer, z = 0;
  lines.forEach((raw, index) => {
    const line = raw.trim();
    if (!line || line.startsWith("#")) return;
    const [op, a, b, c] = line.split(/\s+/);
    const fail = () => { throw new Error("Invalid voxel model at line " + (index + 1)); };
    if (op === "size") {
      if (size) fail();
      size = [a,b,c].map(Number);
      if (size.some(n => !Number.isInteger(n) || n < 1 || n > 32)) fail();
    } else if (op === "p") {
      if (!/^[!-~]$/.test(a) || a === "." || a === "#" || palette.has(a)
        || !/^[0-9a-f]{6}$/i.test(b) || !Number.isFinite(Number(c))) fail();
      palette.set(a, { hex: b, glow: Number(c), index });
    } else if (op === "y") {
      layer = Number(a);
      if (!size || !Number.isInteger(layer) || layer < 0 || layer >= size[1] || layers.has(layer)) fail();
      layers.set(layer, []);
      z = 0;
    } else {
      if (!size || layer === undefined || line.length !== size[0] || z >= size[2]) fail();
      layers.get(layer).push(index);
      [...line].forEach((key, x) => {
        if (key === ".") return;
        if (!palette.has(key)) fail();
        cells.set([x,layer,z].join(","), key);
      });
      z++;
    }
  });
  if (!size || !palette.size) throw new Error("Expected a native text voxel model");
  for (const rows of layers.values()) if (rows.length !== size[2]) throw new Error("Voxel layer row count does not match its depth");
  return { lines, newline, size, palette, layers, cells };
}

export function setVoxel(model, x, y, z, color) {
  const rows = model.layers.get(y);
  const lines = model.lines.slice();
  if (rows) {
    const row = lines[rows[z]];
    const start = row.length - row.trimStart().length;
    lines[rows[z]] = row.slice(0, start+x) + color + row.slice(start+x+1);
  } else if (color !== ".") {
    const empty = ".".repeat(model.size[0]);
    const layer = Array(model.size[2]).fill(empty);
    layer[z] = empty.slice(0, x) + color + empty.slice(x+1);
    let last = lines.length;
    while (last && !lines[last-1].trim()) last--;
    lines.splice(last, 0, "y " + y, ...layer);
  }
  return lines.join(model.newline);
}

export function changeVoxelPalette(model, key, hex, glow) {
  const lines = model.lines.slice();
  const entry = model.palette.get(key);
  const line = "p " + key + " " + hex + " " + glow;
  if (entry) lines[entry.index] = line;
  else {
    const first = lines.findIndex(line => /^\s*y\s+/.test(line));
    if (first >= 0) lines.splice(first, 0, line);
    else lines.push(line);
  }
  return lines.join(model.newline);
}

