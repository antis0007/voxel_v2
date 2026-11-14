import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, statSync } from "node:fs";
import { createHash, randomBytes } from "node:crypto";
import { deflateSync } from "node:zlib";
import path from "node:path";
import { root, state, ensureDirectory } from "./workspace.mjs";

let busy = false;
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
function chunk(type, data) {
  const name = Buffer.from(type), body = Buffer.concat([name, data]);
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  const header = Buffer.alloc(4), footer = Buffer.alloc(4);
  header.writeUInt32BE(data.length); footer.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([header, body, footer]);
}

export function nativeRgbaPng(bytes) {
  if (bytes.subarray(0, 6).toString() !== "AXSP01") throw new Error("Invalid native sprite output");
  const width = bytes.readUInt32LE(6), height = bytes.readUInt32LE(10);
  if (!width || !height || width * height > 1048576 || bytes.length !== 14 + width * height * 4) {
    throw new Error("Invalid native sprite dimensions");
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width); header.writeUInt32BE(height, 4); header[8] = 8; header[9] = 6;
  const rows = Buffer.alloc(height * (width * 4 + 1));
  for (let y = 0; y < height; y++) bytes.copy(rows, y * (width * 4 + 1) + 1, 14 + y * width * 4, 14 + (y + 1) * width * 4);
  return { width, height, png: Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header), chunk("IDAT", deflateSync(rows)), chunk("IEND", Buffer.alloc(0))]) };
}

export async function renderSavedToolArt() {
  if (busy) throw new Error("A saved-mask render is already running");
  const directory = ensureDirectory(path.join(state, "native-art", randomBytes(8).toString("hex")));
  const input = path.join(directory, "sprites16.txt"), raw = path.join(directory, "tools.rgba");
  const source = readFileSync(path.join(root, "assets/tools/sprites16.txt"));
  writeFileSync(input, source);
  busy = true;
  try {
    await new Promise((resolve, reject) => {
      const child = spawn("cargo", ["run", "--offline", "--locked", "--quiet", "-p", "engine_tools",
        "--example", "forge_sprite_preview", "--", input, raw], { cwd: root, windowsHide: true,
        env: { ...process.env, CARGO_TARGET_DIR: path.join(root, "target/forge-native-tools") }, stdio: ["ignore", "ignore", "pipe"] });
      let errors = "", expired = false;
      child.stderr.on("data", bytes => { errors = (errors + bytes).slice(-3000); });
      const timeout = setTimeout(() => { expired = true; child.kill(); }, 180000);
      child.once("error", error => { clearTimeout(timeout); reject(error); });
      child.once("close", code => {
        clearTimeout(timeout);
        if (expired) reject(new Error("Native mask renderer build exceeded three minutes"));
        else if (code !== 0) reject(new Error("Native mask renderer failed: " + errors));
        else resolve();
      });
    });
    const image = nativeRgbaPng(readFileSync(raw)), output = path.join(directory, "tools.png");
    writeFileSync(output, image.png);
    const binary = path.join(root, "target/forge-native-tools/debug/examples/forge_sprite_preview" + (process.platform === "win32" ? ".exe" : ""));
    const evidence = { output, binary, buildTime: statSync(binary).mtime.toISOString(),
      sourceSha256: sha(source), rendererSha256: sha(readFileSync(binary)), imageSha256: sha(image.png),
      width: image.width, height: image.height,
      scope: "Saved masks rendered by the game's native engine_tools composer. This is not a rebuilt game executable." };
    writeFileSync(path.join(directory, "evidence.json"), JSON.stringify(evidence, null, 2));
    return { ...evidence, png: image.png.toString("base64") };
  } finally { busy = false; }
}
