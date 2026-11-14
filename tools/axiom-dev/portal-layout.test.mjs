import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { spawn, spawnSync } from "node:child_process";
import { applyPortalOperations, encodePortalLayout, portalPath } from "./portal-operations.mjs";
import { checkPortalLayout, portalSources, portalDirectory } from "./portal-validator.mjs";
const root = fs.realpathSync(fileURLToPath(new URL("../../", import.meta.url)));
const node = (id, x, dimension = "Surface") => ({ id, label: id, dimension, position: [x,3,0], yaw_degrees: 0, half_size: [1.5,2], canvas: [x*20,60] });
const pair = () => ({ version:1, nodes:[node("a",0),node("b",12)], links:[{id:"fold",kind:"Fold",from:"a",to:"b",two_way:true,enabled:true}] });
const check = graph => checkPortalLayout(root, encodePortalLayout(graph));

test("native RON round trip preserves node geometry, typed links and graph placement", () => {
  const graph=pair(); assert.deepEqual(check(graph).graph,graph); assert.deepEqual(check(graph).warnings,[]);
});
test("cross-dimension links, missing references and scaling fail native validation", () => {
  let graph=pair(); graph.nodes[1].dimension="Hell"; assert.throws(()=>check(graph),/one dimension/);
  graph=pair(); graph.links[0].to="missing"; assert.throws(()=>check(graph),/missing opening/);
  graph=pair(); graph.nodes[1].half_size=[2,2]; assert.throws(()=>check(graph),/same width/);
});
test("duplicate IDs, ambiguous routes, invalid bounds and excessive graphs fail", () => {
  let graph=pair(); graph.nodes[1].id="a"; assert.throws(()=>check(graph),/unique/);
  graph=pair(); graph.nodes.push(node("c",24)); graph.links.push({...graph.links[0],id:"other",to:"c"}); assert.throws(()=>check(graph),/one enabled outgoing/);
  graph=pair(); graph.nodes[0].position[0]=1000001; assert.throws(()=>check(graph),/position/);
  graph=pair(); graph.nodes[0].position[0]=NaN; assert.throws(()=>check(graph),/finite/);
  graph=pair(); graph.nodes=Array.from({length:65},(_,i)=>node("n"+i,i*12)); assert.throws(()=>check(graph),/at most/);
});
test("one-way traversal preview detects absent return, but accepts a directed return cycle", () => {
  const graph=pair(); graph.links[0].two_way=false;
  assert.match(check(graph).warnings[0].message,/no return path/);
  graph.nodes.push(node("c",24));
  graph.links.push({...graph.links[0],id:"bc",from:"b",to:"c"},{...graph.links[0],id:"ca",from:"c",to:"a"});
  assert.deepEqual(check(graph).warnings,[]);
});
test("disabled routes do not claim reachability", () => {
  const graph=pair(); graph.links[0].enabled=false;
  assert.equal(check(graph).warnings.filter(row=>/not on an enabled/.test(row.message)).length,2);
});
test("operation failures are atomic and referenced openings cannot be removed", () => {
  const graph=pair(), before=structuredClone(graph);
  assert.throws(()=>applyPortalOperations(graph,[{op:"update_node",id:"a",changes:{label:"Changed"}},{op:"remove_node",id:"b"}]),/links first/);
  assert.deepEqual(graph,before);
  assert.throws(()=>applyPortalOperations(graph,[{op:"update_node",id:"a",changes:{id:"replacement"}}]),/Unexpected/);
  const removed=applyPortalOperations(graph,[{op:"remove_link",id:"fold"},{op:"remove_node",id:"b"}]);
  assert.equal(removed.nodes.length,1); assert.equal(removed.links.length,0);
});
test("native parser rejects unsupported schema versions and unknown fields", () => {
  assert.throws(()=>checkPortalLayout(root,"(version:2,nodes:[],links:[])"),/Unsupported/);
  assert.throws(()=>checkPortalLayout(root,"(version:1,nodes:[],links:[],ignored:true)"),/Unexpected field/);
  const graph=pair(); graph.links[0].kind="TeleportToAnything"; assert.throws(()=>check(graph),/Unsupported/);
});

test("publication, reload, stale review, generic save validation and revert use real disk history", async t => {
  // An isolated root prevents tests from touching the live editor draft or user saves.
  fs.mkdirSync(path.join(root,"target"),{recursive:true});
  const fixture=fs.mkdtempSync(path.join(root,"target","portal-test-"));
  const copy=(relative)=>{ const target=path.join(fixture,relative); fs.mkdirSync(path.dirname(target),{recursive:true}); fs.copyFileSync(path.join(root,relative),target); };
  fs.mkdirSync(path.join(fixture,"tools"),{recursive:true});
  fs.cpSync(path.join(root,"tools/axiom-dev"),path.join(fixture,"tools/axiom-dev"),{recursive:true});
  for(const relative of portalSources) copy(relative);
  fs.mkdirSync(path.join(fixture,"assets"),{recursive:true});
  fs.writeFileSync(path.join(fixture,portalPath),"(version:1,nodes:[],links:[])\n");
  fs.mkdirSync(path.dirname(portalDirectory(fixture)),{recursive:true});
  fs.cpSync(portalDirectory(root),portalDirectory(fixture),{recursive:true});
  const {portalCommand:command}=await import(pathToFileURL(path.join(fixture,"tools/axiom-dev/portal-service.mjs")));
  const workspace=await import(pathToFileURL(path.join(fixture,"tools/axiom-dev/workspace.mjs")));
  const inspect=()=>command("inspect");
  const create=()=>{const graph=pair();return [...graph.nodes.map(node=>({op:"add_node",node})),...graph.links.map(link=>({op:"connect",link}))];};
  await t.test("preview has no write; apply and reload preserve exact graph",()=>{
    const before=inspect(); const request={version:before.version,operations:create()};
    const preview=command("preview",request); assert.equal(inspect().version,before.version);
    const saved=command("apply",{...request,approval:preview.approval});
    assert.deepEqual(inspect().graph,pair()); assert.ok(saved.bundle); assert.notEqual(saved.version,before.version);
    const restored=command("revert",{bundle:saved.bundle}); assert.deepEqual(restored.graph,before.graph); assert.equal(restored.version,before.version);
  });
  await t.test("altered approval and stale source cannot publish",()=>{
    const request={version:inspect().version,operations:create()}, preview=command("preview",request);
    const altered=structuredClone(request); altered.operations[0].node.label="Unreviewed";
    assert.throws(()=>command("apply",{...altered,approval:preview.approval}),/exact portal revision/);
    fs.appendFileSync(path.join(fixture,portalPath),"// concurrent author\n");
    assert.throws(()=>command("apply",{...request,approval:preview.approval}),/changed on disk/);
    assert.match(fs.readFileSync(path.join(fixture,portalPath),"utf8"),/concurrent author/);
  });
  await t.test("revert refuses concurrent edits",()=>{
    const request={version:inspect().version,operations:create()}, preview=command("preview",request);
    const saved=command("apply",{...request,approval:preview.approval});
    fs.appendFileSync(path.join(fixture,portalPath),"// another author\n");
    assert.throws(()=>command("revert",{bundle:saved.bundle}),/changed on disk/);
    assert.match(fs.readFileSync(path.join(fixture,portalPath),"utf8"),/another author/);
  });
  await t.test("generic source save cannot bypass native graph validation",()=>{
    const before=workspace.read(portalPath);
    const invalid=pair(); invalid.links[0].to="missing";
    assert.throws(()=>workspace.save(portalPath,{version:before.version,text:encodePortalLayout(invalid)}),/missing opening/);
    assert.equal(workspace.read(portalPath).version,before.version);
  });
  await t.test("schema plus layout publication is rejected before writing",()=>{
    const catalog=workspace.read(portalPath), schema=workspace.read(portalSources[0]);
    const graph=pair(); graph.nodes[0].label="Another label";
    assert.throws(()=>workspace.saveTextBundle([{path:portalPath,version:catalog.version,text:encodePortalLayout(graph)},
      {path:portalSources[0],version:schema.version,text:schema.text+"\n// future schema\n"}]),/rebuild the portal schema/);
    assert.equal(workspace.read(portalSources[0]).version,schema.version);
    assert.equal(workspace.read(portalPath).version,catalog.version);
  });
  await t.test("CLI and authenticated HTTP preview, publish and restore are identical",async()=>{
    const before=inspect(), request={version:before.version,operations:[{op:"update_node",id:"a",changes:{label:"HTTP-reviewed opening"}}]};
    const cli=spawnSync(process.execPath,[path.join(fixture,"tools/axiom-dev/portal-cli.mjs"),"preview"],
      {cwd:fixture,input:JSON.stringify(request),encoding:"utf8",windowsHide:true});
    assert.equal(cli.status,0,cli.stderr);
    const expected=JSON.parse(cli.stdout);
    const server=spawn(process.execPath,[path.join(fixture,"tools/axiom-dev/server.mjs")],
      {cwd:fixture,windowsHide:true,stdio:["ignore","pipe","pipe"]});
    try {
      const url=await new Promise((resolve,reject)=>{
        let output="";
        const timer=setTimeout(()=>reject(new Error("Server startup timed out")),10000);
        server.once("error",error=>{clearTimeout(timer);reject(error);});
        server.once("exit",code=>{clearTimeout(timer);reject(new Error("Server exited "+code));});
        server.stdout.on("data",chunk=>{output+=chunk;const match=output.match(/http:\/\/127\.0\.0\.1:\d+\/#\w+/);if(match){clearTimeout(timer);resolve(new URL(match[0]));}});
      });
      const call=async(action,payload)=>{
        const response=await fetch(url.origin+"/api/portal-layout"+(action?"/"+action:""),{
          method:payload?"POST":"GET",headers:{"Content-Type":"application/json",Origin:url.origin,"X-Axiom-Session":url.hash.slice(1)},
          body:payload?JSON.stringify(payload):undefined});
        return {status:response.status,body:await response.json()};
      };
      assert.equal((await fetch(url.origin+"/api/portal-layout")).status,403);
      assert.equal((await call("")).body.version,before.version);
      assert.equal((await call("preview",request)).body.approval,expected.approval);
      assert.equal((await call("apply",{...request,approval:"unreviewed"})).status,400);
      const applied=await call("apply",{...request,approval:expected.approval});
      assert.equal(applied.status,200,JSON.stringify(applied.body));
      assert.equal(inspect().graph.nodes.find(node=>node.id==="a").label,"HTTP-reviewed opening");
      assert.equal((await call("revert",{bundle:applied.body.bundle})).status,200);
      assert.equal(inspect().version,before.version);
    } finally {server.kill();}
  });
  // Fixture retained under target as failure evidence; no shared file cleanup.
});
