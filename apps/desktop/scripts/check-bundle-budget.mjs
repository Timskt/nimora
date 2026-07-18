import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(await readFile(resolve(root, "dist/.vite/manifest.json"), "utf8"));
const entry = manifest["index.html"];
const renderer = manifest["src/components/GltfRenderer.tsx"];

if (!entry?.isEntry || !renderer?.isDynamicEntry) {
  throw new Error("Desktop bundle boundaries are missing from the Vite manifest");
}
if (!entry.dynamicImports?.includes("src/components/GltfRenderer.tsx")) {
  throw new Error("The GLTF renderer must remain a direct lazy dependency of the desktop entry");
}

const budgets = [
  ["desktop entry", entry.file, 350_000],
  ["lazy GLTF renderer", renderer.file, 650_000],
];

for (const [label, file, maximumBytes] of budgets) {
  const { size } = await stat(resolve(root, "dist", file));
  if (size > maximumBytes) {
    throw new Error(`${label} exceeds its ${maximumBytes}-byte budget: ${size} bytes`);
  }
  console.log(`${label}: ${size}/${maximumBytes} bytes`);
}
