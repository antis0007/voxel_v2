import { readFileSync, writeFileSync, readdirSync, openSync, closeSync, readSync, fstatSync } from "node:fs";
import path from "node:path";
import { randomBytes, createHash } from "node:crypto";
import { root, state, ensureDirectory } from "./workspace.mjs";
import * as game from "./game.mjs";

const names = name => typeof name === "string" && /^[a-zA-Z0-9_-]{1,48}$/.test(name);
let observedRevision;
// Four decimals preserve sub-millimetre positions and readable balance values
// without leaking f32 promotion noise into every human/agent observation.
const compact = value => JSON.parse(JSON.stringify(value, (_key, item) =>
  typeof item === "number" && Number.isFinite(item) && Math.abs(item) < 1e9 ? Number(item.toFixed(4)) : item));

function completePng(file) {
  let fd;
  try {
    fd = openSync(file, "r");
    const size = fstatSync(fd).size;
    if(size < 20)return false;
    const head = Buffer.alloc(8), tail = Buffer.alloc(12);
    readSync(fd,head,0,8,0);readSync(fd,tail,0,12,size-12);
    return head.equals(Buffer.from("89504e470d0a1a0a","hex")) && tail.equals(Buffer.from("0000000049454e44ae426082","hex"));
  }catch{return false;}finally{if(fd!==undefined)closeSync(fd);}
}

export async function engineRequest(request) {
  if (!request || typeof request !== "object" || Array.isArray(request) || typeof request.op !== "string") {
    throw new Error("Expected an engine request with op; run engine-cli help");
  }
  const encoded = JSON.stringify(request);
  if (Buffer.byteLength(encoded) > 31980) throw new Error("Engine request exceeds 31980 bytes");
  const reply = await game.command("/engine " + encoded);
  let result;
  try { result = JSON.parse(reply); } catch { throw new Error("This build does not expose Engine Workbench protocol 1; rebuild and relaunch"); }
  if (result.protocol !== 1) throw new Error("Unsupported engine protocol; rebuild and relaunch");
  observedRevision = {run:game.status().run?.id,revision:result.revision};
  if (!result.ok) throw new Error(result.error || "Native request rejected");
  if (request.op === "capture") {
    const capture = path.resolve(result.data.capture);
    const allowed = path.resolve(game.status().saveDirectory, "captures") + path.sep;
    if (!capture.startsWith(allowed)) throw new Error("Capture was outside the active playtest folder");
    const deadline = Date.now() + 15000;
    while (!completePng(capture) && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 100));
    result.data.status = completePng(capture) ? "written" : "pending";
  }
  return compact(result);
}

export function startEngineSession(request) {
  if (!request || typeof request!=="object" || Array.isArray(request) || Object.keys(request).some(k => !["world","seed","scale","generator","scene","view"].includes(k))) {
    throw new Error("Session fields: world, seed, scale, optional generator, scene (exploration or factory) and view (4..64 chunks)");
  }
  const session = { world: "hills", seed: 74, scale: 1, ...request };
  if(session.scene==='factory')session.world='superflat';
  const directory = ensureDirectory(path.join(state, "engine-worlds", randomBytes(8).toString("hex")));
  game.start(false, undefined, undefined, { directory, session });
  return game.status();
}

export async function enginePreset(request) {
  const directory = ensureDirectory(path.join(state,"engine-presets"));
  if (request.op === "list") return { presets: readdirSync(directory).filter(p=>p.endsWith(".json")).map(p=>p.slice(0,-5)) };
  if (!names(request.name)) throw new Error("Preset name: 1..48 letters, digits, hyphens or underscores");
  const file = path.join(directory,request.name + ".json");
  if (request.op === "save") {
    const snapshot = await engineRequest({op:"inspect",sections:["tuning","world"]});
    const preset = {schema:1,created:new Date().toISOString(),tuning:snapshot.data.tuning,
      world:snapshot.data.world,run:game.status().run};
    writeFileSync(file,JSON.stringify(preset,null,2),{flag:"wx"});
    return {name:request.name,path:path.relative(root,file).replaceAll("\\","/")};
  }
  if (request.op === "load") {
    const preset = JSON.parse(readFileSync(file,"utf8"));
    if(preset.schema!==1)throw new Error("Unsupported preset schema");
    return engineRequest({op:"tune",revision:request.revision,values:preset.tuning});
  }
  throw new Error("Preset operation: list, save or load");
}

export async function engineSample(request) {
  const { count = 4, interval_ms = 500, sections = ["player","location"] } = request;
  if (!Number.isInteger(count) || count < 1 || count > 20 || !Number.isInteger(interval_ms)
    || interval_ms < 100 || interval_ms > 2000 || count * interval_ms > 20000) throw new Error("Sample: 1..20 readings, 100..2000 ms apart, at most 20 seconds");
  const records = [];
  const started = Date.now();
  const run = game.status().run;
  const sameRun = () => game.status().running && run?.id === game.status().run?.id;
  for(let i=0;i<count;i++) {
    if(i)await new Promise(resolve=>setTimeout(resolve,interval_ms));
    if(!sameRun())throw new Error("Play session changed during sampling; start a fresh observation series");
    const value = await engineRequest({op:"inspect",sections});
    if(!sameRun())throw new Error("Play session changed during sampling; start a fresh observation series");
    records.push({elapsed_ms:Date.now()-started,revision:value.revision,...value.data});
  }
  return {run,records};
}

export async function engineContext(task = "") {
  if(!game.status().ready)return undefined;
  try {
    const sections=["world","player"];
    if(/mana|ambience|atmosphere|environment|regen|location|nature|weather|sparkle/i.test(task))sections.push("location","tuning");
    if(/model|machine|factory|voxel.art/i.test(task))sections.push("models");
    if(sections.length===2 || /material|terrain|block|surface/i.test(task))sections.push("look");
    const observed=await engineRequest({op:"inspect",sections});
    const result={capturedAt:new Date().toISOString(),run:game.status().run,revision:observed.revision,
      observation:observed.data,scope:"One native observation; not a playthrough or performance measurement",
      controls:"describe engine via engine-cli; live tune/material/model, undo/reset, vitals/camera/time/capture",
      sources:["crates/app/src/dev_panel/engine.rs","tools/axiom-dev/ENGINE_WORKBENCH.md",
        ...(sections.includes("location")?["crates/app/src/world/ambience.rs"]:[]),
        ...(sections.includes("models")?["crates/app/src/mesh/models/voxel_asset.rs"]:[])]};
    result.hash=createHash("sha256").update(JSON.stringify(result)).digest("hex");
    // Live frame-time percentiles ride along only for performance briefs, after hashing: they change every call.
    if(/perf|fps|frame|stutter|lag|hitch|slow|smooth|latency|jank|budget/i.test(task)) {
      try {const {perf,age_ms,scope}=(await engineRequest({op:"perf"})).data;result.livePerf={...perf,age_ms,scope};}
      catch(error) {result.livePerf={unavailable:error.message};}
    }
    return result;
  } catch(error) {return {unavailable:error.message};}
}

export function assertEngineContextFresh(context) {
  if(!context?.run)return;
  const current=game.status();
  if(!current.running || current.run?.id!==context.run.id || (observedRevision?.run===context.run.id && observedRevision.revision!==context.revision)) {
    throw new Error("Native playtest or preview revision changed; gather fresh engine context");
  }
}
