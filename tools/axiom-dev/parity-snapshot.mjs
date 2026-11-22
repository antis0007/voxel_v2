import { createHash } from "node:crypto";
import { execFile, execFileSync } from "node:child_process";
import { closeSync, copyFileSync, existsSync, mkdtempSync, openSync, readFileSync, renameSync, rmdirSync, rmSync, writeFileSync } from "node:fs";
import { lstat, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { publishBundle, revertBundle } from "./agent-bundles.mjs";
import { ensureDirectory, inside, root, state } from "./workspace.mjs";

const exec = promisify(execFile);
const gitOptions = { cwd: root, encoding: "utf8", windowsHide: true, maxBuffer: 16 * 1024 * 1024 };
const command = ["-c", "core.hooksPath=" + (process.platform === "win32" ? "NUL" : "/dev/null")];
export const digest = value => createHash("sha256").update(value).digest("hex");
const relative = (base, file) => path.relative(base, file).split(path.sep).join("/");
const buildInput = file => /^(?:crates\/|vendor\/game-networking-sockets-sys\/|assets\/|\.cargo\/|Cargo\.(?:toml|lock)$|rust-toolchain(?:\.toml)?$)/.test(file);
export const nativeCheckInput = file => buildInput(file) || /^tools\/axiom-dev\/(?:scenarios\/[^/]+\.json|factory-lab\.mjs)$/.test(file)
  || /^(?:docs\/research\/MOD_FEATURE_MATRIX\.md|docs\/MASTER_PLAN\.md)$/.test(file);
const checkInput = file => nativeCheckInput(file) || /^tools\/axiom-dev\/parity-[^/]+\.mjs$/.test(file);
const reference = file => /^docs\/.*\.md$/.test(file) || /^docs\/specs\/.*\.txt$/i.test(file) || file === "AGENTS.md";

async function inventory() {
  const { stdout } = await exec("git", [...command, "ls-files", "-z", "--cached", "--others", "--exclude-standard"], gitOptions);
  return [...new Set(stdout.split("\0").filter(file => checkInput(file) || reference(file)))].sort();
}

async function sourceBytes(base, file) {
  const target = path.resolve(base, file);
  if (!inside(base, target)) throw Error("Invalid snapshot path " + file);
  try {
    if (!(await lstat(target)).isFile() || relative(base, await realpath(target)) !== file) {
      throw Error("Snapshot inputs must be ordinary files inside the checkout: " + file);
    }
    return await readFile(target);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}

async function mapFiles(files, operation) {
  const result = new Map();
  for (let i = 0; i < files.length; i += 32) {
    for (const [file, value] of await Promise.all(files.slice(i, i + 32).map(async file => [file, await operation(file)]))) {
      if (value !== undefined) result.set(file, value);
    }
  }
  return result;
}

export async function captureSnapshot(directory, signal, include = () => true) {
  await mkdir(directory, { recursive: true });
  const files = (await inventory()).filter(include);
  for (const parent of new Set(files.map(file => path.dirname(file)))) {
    signal.throwIfAborted();
    await mkdir(path.join(directory, parent), { recursive: true });
  }
  const inputs = await mapFiles(files, async file => {
    signal.throwIfAborted();
    const bytes = await sourceBytes(root, file);
    if (!bytes) return;
    const target = path.join(directory, file);
    await writeFile(target, bytes, { flag: "wx" });
    return digest(bytes);
  });
  return { directory, inputs };
}

export function checkedInputs(snapshot, changes = new Map(), include = checkInput) {
  return Object.fromEntries([...snapshot.inputs].filter(([file]) => include(file))
    .map(([file, hash]) => [file, changes.has(file) ? digest(changes.get(file)) : hash]));
}

export async function sourceConflict(inputs, include = checkInput) {
  const files = (await inventory()).filter(include);
  const indexed = new Set(files);
  const removed = Object.keys(inputs).find(file => !indexed.has(file));
  if (removed) return removed;
  // Check additions first and stop hashing once a batch establishes drift.
  const ordered = [...files.filter(file => !Object.hasOwn(inputs, file)), ...Object.keys(inputs)];
  for (let i = 0; i < ordered.length; i += 32) {
    const batch = ordered.slice(i, i + 32);
    const hashes = await Promise.all(batch.map(async file => {
      const bytes = await sourceBytes(root, file);
      return bytes && digest(bytes);
    }));
    const conflict = batch.find((file, index) => hashes[index] !== inputs[file]);
    if (conflict) return conflict;
  }
}

// Used only to wake blocked discovery. Exact content hashes still gate edits.
export async function sharedSourceRevision(include = checkInput) {
  const current = await mapFiles((await inventory()).filter(include), async file => {
    const stat = await lstat(path.join(root, file), { bigint: true }).catch(error => {
      if (error.code !== "ENOENT") throw error;
    });
    return stat?.isFile() ? `${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}` : undefined;
  });
  return digest(JSON.stringify([...current]));
}

function git(args, { index, input } = {}) {
  const env = { ...process.env };
  delete env.GIT_INDEX_FILE;
  if (index) env.GIT_INDEX_FILE = index;
  return execFileSync("git", [...command, ...args], { ...gitOptions, env, input, stdio: ["pipe", "pipe", "pipe"] }).trim();
}

const candidateStatus = paths => git(["--literal-pathspecs", "--no-optional-locks", "status", "--porcelain", "--untracked-files=all", "--", ...paths]);
const commitUnavailable = (message, retryable = false) => Object.assign(new Error(message), { code: "AXIOM_COMMIT_UNAVAILABLE", retryable });

export function publishMissionCommit(title, run, changes, onPrepared) {
  const inputs = new Map(changes.map(change => [change.path, change.version]));
  const directory = mkdtempSync(path.join(ensureDirectory(path.join(state, "workflow-commits")), "publish-"));
  try {
    const commit = publishCommit(directory, title, run, { inputs },
      new Map(changes.map(change => [change.path, Buffer.from(change.text)])), onPrepared, "mission");
    return { commit };
  } catch (error) {
    if (error.code !== "AXIOM_COMMIT_UNAVAILABLE") throw error;
    return { reason: error.message };
  } finally {
    try { rmdirSync(directory); }
    catch (error) { process.stderr.write("Mission commit cleanup: " + error.message + "\n"); }
  }
}

export function assertMissionCommit(commit, paths) {
  if (!/^[a-f0-9]{40,64}$/.test(commit)) throw new Error("Invalid prepared mission commit.");
  try {
    git(["merge-base", "--is-ancestor", commit, "HEAD"]);
    if (candidateStatus(paths)) throw new Error("The checkout index or candidate files have changed.");
  } catch (error) {
    throw new Error("Inspect prepared commit " + commit + " and its index before continuing: " + error.message.split("\n")[0]);
  }
}

// Build a commit from the exact candidate blobs. Hold Git's real index lock so
// staging performed by any other process is preserved, and compare-and-swap HEAD.
export function publishCommit(directory, id, run, snapshot, changes, onPrepared, kind = "parity") {
  const paths = [...changes.keys()];
  let ref;
  try { ref = git(["symbolic-ref", "HEAD"]); }
  catch (error) {
    if (error.status !== 128) throw error;
    throw commitUnavailable("Git could not resolve the active branch.");
  }
  const head = git(["rev-parse", ref]);
  const index = git(["rev-parse", "--path-format=absolute", "--git-path", "index"]);
  const lock = index + ".lock";
  const treeIndex = path.join(directory, "commit-index");
  const nextIndex = path.join(directory, "shared-index");
  let fd;
  try { fd = openSync(lock, "wx"); }
  catch (error) {
    if (error.code !== "EEXIST") throw error;
    throw commitUnavailable("Git index is busy.", true);
  }
  let bundle, commit, advanced = false, installed = false;
  try {
    if (candidateStatus(paths)) {
      throw commitUnavailable("Candidate files already contain staged or uncommitted work.");
    }
    const entries = paths.map(file => {
      const before = readFileSync(path.join(root, file));
      const current = digest(before);
      if (current !== snapshot.inputs.get(file)) throw Error("Candidate source changed: " + file);
      const entry = git(["--literal-pathspecs", "ls-tree", head, "--", file]);
      const [, mode, baseBlob] = /^(100(?:644|755)) blob ([a-f0-9]+)\t/.exec(entry) ?? [];
      if (!mode) throw Error("Candidate file is missing from HEAD: " + file);
      if (git(["hash-object", "--path=" + file, "--stdin"], { input: before }) !== baseBlob) {
        throw commitUnavailable("Candidate differs from the committed source: " + file);
      }
      const blob = git(["hash-object", "-w", "--path=" + file, "--stdin"], { input: changes.get(file) });
      return { file, current, mode, blob };
    });
    git(["read-tree", head], { index: treeIndex });
    if (existsSync(index)) copyFileSync(index, nextIndex);
    else git(["read-tree", head], { index: nextIndex });
    const update = entries.map(({ file, mode, blob }) => mode + " " + blob + "\t" + file + "\0").join("");
    for (const target of [treeIndex, nextIndex]) git(["update-index", "-z", "--index-info"], { index: target, input: update });
    const tree = git(["write-tree"], { index: treeIndex });
    const subject = kind === "mission" ? id : "Fix verified parity " + id;
    const trailer = kind === "mission" ? "Mission run " : "Parity run ";
    commit = git(["commit-tree", tree, "-p", head, "-m", subject, "-m", trailer + run]);
    writeFileSync(fd, readFileSync(nextIndex));
    if (git(["symbolic-ref", "HEAD"]) !== ref) throw Error("The active branch changed during publication.");
    bundle = publishBundle(entries.map(({ file, current }) => ({
      path: file, version: current, text: changes.get(file).toString("utf8"),
    })), run).id;
    onPrepared({ bundle, commit });
    git(["update-ref", "-m", kind + ": " + id, ref, commit, head]);
    advanced = true;
    closeSync(fd);
    renameSync(lock, index);
    installed = true;
    return commit;
  } catch (error) {
    if (advanced) throw Error("Commit " + commit + " was created, but the checkout index could not be updated. " + error.message);
    if (bundle) revertBundle(bundle);
    throw error;
  } finally {
    if (!advanced) closeSync(fd);
    if (!installed) rmSync(lock, { force: true });
    for (const file of [treeIndex, nextIndex]) rmSync(file, { force: true });
  }
}
