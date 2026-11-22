// The desktop launcher always reconnects here; ordinary browsing needs no restart.
let revision;
let dirty = false;
let polling = false;
const status = document.createElement('button');
status.type = 'button';
status.className = 'forge-chip axiom-app-status';
status.textContent = 'Connecting…';
status.title = 'One local AXIOM Dev worker. Updates reconnect automatically.';
status.style.cssText = 'position:fixed;right:16px;bottom:12px;z-index:80;padding:8px 12px;background:#19303c;color:#dcebf3;border:1px solid #35596a;border-radius:8px;font:12px system-ui;';
const footer = document.querySelector('body > footer');
if (footer) {
  status.style.cssText = 'margin-left:auto;padding:2px 8px;border-radius:2px;font:11px system-ui;white-space:nowrap;';
  footer.append(status);
} else document.body.append(status);
document.addEventListener('input', event => {
  if (!event.target.closest('.catalog-filter') && event.target.matches('textarea,select,input:not([type=search]),[contenteditable=true]')) dirty = true;
}, true);
status.onclick = () => {
  if (dirty && !confirm('Reload the app? Unsaved text on this page will be lost.')) return;
  location.reload();
};
async function poll() {
  if (polling) return;
  polling = true;
  try {
    const response = await fetch('/api/app', { headers: { 'X-Axiom-Session': location.hash.slice(1) }, signal: AbortSignal.timeout(3500) });
    if (!response.ok) throw new Error('Connection unavailable');
    const app = await response.json();
    revision ??= app.revision;
    const changed = revision !== app.revision;
    if (changed && !app.updating && !dirty && !document.querySelector('dialog[open]')) { location.reload(); return; }
    status.textContent = app.updating ? 'Updating after active work…' : changed ? 'Update ready · reload when ready' : 'Connected · one instance';
    status.title = 'AXIOM Dev · ' + app.origin + (dirty ? ' · Your unsaved input is protected from automatic reload.' : ' · Updates apply automatically.');
  } catch {
    status.textContent = 'Reconnecting automatically…';
  } finally { polling = false; }
}
void poll();
setInterval(poll, 2000);
