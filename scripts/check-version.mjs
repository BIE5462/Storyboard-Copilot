import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function extractCargoPackageVersion(relativePath) {
  const content = readText(relativePath);
  const match = content.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(`Cannot locate [package].version in ${relativePath}`);
  }
  return match[1];
}

function extractCargoLockPackageVersion(relativePath, packageName) {
  const content = readText(relativePath);
  const blockPattern = new RegExp(
    `\\[\\[package\\]\\]\\s+name = "${packageName}"\\s+version = "([^"]+)"`,
    "m"
  );
  const match = content.match(blockPattern);
  if (!match) {
    throw new Error(`Cannot locate ${packageName} package entry in ${relativePath}`);
  }
  return match[1];
}

const packageJson = readJson("package.json");
const packageLock = readJson("package-lock.json");
const tauriConfig = readJson("src-tauri/tauri.conf.json");
const expectedVersion = packageJson.version;

const versions = [
  ["package.json", packageJson.version],
  ["package-lock.json", packageLock.version],
  ["package-lock.json packages[\"\"]", packageLock.packages?.[""]?.version],
  ["src-tauri/tauri.conf.json", tauriConfig.version],
  ["src-tauri/Cargo.toml", extractCargoPackageVersion("src-tauri/Cargo.toml")],
  [
    "src-tauri/Cargo.lock",
    extractCargoLockPackageVersion("src-tauri/Cargo.lock", packageJson.name),
  ],
];

const mismatches = versions.filter(([, version]) => version !== expectedVersion);
if (mismatches.length > 0) {
  console.error(`Version mismatch. Expected ${expectedVersion} everywhere:`);
  for (const [source, version] of mismatches) {
    console.error(`- ${source}: ${version ?? "(missing)"}`);
  }
  process.exit(1);
}

console.log(`Version check passed: ${expectedVersion}`);
