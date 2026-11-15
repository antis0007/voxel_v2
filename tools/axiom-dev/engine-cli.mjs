// Compact client for the same authenticated native controls as the panel.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const sessionIndex = args.indexOf("--session");
const sessionFile = sessionIndex >= 0 ? args.splice(sessionIndex,2)[1]
  : fileURLToPath(new URL("../../target/axiom-dev/engine-session.json",import.meta.url));
const [command="help",...rest] = args;
const input = () => JSON.parse(readFileSync(0,"utf8").replace(/^\uFEFF/,""));
const optionalInput = () => {const text=process.stdin.isTTY?"":readFileSync(0,"utf8").trim();return text?JSON.parse(text):{};};
const help = {commands:{
  "status":"Compact process and build identity",
  "start":"Optional session JSON on stdin; creates an isolated playable world",
  "wait":"Wait up to 60 seconds for a loaded world using read-only observations",
  "close":"Close this panel's disposable engine world without requesting a save",
  "describe [ambience|materials|models|engine]":"Native schemas, units, bounds, source owners and reload support",
  "inspect [player location world look tuning models civilization]":"Only requested state; default player + location",
  "survey [radius_m]":"Nine generated surface positions and biomes around the player",
  "tune":"Field/value JSON on stdin; atomic live changes with automatic revision preflight",
  "request":"Exact native JSON on stdin; caller supplies revisions",
  "sample":"Optional count/interval_ms/sections JSON on stdin, bounded to 20 seconds",
  "model <loaded-asset> <file.vox>":"Validated in-place model swap, no rebuild",
  "material":"Family/tint/optional roughness/reflectance JSON on stdin",
  "undo":"Undo the last preview; at most 16",
  "reset":"Clear live tuning/material/model overrides",
  "capture <name>":"Native screenshot with completion status and local path",
  "command <line>":"Any solo chat command, slash optional, e.g. give Copper 8, time 6000, gamemode creative",
  "systems [id=on|off ...]":"List world systems (assets/systems.ron) or switch them live, e.g. systems magic=off space=on",
  "fill x0 y0 z0 x1 y1 z1 Block":"Set an inclusive box of voxel cells, at most 4096",
  "probe x y z":"Block in one voxel cell and its centre in metres",
  "hedrons [radius_m]":"Mana hedrons near the player with capture state and pylon cells",
  "capture-hedron [radius_m]":"Build the pylon frame on the nearest hedron and report capture",
  "enemies [radius_m] | clear-enemies [radius_m]":"List or remove creatures near the player (no radius clears all)",
  "spawn-enemy Kind [count]":"Summon ahead of the player; stdin JSON {position_m,affixes,elements,health}",
  "scenario file.json":"Run {steps:[{request,expect:{path:value|{min,max}}}]}; prints pass/fail, exit 1 on failure",
  "preset list|save|load [name]":"Named ambience experiments; saved presets never overwrite",
  "test x y z [Kind] [ticks]":"One call: place Kind (optional), apply set JSON from stdin, step ticks; prints end state + changed fields",
  "machine [x y z] | machines [radius_m] [Kind] | machine-kinds":"Read machine state (defaults omitted)",
  "place-machine x y z Kind | set-machine x y z | step-factory ticks [x y z ...]":"Single steps of test; set JSON on stdin",
}, docs:"tools/axiom-dev/ENGINE_WORKBENCH.md", session:"Start tools/axiom-dev/server.mjs; --session FILE selects a specific panel"};

const cell = values => {
  const n=values.slice(0,3).map(Number);
  if(n.length!==3||n.some(v=>!Number.isInteger(v)))throw Error("Expected integer voxel cell x y z");
  return n;
};

async function run() {
  if(command==="help")return help;
  const session=JSON.parse(readFileSync(sessionFile,"utf8"));
  const origin=new URL(session.origin);
  if(origin.hostname!=="127.0.0.1" || origin.protocol!=="http:")throw Error("Expected a local panel session");
  const api=async(route,payload)=>{
    const response=await fetch(new URL(route,origin),{method:payload===undefined?"GET":"POST",
      headers:{"X-Axiom-Session":session.token,"Content-Type":"application/json"},
      body:payload===undefined?undefined:JSON.stringify(payload),signal:AbortSignal.timeout(45000)});
    const value=await response.json();
    if(!response.ok)throw Error(value.error||response.statusText);
    return value;
  };
  const engine=request=>api("/api/engine",request);
  const revision=async()=> (await engine({op:"inspect",sections:["tuning"]})).revision;
  switch(command) {
    case "status": {const s=await api("/api/game");return {running:s.running,ready:s.ready,building:s.building,run:s.run,saveDirectory:s.saveDirectory,latest:s.logs.slice(-3)};}
    case "start": {const s=await api("/api/engine/start",optionalInput());return {running:s.running,ready:s.ready,run:s.run,saveDirectory:s.saveDirectory};}
    case "close": return api("/api/engine/close",{});
    case "wait": {
      const deadline=Date.now()+60000;
      let reason="World is loading";
      let previous;
      while(Date.now()<deadline){
        const status=await api("/api/game");
        if(!status.running)throw Error("No active play session");
        if(status.ready){
          try{
            const observed=await engine({op:"inspect",sections:["world","player","location"]});
            const identity=JSON.stringify([status.run?.id,observed.data.world.seed,observed.data.world.generator,observed.data.world.kind]);
            if(observed.data.location.ready && previous===identity)return observed;
            previous=observed.data.location.ready?identity:undefined;
            reason="World loaded; waiting for a stable local sample";
          }catch(error){previous=undefined;reason=error.message;}
        }
        await new Promise(resolve=>setTimeout(resolve,1000));
      }
      throw Error("Still waiting: " + reason);
    }
    case "describe": return engine({op:"describe",...(rest[0]?{system:rest[0]}:{})});
    case "inspect": return engine({op:"inspect",sections:rest});
    case "survey": return engine({op:"survey",radius_m:rest[0]===undefined?128:Number(rest[0])});
    case "request": return engine(input());
    case "tune": {const values=input();return engine({op:"tune",revision:await revision(),values});}
    case "material": {const fields=input();return engine({...fields,op:"material",revision:await revision()});}
    case "model": {
      if(rest.length!==2)throw Error("model <loaded-asset> <file.vox>");
      return engine({op:"model",revision:await revision(),asset:rest[0],text:readFileSync(rest[1],"utf8")});
    }
    case "undo": case "reset": return engine({op:command,revision:await revision()});
    case "capture": return engine({op:"capture",name:rest[0]});
    // The leading slash is optional: POSIX shells on Windows rewrite /word into a path.
    case "command": {const line=rest.join(" ");return engine({op:"command",line:line.startsWith("/")?line:"/"+line});}
    case "fill": {
      const n=rest.slice(0,6).map(Number);
      if(rest.length!==7||n.some(v=>!Number.isInteger(v)))throw Error("fill x0 y0 z0 x1 y1 z1 BlockName");
      return engine({op:"fill",min:n.slice(0,3),max:n.slice(3,6),block:rest[6]});
    }
    case "systems": {
      const set={};
      for(const pair of rest){const [id,state]=pair.split("=");if(!id||!["on","off"].includes(state))throw Error("systems id=on|off ...");set[id]=state==="on";}
      return engine({op:"systems",set});
    }
    case "probe": return engine({op:"probe",position:rest.slice(0,3).map(Number)});
    case "machine-kinds": return engine({op:"machine_kinds"});
    case "machine": return engine({op:"machine",...(rest.length?{position:cell(rest)}:{})});
    case "machines": {
      const radius=rest.find(v=>Number.isFinite(Number(v))), kind=rest.find(v=>!Number.isFinite(Number(v)));
      return engine({op:"machines",...(radius===undefined?{}:{radius_m:Number(radius)}),...(kind?{kind}:{})});
    }
    case "place-machine": {
      if(rest.length<4)throw Error("place-machine x y z Kind");
      return engine({...optionalInput(),op:"place_machine",position:cell(rest),kind:rest.slice(3).join(" ")});
    }
    case "set-machine": return engine({...input(),op:"set_machine",position:cell(rest)});
    case "test": {
      const extra=rest.slice(3), ticks=extra.find(v=>/^\d+$/.test(v)), kind=extra.filter(v=>!/^\d+$/.test(v)).join(" ");
      return engine({op:"machine_test",position:cell(rest),...(kind?{kind}:{}),ticks:Number(ticks??0),set:optionalInput()});
    }
    case "step-factory": {
      const ticks=Number(rest[0]),coords=rest.slice(1).map(Number);
      if(!Number.isInteger(ticks)||coords.length%3||coords.some(v=>!Number.isInteger(v)))throw Error("step-factory ticks [x y z ...]");
      const probe=[];for(let i=0;i<coords.length;i+=3)probe.push(coords.slice(i,i+3));
      return engine({op:"step_factory",ticks,probe});
    }
    case "enemies": return engine({op:"enemies",...(rest[0]===undefined?{}:{radius_m:Number(rest[0])})});
    case "spawn-enemy": {
      if(!rest.length)throw Error("spawn-enemy Kind [count]; optional JSON on stdin");
      const count=rest.find(v=>/^\d+$/.test(v));
      return engine({...optionalInput(),op:"spawn_enemy",kind:rest.filter(v=>!/^\d+$/.test(v)).join(" "),...(count?{count:Number(count)}:{})});
    }
    case "clear-enemies": return engine({op:"clear_enemies",...(rest[0]===undefined?{}:{radius_m:Number(rest[0])})});
    case "scenario": {
      // File: {steps:[{op,..., expect?}]} (factory-lab shape) or {steps:[{request:{op,...}, expect?}]}.
      // expect: {"dot.path": value | {min?,max?}}; paths index the reply's data.
      const {steps}=JSON.parse(readFileSync(rest[0],"utf8").replace(/^﻿/,""));
      const failures=[];
      for(const [i,step] of steps.entries()){
        let data;
        const {expect:_,...flat}=step;
        try{const reply=await engine(step.request??flat);data=reply.data??reply;}
        catch(error){failures.push({step:i,error:error.message});break;}
        for(const [path,want] of Object.entries(step.expect??{})){
          const got=path.split(".").reduce((v,k)=>v?.[k],data);
          const ok=want!==null&&typeof want==="object"?(want.min===undefined||got>=want.min)&&(want.max===undefined||got<=want.max)
            :JSON.stringify(got)===JSON.stringify(want);
          if(!ok)failures.push({step:i,path,want,got});
        }
      }
      if(failures.length)process.exitCode=1;
      return {pass:!failures.length,steps:steps.length,...(failures.length?{failures}:{})};
    }
    case "hedrons": return engine({op:"hedrons",...(rest[0]===undefined?{}:{radius_m:Number(rest[0])})});
    case "capture-hedron": {
      // Builds the capture frame around the nearest hedron: one fill per pylon cell run.
      const found=(await engine({op:"hedrons",...(rest[0]===undefined?{}:{radius_m:Number(rest[0])})})).data.hedrons[0];
      if(!found)throw Error("No hedron in range; pass a larger radius in metres");
      for(const cell of found.pylon_cells)await engine({op:"fill",min:cell,max:cell,block:found.pylon_block});
      const after=(await engine({op:"hedrons",...(rest[0]===undefined?{}:{radius_m:Number(rest[0])})})).data.hedrons[0];
      return {element:after.element,centre_m:after.centre_m,captured:after.captured,cells:found.pylon_cells.length};
    }
    case "sample": return api("/api/engine/sample",optionalInput());
    case "preset": return api("/api/engine/preset",{op:rest[0],name:rest[1],...(rest[0]==="load"?{revision:await revision()}:{})});
    default: throw Error("Unknown command; run help");
  }
}
const DATA_ONLY=new Set(["machine","machines","machine-kinds","place-machine","set-machine","step-factory","test","probe","fill","hedrons","systems"]);
run().then(value=>console.log(JSON.stringify(DATA_ONLY.has(command)&&value?.data!==undefined?value.data:value))).catch(error=>{console.error(JSON.stringify({error:error.message}));process.exitCode=1;});
