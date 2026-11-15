import { el, button, panel } from "./inspector.mjs";
import { copyText } from "./context-menu.mjs";

// Field controls are generated from the running engine's schema. No second
// browser-side balance table or implicit source saves.
export function enginePanel(api, note) {
  let revision;
  const values=new Map();
  const controls=el("div",{className:"cheats"});
  const output=el("pre",{className:"log","aria-label":"Engine state"});
  const hint=el("p",{className:"muted"},"Live previews · Session only · Undo available");
  const request=async body=>{
    const result=await api("/api/engine",undefined,body);
    revision=result.revision;
    output.textContent=JSON.stringify(result.data,null,2);
    return result.data;
  };
  const act=fn=>async()=>{try{await fn();}catch(error){note(error.message,true);}};
  async function refresh(){
    const schema=await request({op:"describe",system:"ambience"});
    const state=await request({op:"inspect",sections:["tuning","player","location"]});
    controls.replaceChildren();values.clear();
    for(const field of [...schema.controls,...schema.switches]) {
      const input=el("input",field.type==="boolean"?{type:"checkbox"}:{type:"number",min:field.min,max:field.max,step:field.step});
      if(field.type==="boolean")input.checked=state.tuning[field.key];else input.value=state.tuning[field.key];
      values.set(field.key,{field,input});
      controls.append(el("label",{},field.label + (field.unit?" ("+field.unit+")":""),input));
    }
  }
  const preset=el("input",{placeholder:"Preset name","aria-label":"Preset name",pattern:"[a-zA-Z0-9_-]{1,48}"});
  const presetList=el("select",{"aria-label":"Saved preset"});
  async function listPresets(){
    const {presets}=await api("/api/engine/preset",undefined,{op:"list"});
    presetList.replaceChildren(...presets.map(name=>el("option",{value:name},name)));
  }
  const apply=button("Apply live",act(async()=>{
    if(revision===undefined)throw Error("Inspect the running engine first");
    const patch=Object.fromEntries([...values].map(([key,{field,input}])=>[key,field.type==="boolean"?input.checked:Number(input.value)]));
    await request({op:"tune",revision,values:patch});await refresh();
  }),"Apply all fields atomically to the running game");
  const undo=button("Undo",act(async()=>{await request({op:"undo",revision});await refresh();}));
  const reset=button("Reset previews",act(async()=>{await request({op:"reset",revision});await refresh();}));
  const captureName=el("input",{placeholder:"Capture name","aria-label":"Capture name"});
  const sessionWorld=el("select",{"aria-label":"Playtest world"},...["hills","superflat","planet","nether","manyfold","gilded"].map(value=>el("option",{value},value)));
  const sessionSeed=el("input",{type:"number",min:0,max:4294967295,value:74,"aria-label":"World seed"});
  const sessionScale=el("select",{"aria-label":"Voxel size"},...[1,0.5,0.25].map(value=>el("option",{value},value+" m")));
  const scene=el("select",{"aria-label":"Review scene"},el("option",{value:"exploration"},"Exploration"),el("option",{value:"factory"},"Factory models"));
  scene.onchange=()=>{sessionWorld.disabled=scene.value==="factory";if(sessionWorld.disabled)sessionWorld.value="superflat";};
  const surveyRadius=el("input",{type:"number",min:4,max:2048,value:128,"aria-label":"Survey radius in metres"});
  const card=panel("Engine workbench",hint,
    el("div",{className:"chips"},scene,sessionWorld,sessionSeed,sessionScale,
      button("Start fresh world",act(()=>api("/api/engine/start",undefined,{world:sessionWorld.value,seed:Number(sessionSeed.value),scale:Number(sessionScale.value),scene:scene.value})),"Open an isolated world; factory scene uses superflat and a fixed review camera"),
      button("Close experiment",act(async()=>{await api("/api/engine/close",undefined,{});revision=undefined;controls.replaceChildren();values.clear();output.textContent="Experiment closed · Captures and presets remain";}),"Close this disposable world without saving; normal playtest worlds use the game's save and quit controls")),
    el("div",{className:"chips"},button("Inspect / refresh",act(refresh)),apply,undo,reset,
      button("Capabilities",act(()=>request({op:"describe"}))),button("Frame stats",act(()=>request({op:"perf"}))),
      button("Sample recovery",act(async()=>{output.textContent=JSON.stringify(await api("/api/engine/sample",undefined,{count:6,interval_ms:500}),null,2);}))),
    controls,
    el("div",{className:"chips"},el("label",{},"Survey radius (m)",surveyRadius),button("Survey terrain",act(()=>request({op:"survey",radius_m:Number(surveyRadius.value)})))),
    el("div",{className:"chips"},preset,button("Save preset",act(async()=>{await api("/api/engine/preset",undefined,{op:"save",name:preset.value});await listPresets();})),
      presetList,button("List presets",act(listPresets)),button("Load preset",act(async()=>{await api("/api/engine/preset",undefined,{op:"load",name:presetList.value,revision});await refresh();}))),
    el("div",{className:"chips"},captureName,button("Capture frame",act(()=>request({op:"capture",name:captureName.value}))),
      button("Copy state",act(()=>copyText(output.textContent)))),output);
  return card;
}
