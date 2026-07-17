import { cp, mkdir, rm } from "node:fs/promises";
import { chmod } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = join(root, "apps/desktop/src-tauri/binaries");
const target = process.env.TAURI_ENV_TARGET_TRIPLE ?? (await rustHostTriple());
const binaryName = process.platform === "win32"
  ? `nimora-user-code-worker-${target}.exe`
  : `nimora-user-code-worker-${target}`;
const source = join(root, "target/release", process.platform === "win32"
  ? "nimora-user-code-worker.exe"
  : "nimora-user-code-worker");
const destination = join(outputDirectory, binaryName);

await execFileAsync("cargo", ["build", "-p", "nimora-user-code-worker", "--release"], { cwd: root });
await mkdir(outputDirectory, { recursive: true });
await rm(destination, { force: true });
await cp(source, destination);
if (process.platform !== "win32") {
  await chmod(destination, 0o755);
}
console.log(`Prepared Nimora user-code sidecar for ${target}`);

async function rustHostTriple() {
  const { stdout } = await execFileAsync("rustc", ["-vV"], { cwd: root });
  const line = stdout.split("\n").find((entry) => entry.startsWith("host:"));
  if (!line) {
    throw new Error("Unable to determine the Rust host target triple");
  }
  return line.slice("host:".length).trim();
}
