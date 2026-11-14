import { readFileSync } from "node:fs";

const bridge = `<script>
// Sandboxed exports use downloads; direct filesystem handles are unavailable.
Object.defineProperty(window, 'showOpenFilePicker', {value:undefined});
Object.defineProperty(window, 'showSaveFilePicker', {value:undefined});
addEventListener('message', event => {
  if(event.source !== parent || event.origin !== new URL(location.href).origin) return;
  const data=event.data;
  if(data?.type !== 'axiom-normal-albedo' || !(data.blob instanceof Blob)
    || data.blob.type !== 'image/png' || data.blob.size > 24*1024*1024) return;
  const transfer=new DataTransfer();
  transfer.items.add(new File([data.blob], String(data.name).replace(/[\\/]/g,'_'), {type:'image/png'}));
  const input=document.getElementById('albedo-file');
  input.files=transfer.files;
  input.dispatchEvent(new Event('change', {bubbles:true}));
});
</script>`;

export function serveNormalLab(pathname, response) {
  if (pathname !== "/normal-lab.html") return false;
  const source = readFileSync(new URL("./vendor/pixel-art-lighter-3.5.22.html", import.meta.url), "utf8");
  response.writeHead(200, {
    "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff", "Referrer-Policy": "no-referrer",
    "Content-Security-Policy": "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src blob: data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; sandbox allow-scripts allow-downloads; frame-ancestors 'self'",
  });
  response.end(source.replace('<script>', bridge + '<script>'));
  return true;
}
