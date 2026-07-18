import { readFile, stat } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const manifest = JSON.parse(await readFile(resolve(root, "dist/.vite/manifest.json"), "utf8"));
const entry = manifest["index.html"];
const renderer = manifest["src/components/GltfRenderer.tsx"];
const vrmKey = renderer?.dynamicImports?.find((key) => key.includes("three-vrm.module.js"));
const vrm = vrmKey ? manifest[vrmKey] : null;
const workspaceKeys = [
  "src/components/CreatorStudio.tsx",
  "src/components/AgentWorkspace.tsx",
  "src/components/AutomationWorkspace.tsx",
  "src/components/AiCreatorWorkspace.tsx",
  "src/components/DataProtection.tsx",
];

if (!entry?.isEntry || !renderer?.isDynamicEntry || !vrm?.isDynamicEntry) {
  throw new Error("Desktop bundle boundaries are missing from the Vite manifest");
}
if (!entry.dynamicImports?.includes("src/components/GltfRenderer.tsx")) {
  throw new Error("The GLTF renderer must remain a direct lazy dependency of the desktop entry");
}
for (const key of workspaceKeys) {
  if (!manifest[key]?.isDynamicEntry || !entry.dynamicImports?.includes(key)) {
    throw new Error(`${key} must remain a direct lazy dependency of the desktop entry`);
  }
}

async function graphSize(rootKey, ignoredKeys = new Set()) {
  const visited = new Set();
  async function visit(key) {
    if (visited.has(key) || ignoredKeys.has(key)) return 0;
    visited.add(key);
    const chunk = manifest[key];
    if (!chunk) throw new Error(`Bundle manifest references missing chunk ${key}`);
    const own = (await stat(resolve(root, "dist", chunk.file))).size;
    const imports = await Promise.all((chunk.imports ?? []).map(visit));
    return own + imports.reduce((total, size) => total + size, 0);
  }
  return visit(rootKey);
}

const budgets = [
  ["desktop entry", (await stat(resolve(root, "dist", entry.file))).size, 350_000],
  ...await Promise.all(workspaceKeys.map(async (key) => [`lazy workspace ${key}`, await graphSize(key, new Set(["index.html"])), 50_000])),
  ["lazy GLTF renderer graph", await graphSize("src/components/GltfRenderer.tsx", new Set(["index.html"])), 650_000],
  ["optional VRM runtime", await graphSize(vrmKey, new Set(renderer.imports ?? [])), 150_000],
];

for (const [label, size, maximumBytes] of budgets) {
  if (size > maximumBytes) {
    throw new Error(`${label} exceeds its ${maximumBytes}-byte budget: ${size} bytes`);
  }
  console.log(`${label}: ${size}/${maximumBytes} bytes`);
}
