import { el, button } from "./inspector.mjs";
import { patch } from "./syntax.mjs";
import { field } from "./idea-data.mjs";
import { wandPosePath } from "./catalog-data.mjs";
import { add, subtract, mul, dot, cross, unit, degrees, radians, rotateX, rotateY, rotateZ, box, paintModel } from "./model-view.mjs";
import { loadMesh } from "./model-studio.mjs";

const float = value => {
  const text = value.toFixed(6).replace(/0+$/, "");
  return text.endsWith(".") ? text + "0" : text;
};
const vectors = nodes => nodes.children.map(node => Number(node.raw));
const ronscalar = node => ({ start: node.start, end: node.end, value: Number(node.raw) });
const ronvector = node => ({ start: node.start, end: node.end, value: vectors(node) });
const ref = (text, pattern) => {
  const found = pattern.exec(text);
  if (!found) throw new Error("This placement moved in the source. Open the file to inspect it.");
  const start = found.index + found[1].length;
  const value = found[2].includes(",") ? found[2].split(",").map(Number) : Number(found[2]);
  if (!(Array.isArray(value) ? value : [value]).every(Number.isFinite)) throw new Error("Unsupported placement expression");
  return { start, end: start + found[2].length, value };
};
const constant = (text, name, vector = false) => ref(text, vector
  ? new RegExp("(^const " + name + ": Vec3 = Vec3::new\\()([^()]+)\\);", "m")
  : new RegExp("(^const " + name + ": f32 = )([-+\\d.]+);", "m"));

function torch(text, axis) {
  return ref(text, new RegExp("(fn torch_tilt\\(\\) -> Quat \\{[^}]*?from_rotation_" + axis + "\\()([-+\\d.]+)\\)"));
}
const blade = (text, axis) => ref(text,
  new RegExp("(fn blade_rest\\(\\) -> Quat \\{[^}]*?from_rotation_" + axis + "\\()([-+\\d.]+)\\)"));
const encode = (value, outer = false) => Array.isArray(value)
  ? (outer ? "(" : "") + value.map(float).join(", ") + (outer ? ")" : "") : float(value);

function wandSettings(ctx, doc, pose) {
  const root = ctx.tree(doc);
  const key = field(root, pose);
  const offset = ronvector(field(key, "offset"));
  const axis = ronvector(field(key, "axis"));
  const roll = ronscalar(field(key, "roll"));
  const scale = ronscalar(field(root, "scale"));
  const fov = ronscalar(field(root, "fov_degrees"));
  const hand = field(root, "left_handed");
  return { offset, axis, roll, scale, fov, hand, left: hand.raw === "true",
    yaw: degrees(Math.atan2(axis.value[0], axis.value[2])),
    pitch: degrees(Math.asin(unit(axis.value)[1])) };
}

function itemSettings(doc, pose) {
  const text = doc.text;
  const offset = constant(text, "REST", true);
  const seat = pose === "Block" ? constant(text, "BLOCK_SEAT", true)
    : pose === "Torch" ? constant(text, "TORCH_SEAT", true)
    : pose === "Empty hand" ? constant(text, "BARE_SEAT", true) : null;
  const yaw = pose === "Block" ? constant(text, "BLOCK_YAW")
    : pose === "Torch" ? torch(text, "y") : pose === "Blade" ? blade(text, "y") : null;
  const pitch = pose === "Block" ? constant(text, "BLOCK_PITCH")
    : pose === "Empty hand" ? constant(text, "BARE_PITCH") : pose === "Blade" ? blade(text, "x") : null;
  const roll = pose === "Block" ? constant(text, "BLOCK_ROLL")
    : pose === "Torch" ? torch(text, "z") : pose === "Blade" ? blade(text, "z") : constant(text, "BARE_ROLL");
  const contact = pose === "Torch" ? constant(text, "TORCH_HAND_CONTACT", true) : null;
  return { offset, seat, contact, yaw, pitch, roll };
}

export async function poseStudio(ctx, doc, changed) {
  const wand = doc.path === wandPosePath;
  const options = wand ? ["rest", "aim", "inspect", "interact", "sprint"] : ["Block", "Torch", "Blade", "Empty hand"];
  let pose = options[0];
  let drag, pending, temporary;
  const scene = el("canvas", { className: "studio-canvas", width: 720, height: 490,
    "aria-label": "Held equipment framing preview" });
  const controls = el("div", { className: "pose-controls" });
  const status = el("p", { className: "muted", role: "status" });
  const meshes = await Promise.all([
    loadMesh(ctx, "assets/runtime/viewmodels/hand_v6/hand.glb").catch(() => []),
    wand ? loadMesh(ctx, "assets/runtime/viewmodels/artificer_tally/wand.glb").catch(() => []) : [],
  ]);
  const [arm, wandMesh] = meshes;
  const sharedRig = wand ? doc : await ctx.load(wandPosePath);
  const picker = el("select", { onchange: event => { pose = event.target.value; show(); } },
    options.map(value => el("option", { value }, value.replace(/^\w/, letter => letter.toUpperCase()))));
  const read = () => wand ? wandSettings(ctx, doc, pose) : itemSettings(doc, pose);

  function commit(reference, value, outer = false, rebuild = false) {
    if (!ctx.edit(doc, patch(doc.text, reference.start, reference.end, encode(value, outer)), !rebuild)) return false;
    temporary = undefined;
    changed();
    if (rebuild) show();
    else repaint();
    return true;
  }

  function input(name, value, step, callback) {
    const control = el("input", { type: "number", step, value: String(Math.round(value*1000)/1000),
      oninput: event => {
        const value = Number(event.target.value);
        if (event.target.value !== "" && Number.isFinite(value)) callback(value);
      } });
    return el("label", {}, name, control);
  }

  function offsets(title, key, axisNames) {
    const current = read()[key];
    return el("section", {}, el("h3", {}, title), el("div", { className: "pose-axis-row" },
      axisNames.map((name, index) => input(name + " (cm)", current.value[index]*100, .5, value => {
        const next = read()[key];
        const vector = next.value.map((previous, axis) => axis === index ? value/100 : previous);
        commit(next, vector, wand);
      }))));
  }

  function rotateWand(key, value) {
    const state = read();
    const yaw = radians(key === "yaw" ? value : state.yaw);
    const pitch = radians(Math.max(-89.5, Math.min(89.5, key === "pitch" ? value : state.pitch)));
    const length = Math.hypot(...state.axis.value) || 1;
    commit(state.axis, mul([Math.sin(yaw)*Math.cos(pitch), Math.sin(pitch), Math.cos(yaw)*Math.cos(pitch)], length), true);
  }

  function show() {
    const state = read();
    controls.replaceChildren(offsets(wand ? "Grip offset" : "Shared hold offset", "offset", ["Right", "Up", "Forward"]));
    if (state.seat) controls.append(offsets(pose + " offset", "seat", ["Right", "Up", "Toward camera"]));
    if (state.contact) controls.append(offsets("Torch hand contact", "contact", ["X", "Y", "Z"]));
    const angles = [["yaw","Yaw"], ["pitch","Pitch"], ["roll","Roll"]].flatMap(([key, title]) => {
      const ref = state[key];
      if (ref === null || ref === undefined) return [];
      const value = wand && key !== "roll" ? ref : degrees(ref.value);
      return [input(title + " (°)", value, 1, value => {
        if (wand && key !== "roll") rotateWand(key, value);
        else commit(read()[key], radians(value));
      })];
    });
    controls.append(el("section", {}, el("h3", {}, "Rotation"), el("div", { className: "pose-axis-row" }, angles)));
    if (wand) {
      const handed = el("input", { type: "checkbox", checked: state.left, onchange: event => {
        const node = read().hand;
        if (ctx.edit(doc, patch(doc.text, node.start, node.end, String(event.target.checked)))) { changed(); repaint(); }
      } });
      controls.append(el("section", {}, el("h3", {}, "Camera"), el("div", { className: "pose-axis-row" },
        input("Size", state.scale.value, .01, value => commit(read().scale, Math.max(.05, Math.min(4, value)))),
        input("FOV (°)", state.fov.value, 1, value => commit(read().fov, Math.max(40, Math.min(110, value)))),
        el("label", {}, "Left handed", handed))));
    }
    repaint();
  }

  function render() {
    pending = undefined;
    const state = read();
    const position = temporary ?? state.offset.value;
    let faces = [], fov = 72, left = false;
    if (wand) {
      const axis = unit(state.axis.value);
      const side = unit(subtract([1,0,0], mul(axis, axis[0])));
      const forward = mul(cross(side, axis), -1);
      const transform = point => {
        const [x,y,z] = rotateY(mul(point, state.scale.value), state.roll.value);
        return add(position, add(add(mul(side,x),mul(axis,y)),mul(forward,z)));
      };
      faces = [...arm, ...wandMesh].map(face => ({ color: face.color, points: face.points.map(transform) }));
      if (!faces.length) faces = box([-.018,-.08,-.018],[.036,.31,.036],[164,133,87],transform);
      fov = state.fov.value;
      left = state.left;
    } else {
      const yaw = state.yaw?.value ?? 0, pitch = state.pitch?.value ?? 0, roll = state.roll?.value ?? 0;
      const toCam = ([x,y,z]) => [x,y,-z];
      const itemRotation = point => pose === "Blade" ? rotateX(rotateY(rotateZ(point,roll),yaw),pitch)
        : pose === "Torch" ? rotateY(rotateZ(point,roll),yaw) : rotateZ(rotateX(rotateY(point,yaw),pitch),roll);
      const anchor = add(position, toCam(state.seat?.value ?? [0,0,0]));
      const itemTransform = point => add(anchor,toCam(itemRotation(point)));
      const handPosition = pose === "Block" ? add(position, toCam([.105,-.065,.04]))
        : pose === "Torch" ? add(anchor, toCam(itemRotation(add([0,-.085,0], state.contact.value)))) : anchor;
      const armScale = Number(field(ctx.tree(sharedRig), "scale").raw);
      const handTransform = point => add(handPosition, toCam(pose === "Block"
        ? rotateZ(mul(point,armScale),roll) : itemRotation(mul(point,armScale))));
      faces = arm.map(face => ({ color: face.color, points: face.points.map(handTransform) }));
      if (pose === "Block") faces.push(...box([-.0625,-.0625,-.0625],[.125,.125,.125],[137,154,133],itemTransform));
      if (pose === "Torch") {
        faces.push(...box([-.019,-.2404,-.019],[.038,.2772,.038],[154,111,68],itemTransform),
          ...box([-.027,.0368,-.027],[.054,.147,.054],[240,154,76],itemTransform));
      }
      if (pose === "Blade") {
        faces.push(...box([-.021,-.08,-.018],[.042,.16,.036],[138,100,65],itemTransform),
          ...box([-.026,.08,-.008],[.052,.35,.016],[169,186,198],itemTransform));
      }
    }
    const focal = scene.height/2 / Math.tan(radians(fov)*.5);
    const project = point => point[2] <= .04 ? null : [scene.width/2 + (left ? -1 : 1)*point[0]*focal/point[2],
      scene.height/2 - point[1]*focal/point[2]];
    paintModel(scene, faces, { transform: point => point, project, cull: false });
    status.textContent = "Static framing · drag to move the grip";
  }
  const repaint = () => { pending ??= requestAnimationFrame(render); };
  scene.onpointerdown = event => {
    if (event.button !== 0) return;
    drag = { x: event.clientX, y: event.clientY, at: read().offset.value };
    scene.setPointerCapture(event.pointerId);
  };
  scene.onpointermove = event => {
    if (!drag) return;
    const factor = drag.at[2] * 1.45 / (scene.getBoundingClientRect().height || 1);
    const sign = wand && read().left ? -1 : 1;
    temporary = [drag.at[0] + (event.clientX-drag.x)*factor*sign,
      drag.at[1] - (event.clientY-drag.y)*factor, drag.at[2]];
    repaint();
  };
  scene.onpointerup = () => {
    if (drag && temporary) commit(read().offset, temporary, wand, true);
    drag = undefined;
  };
  scene.onlostpointercapture = () => { drag = undefined; temporary = undefined; repaint(); };
  show();
  return el("div", { className: "pose-studio" }, el("div", { className: "studio-preview" }, scene, status),
    el("div", {}, el("div", { className: "studio-tools" }, el("label", {}, wand ? "Pose" : "Equipment", picker),
      button("Open source", () => ctx.open(doc.path, undefined, 1))), controls));
}

