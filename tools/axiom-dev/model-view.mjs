export const add = (a, b) => a.map((value, i) => value + b[i]);
export const subtract = (a, b) => a.map((value, i) => value - b[i]);
export const mul = (a, factor) => a.map(value => value * factor);
export const dot = (a, b) => a.reduce((total, value, i) => total + value * b[i], 0);
export const cross = (a, b) => [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
export const unit = a => mul(a, 1 / (Math.hypot(...a) || 1));
export const radians = value => value * Math.PI / 180;
export const degrees = value => value * 180 / Math.PI;
export const rotateX = ([x, y, z], a) => [x, y*Math.cos(a)-z*Math.sin(a), y*Math.sin(a)+z*Math.cos(a)];
export const rotateY = ([x, y, z], a) => [x*Math.cos(a)+z*Math.sin(a), y, -x*Math.sin(a)+z*Math.cos(a)];
export const rotateZ = ([x, y, z], a) => [x*Math.cos(a)-y*Math.sin(a), x*Math.sin(a)+y*Math.cos(a), z];

const normals = [[1,0,0],[-1,0,0],[0,1,0],[0,-1,0],[0,0,1],[0,0,-1]];
const faceCorners = [
  [[1,0,0],[1,1,0],[1,1,1],[1,0,1]], [[0,0,1],[0,1,1],[0,1,0],[0,0,0]],
  [[0,1,0],[0,1,1],[1,1,1],[1,1,0]], [[0,0,1],[0,0,0],[1,0,0],[1,0,1]],
  [[1,0,1],[1,1,1],[0,1,1],[0,0,1]], [[0,0,0],[0,1,0],[1,1,0],[1,0,0]],
];
export const box = (at, size, color, transform = point => point) => faceCorners.map(corners => ({
  color, points: corners.map(point => transform(add(at, point.map((value, i) => value * size[i])))),
}));

export function voxelFaces(model) {
  const faces = [];
  for (const [key, ink] of model.cells) {
    const point = key.split(",").map(Number);
    const material = model.palette.get(ink);
    if (!material) continue;
    const color = material.hex.match(/../g).map(value => Number.parseInt(value, 16));
    for (let i = 0; i < normals.length; i++) {
      if (model.cells.has(add(point, normals[i]).join(","))) continue;
      faces.push({ color, points: faceCorners[i].map(corner => add(corner, point)) });
    }
  }
  return faces;
}

export function paintModel(canvas, faces, { transform, project, cull = true, axes } = {}) {
  const g = canvas.getContext("2d");
  const w = canvas.width, h = canvas.height;
  const gradient = g.createRadialGradient(w*.52, h*.47, 12, w*.52, h*.47, w*.55);
  gradient.addColorStop(0, "#24354b"); gradient.addColorStop(1, "#0c1420");
  g.fillStyle = gradient;
  g.fillRect(0, 0, w, h);
  g.strokeStyle = "#6e8ba333"; g.beginPath();
  g.moveTo(w/2-14, h/2); g.lineTo(w/2+14, h/2); g.moveTo(w/2, h/2-14); g.lineTo(w/2, h/2+14); g.stroke();
  const ordered = [];
  for (const face of faces) {
    const points = face.points.map(transform);
    const normal = unit(cross(subtract(points[1], points[0]), subtract(points[2], points[0])));
    if (cull && normal[2] > .001) continue;
    const screen = points.map(project);
    if (screen.some(point => !point || !point.every(Number.isFinite))) continue;
    const shade = .62 + Math.abs(dot(normal, unit([-.45,.68,-.8]))) * .38;
    ordered.push({ points: screen, depth: points.reduce((sum, point) => sum + point[2], 0) / points.length,
      fill: "rgb(" + face.color.map(value => Math.round(value * shade)).join(",") + ")" });
  }
  ordered.sort((a, b) => b.depth - a.depth);
  for (const face of ordered) {
    g.beginPath();
    face.points.forEach((point, index) => index ? g.lineTo(...point) : g.moveTo(...point));
    g.closePath();
    g.fillStyle = g.strokeStyle = face.fill;
    g.lineWidth = .55;
    g.fill(); g.stroke();
  }
  for (const [from, to, label, color] of axes ?? []) {
    const a = project(from), b = project(to);
    if (!a || !b) continue;
    g.beginPath(); g.moveTo(...a); g.lineTo(...b);
    g.strokeStyle = color; g.lineWidth = 2; g.stroke();
    g.fillStyle = color; g.font = "bold 13px system-ui"; g.fillText(label, b[0]+4, b[1]-4);
  }
}

export function orbitModel(canvas, getFaces, getFrame = () => null) {
  let yaw = -.67, pitch = -.48, zoom = 1, drag, pending;
  function render() {
    pending = undefined;
    const faces = getFaces();
    const frame = getFrame();
    const center = frame?.center ?? [0,0,0];
    const scale = Math.min(canvas.width, canvas.height) * .31 * zoom / (frame?.radius || 1);
    const transform = point => rotateX(rotateY(subtract(point, center), yaw), pitch);
    const project = point => [canvas.width/2 + point[0]*scale, canvas.height/2 - point[1]*scale];
    const origin = transform(center);
    const axis = (direction, label, color) => [origin, transform(add(center, mul(direction, (frame?.radius || 1)*.45))), label, color];
    paintModel(canvas, faces, { transform, project, cull: frame?.cull ?? true,
      axes: [axis([1,0,0],"X","#f1a2a3"), axis([0,1,0],"Y","#a0e1bd"), axis([0,0,1],"Z","#9fc8fb")] });
  }
  const repaint = () => { pending ??= requestAnimationFrame(render); };
  canvas.onpointerdown = event => {
    if (event.button !== 0) return;
    drag = [event.clientX, event.clientY];
    canvas.setPointerCapture(event.pointerId);
  };
  canvas.onpointermove = event => {
    if (!drag) return;
    yaw += (event.clientX-drag[0])*.012;
    pitch = Math.max(-1.5, Math.min(1.5, pitch+(event.clientY-drag[1])*.012));
    drag = [event.clientX, event.clientY]; repaint();
  };
  canvas.onlostpointercapture = () => { drag = undefined; };
  canvas.addEventListener("wheel", event => {
    event.preventDefault();
    zoom = Math.max(.4, Math.min(3, zoom * Math.exp(-event.deltaY*.0015)));
    repaint();
  }, { passive: false });
  repaint();
  return repaint;
}

function glbAccessor(json, bin, index) {
  const accessor = json.accessors[index], view = json.bufferViews[accessor.bufferView];
  if (accessor.sparse || view.buffer !== 0) throw new Error("Preview needs embedded, dense GLB geometry");
  const format = { 5121:[1,"getUint8"], 5123:[2,"getUint16"], 5125:[4,"getUint32"], 5126:[4,"getFloat32"] }[accessor.componentType];
  const width = { SCALAR:1, VEC2:2, VEC3:3, VEC4:4 }[accessor.type];
  if (!format || !width) throw new Error("Unsupported GLB vertex format");
  const [bytes, method] = format;
  const start = (view.byteOffset ?? 0) + (accessor.byteOffset ?? 0);
  const stride = view.byteStride ?? bytes*width;
  return { count: accessor.count, get: i => Array.from({ length: width }, (_, j) =>
    bin[method](start + i*stride + j*bytes, true)) };
}

export function glbFaces(buffer) {
  const data = new DataView(buffer);
  if (data.getUint32(0, true) !== 0x46546c67 || data.getUint32(4, true) !== 2) throw new Error("Expected GLB 2");
  const jsonLength = data.getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 20, jsonLength)));
  const bin = new DataView(buffer, 28 + jsonLength);
  const faces = [];
  for (const node of json.nodes ?? []) {
    if (node.mesh === undefined) continue;
    for (const primitive of json.meshes[node.mesh].primitives) {
      if (primitive.mode !== undefined && primitive.mode !== 4) continue;
      const positions = glbAccessor(json, bin, primitive.attributes.POSITION);
      const indices = primitive.indices === undefined ? null : glbAccessor(json, bin, primitive.indices);
      const length = indices?.count ?? positions.count;
      if (faces.length + length/3 > 24000) throw new Error("This mesh exceeds the lightweight preview limit");
      const material = json.materials?.[primitive.material];
      const name = material?.name?.toLowerCase() ?? "";
      const factor = material?.pbrMetallicRoughness?.baseColorFactor;
      const color = factor && factor.slice(0,3).some(value => value < .95) ? factor.slice(0,3).map(value => Math.round(value*255))
        : /cloth|sleeve/.test(name) ? [80, 91, 105] : /brass|copper/.test(name) ? [183, 131, 82]
        : /crystal|glass/.test(name) ? [124, 181, 180] : [164, 159, 151];
      for (let i = 0; i < length-2; i += 3) {
        const points = [i,i+1,i+2].map(index => positions.get(indices ? indices.get(index)[0] : index));
        if (points.every(point => point.every(Number.isFinite))) faces.push({ points, color });
      }
    }
  }
  if (!faces.length) throw new Error("No supported triangles in this model");
  return faces;
}

export function modelBounds(faces) {
  const min = [Infinity,Infinity,Infinity], max = [-Infinity,-Infinity,-Infinity];
  for (const face of faces) for (const point of face.points) point.forEach((value,i) => {
    min[i] = Math.min(min[i],value); max[i] = Math.max(max[i],value);
  });
  return { center: mul(add(min,max),.5), radius: Math.max(.01, ...subtract(max,min))*.65, cull: false };
}

