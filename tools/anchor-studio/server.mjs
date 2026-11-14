import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { mkdir, readFile, readdir, rename, writeFile, stat } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const web = dirname(fileURLToPath(import.meta.url));
const root = resolve(web, "../..");
const storage = join(root, "target", "anchor-studio");
const library = join(root, "assets", "anchor-studio", "designs");
const exe = process.platform === "win32" ? ".exe" : "";
const baker = join(storage, "compiler", "release-fast", "anchor_studio" + exe);
const port = Number(process.env.ANCHOR_STUDIO_PORT ?? "4377");
const origin = "http://127.0.0.1:" + port;
const files = new Map([
    ["/", ["index.html", "text/html"]], ["/style.css", ["style.css", "text/css"]],
    ["/editor.js", ["editor.js", "text/javascript"]], ["/design.js", ["design.js", "text/javascript"]],
    ["/viewport.js", ["viewport.js", "text/javascript"]],
]);

let reviewing = false;
await mkdir(library, { recursive: true });
await stat(baker).catch(() => { throw Error("Build the Anchor Studio baker with tools/Open-Anchor-Studio.ps1."); });

function run(command, args, { input, env, timeout = 20000, max = 6 * 1024 * 1024, progress } = {}) {
    return new Promise((done, fail) => {
        const child = spawn(command, args, { cwd: root, windowsHide: true, env });
        const timer = setTimeout(() => child.kill(), timeout);
        const output = [];
        let bytes = 0, notes = "", failure;
        child.stdout.on("data", chunk => {
            bytes += chunk.length;
            if (bytes > max) {
                failure = Error("Output was larger than the tool can display.");
                child.kill();
            } else {
                output.push(chunk);
            }
        });
        child.stderr.on("data", chunk => {
            notes = (notes + chunk.toString()).slice(-6000);
            progress?.(notes);
        });
        child.on("error", error => { clearTimeout(timer); fail(error); });
        child.on("close", code => {
            clearTimeout(timer);
            if (code === 0 && !failure) done(Buffer.concat(output));
            else fail(failure ?? Error(notes.trim() || "The local tool did not finish."));
        });
        child.stdin.on("error", () => {});
        child.stdin.end(input);
    });
}

function json(res, value, code = 200) {
    res.writeHead(code, { "Content-Type": "application/json", "Cache-Control": "no-store" });
    res.end(JSON.stringify(value));
}

async function body(req) {
    if (req.headers["content-type"] !== "application/json") throw Error("Expected a Studio request.");
    let length = 0;
    const chunks = [];
    for await (const chunk of req) {
        length += chunk.length;
        if (length > 64 * 1024) throw Error("Design request is too large.");
        chunks.push(chunk);
    }
    const data = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (typeof data.source !== "string" || Buffer.byteLength(data.source) > 16384) {
        throw Error("The design must be a VXPLAN file up to 16 KB.");
    }
    if (!data.source.trimStart().startsWith("VXPLAN 1")) throw Error("Expected a VXPLAN 1 design.");
    return data;
}

function projectFile(name) {
    if (!/^[A-Za-z0-9 _-]{1,40}$/.test(name) || name !== name.trim() || /^(CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])$/i.test(name)) {
        throw Error("Use 1–40 letters, numbers, spaces, dashes or underscores for the project name.");
    }
    return join(library, name + ".vxplan");
}

function resolution(data) {
    if (![1, 0.5, 0.25].includes(data.scale)) throw Error("Choose a valid block size.");
    return String(data.scale);
}

async function handle(req, res) {
    if (req.headers.host !== "127.0.0.1:" + port || req.headers.origin && req.headers.origin !== origin) {
        return json(res, { error: "Open the local Studio address." }, 403);
    }
    const url = new URL(req.url, origin);
    const path = url.pathname;
    if (req.method === "GET" && files.has(path)) {
        const [file, type] = files.get(path);
        res.writeHead(200, { "Content-Type": type + "; charset=utf-8", "Cache-Control": "no-store",
            "Content-Security-Policy": "default-src 'self'; img-src 'self' blob:; style-src 'self'; object-src 'none'; frame-ancestors 'none'" });
        return res.end(await readFile(join(web, file)));
    }
    if (req.method === "GET" && path === "/api/library") {
        const names = (await readdir(library, { withFileTypes: true }))
            .filter(entry => entry.isFile() && entry.name.endsWith(".vxplan"))
            .map(entry => basename(entry.name, ".vxplan")).sort((a, b) => a.localeCompare(b));
        return json(res, { names });
    }
    if (path.startsWith("/api/library/")) {
        const target = projectFile(decodeURIComponent(path.slice("/api/library/".length)));
        if (req.method === "GET") return json(res, { source: await readFile(target, "utf8") });
        if (req.method === "PUT") {
            const { source } = await body(req);
            const temp = join(library, randomUUID() + ".tmp");
            await writeFile(temp, source, { flag: "wx" });
            await rename(temp, target);
            return json(res, { saved: true });
        }
    }
    if (req.method === "POST" && path === "/api/bake") {
        const data = await body(req);
        const output = await run(baker, ["bake", resolution(data)], { input: data.source });
        res.writeHead(200, { "Content-Type": "application/octet-stream", "Cache-Control": "no-store" });
        return res.end(output);
    }
    if (req.method === "POST" && path === "/api/review") {
        if (reviewing) return json(res, { error: "A city preview is already running." }, 409);
        const { source } = await body(req);
        if (reviewing) return json(res, { error: "A city preview is already running." }, 409);
        reviewing = true;
        try {
            const output = await run(baker, ["city"], { input: source, timeout: 90000 });
            res.writeHead(200, { "Content-Type": "application/octet-stream", "Cache-Control": "no-store" });
            return res.end(output);
        } finally {
            reviewing = false;
        }
    }
    return json(res, { error: "This Studio page does not exist." }, 404);
}

createServer((req, res) => {
    handle(req, res).catch(error => {
        if (!res.headersSent) json(res, { error: error.message.replace(/^Error:\s*/i, "") }, 400);
        else res.end();
    });
}).listen(port, "127.0.0.1", () => {
    console.log("Anchor Studio is open at " + origin);
    if (process.argv.includes("--open") && process.platform === "win32") {
        const browser = spawn("explorer.exe", [origin], { windowsHide: true, detached: true, stdio: "ignore" });
        browser.on("error", () => console.log("Open " + origin + " in your browser."));
        browser.unref();
    }
});
