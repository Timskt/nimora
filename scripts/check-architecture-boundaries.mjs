import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const violations = [];

function findForbiddenImports(source, forbiddenPrefixes) {
  const matches = [];
  const importPattern = /(?:from\s+|import\s*\(|require\s*\()(["'])([^"']+)\1/g;
  for (const match of source.matchAll(importPattern)) {
    if (forbiddenPrefixes.some((prefix) => match[2] === prefix || match[2].startsWith(`${prefix}/`))) {
      matches.push(match[2]);
    }
  }
  return matches;
}

function findForbiddenCargoDependencies(manifest, forbiddenNames) {
  const matches = [];
  const dependencyPattern = /^([a-zA-Z0-9_-]+)(?:\.[a-zA-Z0-9_-]+)?\s*=/gm;
  for (const match of manifest.matchAll(dependencyPattern)) {
    if (forbiddenNames.has(match[1])) matches.push(match[1]);
  }
  return matches;
}

function findForbiddenRustHostSymbols(source) {
  const forbidden = ["tauri::", "use tauri", "State<'", "AppHandle", "#[tauri::command]"];
  return forbidden.filter((symbol) => source.includes(symbol));
}

function assertDetectorCoverage() {
  if (!findForbiddenImports('import { invoke } from "@tauri-apps/api/core";', ["@tauri-apps"]).length) {
    throw new Error("architecture detector failed its TypeScript self-check");
  }
  if (!findForbiddenCargoDependencies("rusqlite.workspace = true\n", new Set(["rusqlite"])).length) {
    throw new Error("architecture detector failed its Cargo self-check");
  }
  if (!findForbiddenRustHostSymbols("fn command(state: State<'_, Host>) {}\n").length) {
    throw new Error("architecture detector failed its Rust host-symbol self-check");
  }
}

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...await walk(target));
    else files.push(target);
  }
  return files;
}

async function checkDesktopUi() {
  const sourceRoot = path.join(root, "apps/desktop/src");
  const allowedAdapter = path.join(sourceRoot, "platform/desktop.ts");
  for (const file of await walk(sourceRoot)) {
    if (!/\.[cm]?[jt]sx?$/.test(file) || file === allowedAdapter) continue;
    const source = await readFile(file, "utf8");
    for (const dependency of findForbiddenImports(source, ["@tauri-apps"])) {
      violations.push(`${path.relative(root, file)} imports ${dependency}; UI must use platform/desktop.ts`);
    }
  }
}

async function checkRustLayers() {
  const rules = new Map([
    ["runtime-core", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["agent-runtime", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["automation-runtime", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["user-code-policy", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["skill-runtime", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["skill-worker", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["user-code-worker", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
    ["module-agent-adapter", ["rusqlite", "tauri", "reqwest", "nimora-agent-provider-worker"]],
  ]);
  for (const [crate, forbidden] of rules) {
    const manifestPath = path.join(root, "crates", crate, "Cargo.toml");
    const manifest = await readFile(manifestPath, "utf8");
    for (const dependency of findForbiddenCargoDependencies(manifest, new Set(forbidden))) {
      violations.push(`crates/${crate}/Cargo.toml depends on forbidden boundary ${dependency}`);
    }
  }
}

async function checkDesktopApplicationModules() {
  const modules = ["apps/desktop/src-tauri/src/asset_selection.rs"];
  for (const relativePath of modules) {
    const source = await readFile(path.join(root, relativePath), "utf8");
    for (const symbol of findForbiddenRustHostSymbols(source)) {
      violations.push(`${relativePath} references ${symbol}; application modules must remain Tauri-free`);
    }
  }
}

assertDetectorCoverage();
await Promise.all([checkDesktopUi(), checkRustLayers(), checkDesktopApplicationModules()]);

if (violations.length) {
  console.error("Architecture boundary violations:\n" + violations.map((item) => `- ${item}`).join("\n"));
  process.exitCode = 1;
} else {
  console.log("Architecture boundaries: passed");
}
