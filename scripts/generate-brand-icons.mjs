import { mkdir, readdir, copyFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

// Tauri rasterizes the same SVG used by the UI. Keep future mobile assets alone.
const output = resolve(".tools/infrawerf-icons");
await mkdir(output, { recursive: true });
const result = spawnSync(process.execPath, ["node_modules/@tauri-apps/cli/tauri.js", "icon", "public/brand/infrawerf-mark.svg", "--output", output], { stdio: "inherit" });
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);
for (const item of await readdir(output, { withFileTypes: true })) {
  if (item.isFile() && /\.(png|ico|icns)$/.test(item.name)) {
    await copyFile(resolve(output, item.name), resolve("src-tauri/icons", item.name));
  }
}
