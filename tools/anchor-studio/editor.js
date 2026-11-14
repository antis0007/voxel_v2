import { roles, roomTypes, colors, materials, template, roomAt, putRoom, fillGaps,
    validDoor, validStair, resize, autoDoors, autoStairs, mirror, serialize, parse, decodeBlueprint } from "./design.js";
import { Viewport } from "./viewport.js";

const $ = id => document.getElementById(id);
const buttons = [...document.querySelectorAll("[data-tool]")];
const roomNames = Object.keys(roomTypes);
const styleNames = { SteepHip:"Steep hip", Apartments:"Apartments", Clinic:"Clinic",
    Offices:"Offices", Workshop:"Workshop", Relay:"Crystal relay" };
const label = name => styleNames[name] ?? name.replace(/([a-z])([A-Z])/g,"$1 $2");
const hints = {
    select:"Click a room to inspect its use.",
    room:"Drag a rectangle across the grid to draw a room.",
    door:"Click a shared wall to add or remove a door.",
    entrance:"Click an outside wall to add or remove an entrance.",
    stairs:"Click the first bay of a clear 2 × 2 area inside one room.",
    erase:"Click a room to empty its bays. Undo restores it.",
};
const layout = $("plan");
const canvas = $("view");
const ctx = layout.getContext("2d");
let view;
try { view = new Viewport(canvas); } catch (error) {
    $("view-help").textContent = error.message;
    $("edit-3d").disabled = $("fit").disabled = true;
}
let state = { name:"Freight workshop", design:template("Workshop"), scale:1 };
try {
    const draft = JSON.parse(localStorage.getItem("anchor-studio-draft"));
    if (draft && typeof draft.name === "string" && [1,.5,.25].includes(draft.scale)) {
        state = { name:draft.name.slice(0,40), design:parse(draft.source), scale:draft.scale };
    }
} catch { /* A stale browser draft can be ignored. */ }
const past = [], future = [];
let selected = -1, tool = "select", rectangle = null, gesture = null, editing3d = false;
let model = null, lastBaked = "", timer, request = 0, cutBeforeEdit = 0;
let saved = "", savedName = "", fitNext = true;

function options(element, values) {
    element.replaceChildren(...values.map(value => new Option(label(value), value)));
}

options($("role"),roles);
options($("selected-role"),roomNames);
options($("brush-role"),roomNames);
options($("roof"),["Flat","Gable","Hip","SteepHip","Lean","Glass"]);
options($("windows"),["None","Panes","Slits","Glazed"]);
const palette = ["Walls","Frames","Base","Floor","Roof","Glass"].map((name,i) => {
    const item = document.createElement("label"), select = document.createElement("select");
    item.textContent = name;
    options(select,materials);
    select.addEventListener("change",() => change(d => d.palette[i] = select.value));
    item.append(select);
    $("palette").append(item);
    return select;
});

function status(message) { $("notice").textContent = message; }
function source() { return serialize(state.design); }
function requestData() { return JSON.stringify({ source:source(), scale:state.scale }); }
function remember() {
    try { localStorage.setItem("anchor-studio-draft",JSON.stringify({ name:state.name, scale:state.scale, source:source() })); }
    catch { /* Disk saves remain available if browser storage is disabled. */ }
}

function change(edit, refit = false) {
    const before = structuredClone(state);
    edit(state.design);
    if (JSON.stringify(state) === JSON.stringify(before)) return;
    past.push(before);
    if (past.length > 64) past.shift();
    future.length = 0;
    refresh(refit);
}

function history(from,to) {
    if (!from.length) return;
    to.push(state);
    state = from.pop();
    selected = -1;
    refresh(true);
}

function setDesign(design,name) {
    selected = -1;
    change(() => { state.design=design; state.name=name; },true);
}

function refresh(refit = false) {
    const d=state.design;
    $("name").value=state.name;
    for (const id of ["role","nx","nz","floors","bay","height","roof","windows","age"]) $(id).value=d[id];
    $("scale").value=String(state.scale);
    $("weather-value").value=Math.round(d.age*100)+"%";
    palette.forEach((field,i) => {
        const current = d.palette[i];
        if (![...field.options].some(o => o.value === current)) field.add(new Option(label(current),current));
        field.value=current;
    });
    $("undo").disabled=!past.length;
    $("redo").disabled=!future.length;
    $("file-state").textContent=savedName===state.name && saved===source() ? "Saved" : "Draft";
    showSelection();
    drawPlan();
    remember();
    fitNext ||= refit;
    queueBake();
}

function showSelection() {
    const r=state.design.rooms[selected];
    $("selected-field").hidden=!r;
    $("selected-label").textContent=r ? "Room " + (selected+1) + " · " + (r.box[2]-r.box[0]) + " × " + (r.box[3]-r.box[1]) + " bays"
        : "Click a room to select it.";
    if (r) $("selected-role").value=r.role;
}

function geometry() {
    const d=state.design, rect=layout.getBoundingClientRect();
    const cell=Math.max(1,Math.min((rect.width-34)/d.nx,(rect.height-35)/d.nz));
    return { rect,cell,left:(rect.width-d.nx*cell)/2,top:(rect.height-d.nz*cell)/2 };
}

function drawPlan() {
    const {rect,cell,left,top}=geometry(), d=state.design;
    if (!rect.width || !rect.height) return;
    const dpi=Math.min(2,devicePixelRatio||1);
    layout.width=Math.round(rect.width*dpi);
    layout.height=Math.round(rect.height*dpi);
    ctx.setTransform(dpi,0,0,dpi,0,0);
    ctx.clearRect(0,0,rect.width,rect.height);
    ctx.save();
    ctx.translate(left,top);
    const W=d.nx*cell,H=d.nz*cell;
    ctx.fillStyle="#101921"; ctx.fillRect(0,0,W,H);
    d.rooms.forEach(({role,box:[a,b,c,e]},i) => {
        const x=a*cell,z=b*cell,w=(c-a)*cell,h=(e-b)*cell;
        ctx.fillStyle=roomTypes[role]??"#59636f";
        ctx.globalAlpha=i===selected?.75:.46; ctx.fillRect(x,z,w,h);
        ctx.globalAlpha=1; ctx.lineWidth=i===selected?2.5:1.25; ctx.strokeStyle=i===selected?"#ffe1ac":"#9aa8a380";
        ctx.strokeRect(x+.5,z+.5,w-1,h-1);
        if (cell>=17) {
            ctx.fillStyle="#f0eeea"; ctx.font=(cell>=35?"12px":"10px")+" Segoe UI, sans-serif";
            ctx.textAlign="center"; ctx.textBaseline="middle";
            ctx.fillText(role,w/2+x,h/2+z,w-5);
        }
    });
    ctx.strokeStyle="#a0b8bb25"; ctx.lineWidth=1; ctx.beginPath();
    for (let x=1;x<d.nx;x++) {ctx.moveTo(x*cell,0);ctx.lineTo(x*cell,H);}
    for (let z=1;z<d.nz;z++) {ctx.moveTo(0,z*cell);ctx.lineTo(W,z*cell);}
    ctx.stroke();
    if (d.stairs) {
        const [x,z]=d.stairs;
        ctx.strokeStyle="#a1dfd4"; ctx.lineWidth=1; ctx.setLineDash([3,3]);
        ctx.strokeRect(x*cell+4,z*cell+4,2*cell-8,2*cell-8); ctx.setLineDash([]);
        if (cell>18) {
            ctx.font="10px Segoe UI, sans-serif"; ctx.textAlign="center"; ctx.textBaseline="top"; ctx.fillStyle="#c6fff3";
            ctx.fillText("STAIR", (x+1)*cell,z*cell+6);
        }
    }
    function opening(axis,x,z,exterior) {
        const horizontal=axis==="Z";
        const a=x*cell+(horizontal?.24*cell:0),b=z*cell+(horizontal?0:.24*cell);
        ctx.strokeStyle=exterior?"#a4f1df":"#ffd29b"; ctx.lineWidth=4; ctx.beginPath();
        ctx.moveTo(a,b); ctx.lineTo(a+(horizontal?.52*cell:0),b+(horizontal?0:.52*cell)); ctx.stroke();
    }
    for (const [axis,x,z] of d.doors) opening(axis,x,z,false);
    for (const [side,n] of d.entrances) {
        if (side==="Front"||side==="Back") opening("Z",n,side==="Front"?0:d.nz,true);
        else opening("X",side==="Left"?0:d.nx,n,true);
    }
    if (rectangle) {
        const [a,b,c,e]=rectangle;
        ctx.fillStyle="#ffe0a63d";ctx.fillRect(a*cell,b*cell,(c-a)*cell,(e-b)*cell);
        ctx.strokeStyle="#ffd09a";ctx.lineWidth=2;ctx.strokeRect(a*cell,b*cell,(c-a)*cell,(e-b)*cell);
    }
    ctx.fillStyle="#b4c7c6";ctx.font="10px Segoe UI, sans-serif";ctx.textAlign="center";ctx.textBaseline="bottom";
    ctx.fillText("FRONT",W/2,-3);
    ctx.restore();
}

async function api(path,init={}) {
    const response=await fetch(path,init);
    if (!response.ok) throw Error((await response.json()).error ?? "The Studio request failed.");
    return response;
}
const send = (method,body) => ({ method, body, headers:{"Content-Type":"application/json"} });

function bake() { return api("/api/bake",send("POST",requestData())).then(r=>r.arrayBuffer()); }
function queueBake() {
    clearTimeout(timer);
    const current=++request;
    if (lastBaked===requestData() && model) {
        if (fitNext) {view?.fit(); fitNext=false;}
        $("compile").hidden=true;
        return;
    }
    $("compile").hidden=false;
    $("compile").classList.remove("error");
    $("compile").textContent="Generating building…";
    timer=setTimeout(async () => {
        const key=requestData();
        try {
            const result=decodeBlueprint(await bake());
            if (current!==request) return;
            const full=!model || Number($("cut").value)===model.size[1];
            model=result;
            lastBaked=key;
            $("cut").max=model.size[1];
            if (editing3d) $("cut").value=Math.min(model.size[1],Math.ceil(1.8/state.scale));
            else if (full) $("cut").value=model.size[1];
            $("model-size").textContent=model.size.map(n=>Number((n*state.scale).toFixed(2))).join(" × ")+" m";
            $("compile").hidden=true;
            renderModel(fitNext);
            fitNext=false;
        } catch(error) {
            if (current!==request) return;
            $("compile").classList.add("error");
            const note = error.message.replace(/^Error:\s*/,"");
            $("compile").textContent = note + (model ? " · Showing previous preview" : "");
        }
    },130);
}

function renderModel(refit=false) {
    if (!model) return;
    const cut=Number($("cut").value);
    $("cut-value").value=cut===model.size[1]?"Full":Number((cut*state.scale).toFixed(2))+" m";
    view?.model(model,cut,refit);
}

function point(event, in3d) {
    if (in3d) return view.planPoint(event,state.design.bay,state.scale);
    const {rect,cell,left,top}=geometry();
    return [(event.clientX-rect.left-left)/cell,(event.clientY-rect.top-top)/cell];
}
const clamp = (n,a,b) => Math.max(a,Math.min(b,n));
function cellAt([x,z]) {
    const d=state.design;
    return [clamp(Math.floor(x),0,d.nx-1),clamp(Math.floor(z),0,d.nz-1)];
}
function within([x,z]) {
    const d=state.design;
    return x>=0 && z>=0 && x<d.nx && z<d.nz;
}
function toggle(list,item) {
    const index=list.findIndex(value=>value.join("/")===item.join("/"));
    if (index>=0) list.splice(index,1);
    else list.push(item);
}
function mark(p) {
    const [u,w]=p,d=state.design,[x,z]=cellAt(p);
    if (tool==="entrance") {
        const nearest=[[Math.abs(w),"Front",x],[Math.abs(w-d.nz),"Back",x],[Math.abs(u),"Left",z],[Math.abs(u-d.nx),"Right",z]]
            .sort((a,b)=>a[0]-b[0])[0];
        if (nearest[0]>.48 || u<-.45 || w<-.45 || u>d.nx+.45 || w>d.nz+.45) return status(hints.entrance);
        return change(d=>toggle(d.entrances,nearest.slice(1)));
    }
    if (!within(p)) return;
    if (tool==="select") { selected=roomAt(d,x,z); showSelection(); drawPlan(); return; }
    if (tool==="erase") {
        const index=roomAt(d,x,z);
        if (index<0) return;
        selected=-1;
        return change(d=>{d.rooms.splice(index,1); d.doors=d.doors.filter(e=>validDoor(d,e));
            if (d.stairs && !validStair(d,...d.stairs)) d.stairs=null;});
    }
    if (tool==="stairs") {
        if (!validStair(d,x,z)) return status(hints.stairs);
        return change(d=>{d.stairs=[x,z]; if (d.floors===1) d.floors=2;});
    }
    if (tool==="door") {
        const bx=Math.round(u),bz=Math.round(w);
        const dx=Math.abs(u-bx),dz=Math.abs(w-bz);
        const edge=dx<dz?["X",bx,z]:["Z",x,bz];
        if (Math.min(dx,dz)>.32 || !validDoor(d,edge)) return status(hints.door);
        return change(d=>toggle(d.doors,edge));
    }
}

function start(e,in3d) {
    if (gesture || (in3d&&!view)) return;
    const element=e.currentTarget;
    if (in3d && (!editing3d || e.button!==0 || e.shiftKey)) {
        gesture={type:"orbit",id:e.pointerId,x:e.clientX,y:e.clientY,pan:e.shiftKey||e.button===1};
    } else {
        if (e.button!==0) return;
        const p=point(e,in3d);
        if (tool!=="room") {mark(p);return;}
        if (!within(p)) return;
        const [x,z]=cellAt(p);
        gesture={type:"room",id:e.pointerId,x,z};
        rectangle=[x,z,x+1,z+1];
        drawPlan();
    }
    element.setPointerCapture(e.pointerId);
}
function move(e,in3d) {
    if (gesture?.id!==e.pointerId) return;
    if (gesture.type==="orbit") {
        view.orbit(e.clientX-gesture.x,e.clientY-gesture.y,gesture.pan);
        gesture.x=e.clientX;gesture.y=e.clientY;
        return;
    }
    const [x,z]=cellAt(point(e,in3d)), a=gesture.x,b=gesture.z;
    rectangle=[Math.min(x,a),Math.min(z,b),Math.max(x,a)+1,Math.max(z,b)+1];
    drawPlan();
}
function end(e,commit) {
    if (gesture?.id!==e.pointerId) return;
    const box=rectangle;
    rectangle=null;
    gesture=null;
    if (commit && box) change(d=>{selected=putRoom(d,box,$("brush-role").value);});
    else drawPlan();
}
for (const [element,in3d] of [[layout,false],[canvas,true]]) {
    element.addEventListener("pointerdown",e=>start(e,in3d));
    element.addEventListener("pointermove",e=>move(e,in3d));
    element.addEventListener("pointerup",e=>end(e,true));
    element.addEventListener("pointercancel",e=>end(e,false));
    element.addEventListener("lostpointercapture",e=>end(e,false));
}
new ResizeObserver(drawPlan).observe(layout);

for (const button of buttons) button.addEventListener("click",() => {
    tool=button.dataset.tool;
    for (const other of buttons) other.setAttribute("aria-pressed",String(other===button));
    $("tool-hint").textContent=hints[tool];
    $("brush-label").hidden=tool!=="room";
});
$("name").addEventListener("change",()=>change(()=>state.name=$("name").value.trim()));
for (const id of ["nx","nz","floors","bay","height","age"]) {
    $(id).addEventListener("change",()=>{
        const value=$(id).valueAsNumber;
        if (!Number.isFinite(value)) {refresh();return;}
        const n=clamp(value,Number($(id).min),Number($(id).max));
        selected=-1;
        change(d=>{
            if (id==="nx"||id==="nz") resize(d,id==="nx"?Math.round(n):d.nx,id==="nz"?Math.round(n):d.nz);
            else d[id]=id==="floors"?Math.round(n):n;
        },["nx","nz","floors","bay","height"].includes(id));
    });
}
for (const id of ["role","roof","windows"]) $(id).addEventListener("change",()=>change(d=>d[id]=$(id).value));
$("scale").addEventListener("change",()=>change(()=>state.scale=Number($("scale").value),true));
$("selected-role").addEventListener("change",()=>change(d=>d.rooms[selected].role=$("selected-role").value));
$("template").addEventListener("click",()=> {
    const role=$("role").value;
    setDesign(template(role),label(role)+" design");
    status("Template loaded. Draw a room or change the building settings.");
});
async function branch(kind, edit) {
    const original=state.name;
    try {
        const {names}=await (await api("/api/library")).json();
        const used=new Set(names.map(name=>name.toLowerCase()));
        let i=1, next;
        do {
            const suffix=" - "+kind+(i===1?"":" "+i);
            next=(original||"Building").slice(0,40-suffix.length).trimEnd()+suffix;
            i++;
        } while (used.has(next.toLowerCase()));
        if (state.name!==original) return;
        change(d=>{state.name=next;edit?.(d);});
        status("New draft. Save to keep it alongside the original.");
    } catch (error) {status(error.message);}
}
$("duplicate").addEventListener("click",()=>branch("copy"));
$("variant").addEventListener("click",()=>branch("variant",d=>{
    d.seed=(Math.imul(d.seed,1664525)+1013904223)>>>0;
    const schemes=[
        ["LimePlaster","SteelPlate","WeatheredBricks","DressedStone","CopperBlock","WindowGlass"],
        ["Basalt","RustedSteel","WeatheredBricks","Concrete","SteelPlate","WindowGlass"],
        ["WeatheredBricks","DressedStone","StoneBricks","Planks","RoofTiles","WindowGlass"],
    ];
    d.palette=[...schemes[d.seed%schemes.length]];
    d.age=.12+(d.seed%16)/100;
}));
for (const [id,edit] of [
    ["fill",d=>fillGaps(d,$("brush-role").value)],["auto-doors",autoDoors],["auto-stairs",autoStairs],["mirror",mirror],
    ["blank",d=>{selected=-1;d.rooms=[];d.doors=[];d.stairs=null;d.floors=1;}],
]) $(id).addEventListener("click",e=>{e.currentTarget.closest("details").open=false; change(edit);});
$("undo").addEventListener("click",()=>history(past,future));
$("redo").addEventListener("click",()=>history(future,past));
document.addEventListener("keydown",e=>{
    if (!(e.ctrlKey||e.metaKey) || /INPUT|TEXTAREA|SELECT/.test(e.target.tagName)) return;
    if (e.key.toLowerCase()==="z") {e.preventDefault(); e.shiftKey?history(future,past):history(past,future);}
    if (e.key.toLowerCase()==="y") {e.preventDefault(); history(future,past);}
});
$("cut").addEventListener("input",()=>renderModel());
$("fit").addEventListener("click",()=>view?.fit());
$("edit-3d").addEventListener("click",()=>{
    if (!view) return;
    editing3d=!editing3d;
    $("edit-3d").setAttribute("aria-pressed",String(editing3d));
    canvas.classList.toggle("editing",editing3d);
    $("view-help").textContent=editing3d ? "Use the active plan tool here · Right-drag to orbit" : "Drag to orbit · Scroll to zoom · Shift-drag to pan";
    if (model) {
        if (editing3d) {cutBeforeEdit=Number($("cut").value);$("cut").value=Math.min(model.size[1],Math.ceil(1.8/state.scale));}
        else $("cut").value=Math.min(model.size[1],cutBeforeEdit||model.size[1]);
        renderModel();
    }
});

function filename() {return state.name.trim().replace(/[^A-Za-z0-9 _-]/g,"-").slice(0,40)||"Anchor building";}
function download(blob,ext) {
    const url=URL.createObjectURL(blob), link=document.createElement("a");
    link.href=url;link.download=filename()+ext;link.click();
    setTimeout(()=>URL.revokeObjectURL(url),1000);
}
$("save").addEventListener("click",async () => {
    const name=state.name.trim(),text=source();
    try {
        await api("/api/library/"+encodeURIComponent(name),send("PUT",JSON.stringify({source:text})));
        savedName=name;saved=text;refresh();
        status("Saved “"+name+"” to your design library.");
    } catch(error) {status(error.message);}
});
$("library").addEventListener("click",async ()=>{
    const items=$("library-items");
    items.textContent="Reading saved designs…";
    $("library-dialog").showModal();
    try {
        const {names}=await (await api("/api/library")).json();
        items.replaceChildren();
        if (!names.length) items.textContent="Your saved designs will appear here.";
        for (const name of names) {
            const b=document.createElement("button");b.textContent=name;
            b.addEventListener("click",async()=>{
                try {
                    const {source:text}=await (await api("/api/library/"+encodeURIComponent(name))).json();
                    const design=parse(text);savedName=name;saved=serialize(design);
                    setDesign(design,name);refresh();
                    $("library-dialog").close();status("Opened “"+name+"”.");
                } catch(error) {status(error.message);}
            });
            items.append(b);
        }
    } catch(error) {items.textContent=error.message;}
});
$("close-library").addEventListener("click",()=>$("library-dialog").close());
$("import-plan").addEventListener("change",async e=>{
    const file=e.target.files[0];
    try {
        if (!file) return;
        if (file.size>16384) throw Error("Choose an editable .vxplan file up to 16 KB.");
        setDesign(parse(await file.text()),file.name.replace(/\.vxplan$/i,"").slice(0,40));
        $("library-dialog").close();status("Design imported. Save it to keep it in your library.");
    } catch(error) {status(error.message);}
    finally {e.target.value="";}
});
$("export-plan").addEventListener("click",()=>{
    $("export-menu").open=false;download(new Blob([source()],{type:"text/plain"}),".vxplan");
});
$("export-blueprint").addEventListener("click",async()=>{
    $("export-menu").open=false;
    try {download(new Blob([await bake()]),".vxbp");status("Blueprint exported at "+state.scale+" m per block.");}
    catch(error){status(error.message);}
});

$("close-city").addEventListener("click",()=>$("city-dialog").close());
let cityView;
const cityCanvas=$("city-view");
let cityDrag;
cityCanvas.addEventListener("pointerdown",event=>{
    cityDrag=[event.clientX,event.clientY];
    cityCanvas.setPointerCapture(event.pointerId);
});
cityCanvas.addEventListener("pointermove",event=>{
    if (!cityDrag) return;
    cityView?.orbit(event.clientX-cityDrag[0],event.clientY-cityDrag[1],event.shiftKey);
    cityDrag=[event.clientX,event.clientY];
});
cityCanvas.addEventListener("lostpointercapture",()=>cityDrag=null);
$("review").addEventListener("click",async()=>{
    const b=$("review"),statusLine=$("city-progress");
    b.disabled=true;
    statusLine.textContent="Generating city preview…";
    try {
        const result=decodeBlueprint(await (await api("/api/review",send("POST",requestData()))).arrayBuffer());
        $("city-dialog").showModal();
        cityView ??= new Viewport(cityCanvas);
        cityView.model(result,result.size[1],true);
        statusLine.textContent="";
    } catch(error) {
        $("city-dialog").close();
        statusLine.textContent=error.message;
    } finally {
        b.disabled=false;
    }
});

refresh(true);
