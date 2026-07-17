import { chmod, cp, mkdir, rm } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = join(root, "apps/desktop/src-tauri/binaries");
const target = process.env.TAURI_ENV_TARGET_TRIPLE ?? (await rustHostTriple());
const sidecars = [
  { packageName: "nimora-user-code-worker", binaryName: "nimora-user-code-worker" },
  { packageName: "nimora-model-importer", binaryName: "nimora-model-importer-worker" },
];

await mkdir(outputDirectory, { recursive: true });
for (const sidecar of sidecars) {
  await prepareSidecar(sidecar);
}

async function prepareSidecar({ packageName, binaryName }) {
  await execFileAsync("cargo", ["build", "-p", packageName, "--release"], { cwd: root });
  const executableSuffix = process.platform === "win32" ? ".exe" : "";
  const source = join(root, "target/release", `${binaryName}${executableSuffix}`);
  const destination = join(outputDirectory, `${binaryName}-${target}${executableSuffix}`);
  await rm(destination, { force: true });
  await cp(source, destination);
  if (process.platform !== "win32") await chmod(destination, 0o755);
  console.log(`Prepared ${binaryName} for ${target}`);
}

async function rustHostTriple() {
  const { stdout } = await execFileAsync("rustc", ["-vV"], { cwd: root });
  const line = stdout.split("\n").find((entry) => entry.startsWith("host:"));
  if (!line) throw new Error("Unable to determine the Rust host target triple");
  return line.slice("host:".length).trim();
}
