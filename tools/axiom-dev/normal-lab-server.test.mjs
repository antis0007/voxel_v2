import { test } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { serveNormalLab } from './normal-lab-server.mjs';

function page() {
  const result = {};
  assert.equal(serveNormalLab('/normal-lab.html', {
    writeHead: (status, headers) => Object.assign(result, {status, headers}),
    end: html => { result.html = html; },
  }), true);
  return result;
}

test('offline source retains its licence and recorded hash', () => {
  const source = readFileSync(new URL('./vendor/pixel-art-lighter-3.5.22.html', import.meta.url));
  const provenance = JSON.parse(readFileSync(new URL('./vendor/pixel-art-lighter-provenance.json', import.meta.url)));
  assert.equal(createHash('sha256').update(source).digest('hex'), provenance.offline_sha256);
  assert.match(source.toString(), /MIT License/);
  assert.doesNotMatch(source.toString(), /cloudflareinsights|challenge-platform|<script[^>]*src=/);
});

test('lab response blocks networking and same-origin access', () => {
  const result = page();
  assert.equal(result.status, 200);
  const csp = result.headers['Content-Security-Policy'];
  assert.match(csp, /connect-src 'none'/);
  assert.match(csp, /sandbox allow-scripts allow-downloads;/);
  assert.doesNotMatch(csp, /allow-same-origin/);
  for (const script of result.html.matchAll(/<script[^>]*>([\s\S]*?)<\/script\s*>/g)) new vm.Script(script[1]);
  assert.equal(serveNormalLab('/api/save', {}), false);
});

test('bridge accepts only a parent PNG from the editor origin', () => {
  const script = page().html.match(/<script>([\s\S]*?)<\/script>/)[1];
  let listener, changed = 0;
  const parent = {}, input = {dispatchEvent: () => changed++};
  class Transfer {
    files = [];
    items = {add: file => this.files.push(file)};
  }
  vm.runInNewContext(script, {
    window: {}, parent, URL, Blob, File, Event,
    location: {href:'http://127.0.0.1:1234/normal-lab.html'},
    addEventListener: (_, fn) => { listener = fn; },
    document: {getElementById: id => { assert.equal(id,'albedo-file'); return input; }},
    DataTransfer: Transfer,
  });
  const data = {type:'axiom-normal-albedo',blob:new Blob(['test'],{type:'image/png'}),name:'copper.png'};
  listener({source:{},origin:'http://127.0.0.1:1234',data});
  listener({source:parent,origin:'http://example.com',data});
  listener({source:parent,origin:'http://127.0.0.1:1234',data:{...data,blob:new Blob(['test'],{type:'text/html'})}});
  assert.equal(changed,0);
  listener({source:parent,origin:'http://127.0.0.1:1234',data});
  assert.equal(changed,1);
  assert.equal(input.files[0].name,'copper.png');
});
