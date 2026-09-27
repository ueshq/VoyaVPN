// Builds the static site into dist/: Vite compiles the stylesheet and copies
// public/, then every page is rendered to HTML with React on Node.
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build, runnerImport } from "vite";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(root, "dist");

await build({ root, logLevel: "warn" });

const manifestPath = resolve(dist, ".vite/manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
const css = `/${manifest["src/site.css"].file}`;
// The manifest only told us the hashed name; it is not served.
await rm(resolve(dist, ".vite"), { recursive: true });

const { module } = await runnerImport(resolve(root, "src/render.tsx"), {
  root,
  configFile: false,
  logLevel: "warn",
});

for (const { file, contents } of module.renderSite(css)) {
  const path = resolve(dist, file);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, contents);
}

console.log(`built ${module.renderSite(css).length} files and ${css} into dist/`);
