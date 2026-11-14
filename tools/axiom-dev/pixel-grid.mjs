export function line(x, y, endX, endY, draw) {
  const dx = Math.abs(endX - x);
  const dy = -Math.abs(endY - y);
  const sx = x < endX ? 1 : -1;
  const sy = y < endY ? 1 : -1;
  let error = dx + dy;
  for (;;) {
    draw(x, y);
    if (x === endX && y === endY) break;
    const doubled = 2 * error;
    if (doubled >= dy) { error += dy; x += sx; }
    if (doubled <= dx) { error += dx; y += sy; }
  }
}

export function fill(pixels, width, height, x, y, ink, allowed = () => true) {
  const start = y * width + x;
  const color = pixels[start];
  if (color === ink || !allowed(start)) return;
  const queue = [start];
  pixels[start] = ink;
  const add = i => {
    if (pixels[i] === color && allowed(i)) { pixels[i] = ink; queue.push(i); }
  };
  for (let head = 0; head < queue.length; head++) {
    const i = queue[head];
    if (i % width) add(i - 1);
    if (i % width < width - 1) add(i + 1);
    if (i >= width) add(i - width);
    if (i < (height - 1) * width) add(i + width);
  }
}

export function readTile(bitmap, rect) {
  const { x, y, width, height } = rect;
  const pixels = new Uint32Array(width * height);
  for (let row = 0; row < height; row++) {
    const from = (y + row) * bitmap.width + x;
    pixels.set(bitmap.pixels.subarray(from, from + width), row * width);
  }
  return pixels;
}

export function writeTile(bitmap, rect, pixels) {
  const { x, y, width, height } = rect;
  for (let row = 0; row < height; row++) {
    bitmap.pixels.set(pixels.subarray(row * width, (row + 1) * width), (y + row) * bitmap.width + x);
  }
}

export function canvasPixels(canvas, pixels, width, height, resolve = value => value) {
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  const data = context.createImageData(width, height);
  for (let index = 0; index < pixels.length; index++) {
    const color = resolve(pixels[index]);
    const to = index * 4;
    data.data[to] = color >>> 24;
    data.data[to + 1] = (color >>> 16) & 255;
    data.data[to + 2] = (color >>> 8) & 255;
    data.data[to + 3] = color & 255;
  }
  context.putImageData(data, 0, 0);
}

export function pngChanges(bitmap) {
  let count = 0;
  for (let i = 0; i < bitmap.pixels.length; i++) {
    if (bitmap.pixels[i] !== bitmap.original[i]) count++;
  }
  return count;
}

export function changedRegion(bitmap) {
  const { width, height, pixels, original } = bitmap;
  let left = width, top = height, right = -1, bottom = -1;
  for (let index = 0; index < pixels.length; index++) {
    if (pixels[index] === original[index]) continue;
    const x = index % width;
    const y = Math.floor(index / width);
    left = Math.min(left, x);
    right = Math.max(right, x);
    top = Math.min(top, y);
    bottom = Math.max(bottom, y);
  }
  const x = Math.max(0, left - 2), y = Math.max(0, top - 2);
  return { x, y, width: Math.min(width, right + 3) - x, height: Math.min(height, bottom + 3) - y };
}

export function pngDimensions(view) {
  if (view.byteLength < 33 || view.getUint32(0) !== 0x89504e47 || view.getUint32(4) !== 0x0d0a1a0a
    || view.getUint32(8) !== 13 || view.getUint32(12) !== 0x49484452) {
    throw new Error("This file is not a PNG");
  }
  const width = view.getUint32(16);
  const height = view.getUint32(20);
  if (!width || !height || width > 2048 || height > 2048 || width * height > 1_048_576) {
    throw new Error("Sprite editing supports up to 2,048 px per side and 1,048,576 pixels total");
  }
  if (view.getUint8(24) === 16) throw new Error("16-bit PNGs can be replaced from Asset preview");
  for (let pos = 8; pos + 12 <= view.byteLength;) {
    const type = view.getUint32(pos + 4);
    if (type === 0x6163544c) throw new Error("Animated PNGs can be replaced from Asset preview");
    const next = pos + view.getUint32(pos) + 12;
    if (next > view.byteLength) break;
    if (type === 0x49444154) return { width, height };
    pos = next;
  }
  throw new Error("PNG image data is missing or truncated");
}

export async function loadPng(blob) {
  const { width, height } = pngDimensions(new DataView(await blob.arrayBuffer()));
  const bitmap = await createImageBitmap(blob, { colorSpaceConversion: "none" });
  try {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(0, 0, width, height);
    const pixels = new Uint32Array(width * height);
    for (let i = 0; i < pixels.length; i++) {
      const j = i * 4;
      pixels[i] = ((data[j] << 24) | (data[j + 1] << 16) | (data[j + 2] << 8) | data[j + 3]) >>> 0;
    }
    return { width, height, pixels, original: pixels.slice(), changes: 0, undo: [], redo: [] };
  } finally { bitmap.close(); }
}

export async function encodePng(bitmap) {
  const canvas = document.createElement("canvas");
  canvasPixels(canvas, bitmap.pixels, bitmap.width, bitmap.height);
  const blob = await new Promise((resolve, reject) => canvas.toBlob(value =>
    value ? resolve(value) : reject(new Error("PNG encoding failed")), "image/png"));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result.split(",")[1]);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

export function stepPng(bitmap, from, to) {
  const entry = bitmap[from].pop();
  if (!entry) return;
  bitmap[to].push({ rect: entry.rect, pixels: readTile(bitmap, entry.rect) });
  writeTile(bitmap, entry.rect, entry.pixels);
  bitmap.changes = pngChanges(bitmap);
}

