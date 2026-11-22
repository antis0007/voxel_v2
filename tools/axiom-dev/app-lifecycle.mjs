import { createHash, randomBytes } from 'node:crypto';
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { state, ensureDirectory } from './workspace.mjs';

export const appPort = 43170;
export function appToken() {
  ensureDirectory(state);
  const file = path.join(state, 'app-session-token');
  try {
    const value = readFileSync(file, 'utf8').trim();
    if (!/^[a-f0-9]{48}$/.test(value)) throw new Error('Invalid app session key. Retain it for inspection.');
    return value;
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    const value = randomBytes(24).toString('hex');
    try { writeFileSync(file, value, { flag: 'wx', mode: 0o600 }); }
    catch (writeError) { if (writeError.code !== 'EEXIST') throw writeError; return appToken(); }
    return value;
  }
}

// Metadata polling catches atomic editor saves as well as in-place changes.
// No game source or generated logs are watched, so ordinary agent work cannot restart the app.
export function appRevision() {
  const directory = new URL('.', import.meta.url);
  const hash = createHash('sha256');
  for (const name of readdirSync(directory).filter(name => /\.(mjs|css|html)$/.test(name) && !/\.test\.mjs$/.test(name)).sort()) {
    const stat = statSync(new URL(name, directory));
    hash.update(name + ':' + stat.size + ':' + stat.mtimeMs + ';');
  }
  return hash.digest('hex').slice(0, 20);
}

// Only imported server modules need a worker restart. Browser-only JS/CSS/HTML
// can refresh immediately, even while an agent is working.
export function backendRevision() {
  const hash = createHash('sha256');
  const visited = new Set();
  const visit = url => {
    if (visited.has(url.href)) return;
    visited.add(url.href);
    const source = readFileSync(url, 'utf8');
    hash.update(url.href).update(source);
    for (const match of source.matchAll(/(?:from\s*|import\s*\(?\s*)["'](\.\.?\/[^"']+\.mjs)["']/g)) visit(new URL(match[1], url));
  };
  visit(new URL('./server.mjs', import.meta.url));
  return hash.digest('hex').slice(0, 20);
}
