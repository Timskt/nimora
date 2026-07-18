import { chmod, cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { promisify } from "node:util";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = join(root, "apps/desktop/src-tauri/binaries");
const target = process.env.TAURI_ENV_TARGET_TRIPLE ?? (await rustHostTriple());
const sidecars = [
  { packageName: "nimora-user-code-worker", binaryName: "nimora-user-code-worker" },
  { packageName: "nimora-skill-worker", binaryName: "nimora-skill-worker" },
  { packageName: "nimora-model-importer", binaryName: "nimora-model-importer-worker" },
  {
    packageName: "nimora-agent-provider-worker",
    binaryName: "nimora-agent-provider-worker",
    providerWorker: true,
  },
];

await mkdir(outputDirectory, { recursive: true });
for (const sidecar of sidecars) {
  await prepareSidecar(sidecar);
}

async function prepareSidecar({ packageName, binaryName, providerWorker }) {
  await execFileAsync("cargo", ["build", "-p", packageName, "--release"], { cwd: root });
  const executableSuffix = process.platform === "win32" ? ".exe" : "";
  const source = join(root, "target/release", `${binaryName}${executableSuffix}`);
  const destination = join(outputDirectory, `${binaryName}-${target}${executableSuffix}`);
  await rm(destination, { force: true });
  await cp(source, destination);
  if (process.platform !== "win32") await chmod(destination, 0o755);
  if (providerWorker) await writeProviderManifest(destination);
  console.log(`Prepared ${binaryName} for ${target}`);
}

async function writeProviderManifest(executablePath) {
  const executable = basename(executablePath);
  const bytes = await readFile(executablePath);
  const manifest = {
    spec: "nimora.provider-worker-manifest/1",
    workerProtocolVersion: 1,
    capabilities: ["provider:ollama-loopback/1", "provider:openai-compatible/1"],
    executable,
    executableBytes: bytes.byteLength,
    executableSha256: createHash("sha256").update(bytes).digest("hex"),
  };
  const manifestBytes = Buffer.from(`${JSON.stringify(manifest)}\n`);
  const manifestPath = join(outputDirectory, "agent-provider-worker.json");
  await writeFile(manifestPath, manifestBytes);
  await writeFile(
    `${manifestPath}.sha256`,
    `${createHash("sha256").update(manifestBytes).digest("hex")}  agent-provider-worker.json\n`,
  );
}

async function rustHostTriple() {
  const { stdout } = await execFileAsync("rustc", ["-vV"], { cwd: root });
  const line = stdout.split("\n").find((entry) => entry.startsWith("host:"));
  if (!line) throw new Error("Unable to determine the Rust host target triple");
  return line.slice("host:".length).trim();
}
