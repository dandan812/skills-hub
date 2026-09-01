#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";

const root = process.cwd();
const args = process.argv.slice(2);
const requireUpdater = args.includes("--require-updater");
const tagIndex = args.indexOf("--tag");
const releaseTag = tagIndex === -1 ? null : args[tagIndex + 1];
const expectedRepoIndex = args.indexOf("--expected-repo");
const expectedRepository = expectedRepoIndex === -1 ? null : args[expectedRepoIndex + 1];

if (tagIndex !== -1 && !releaseTag) {
  throw new Error("--tag requires a value");
}
if (expectedRepoIndex !== -1 && (!expectedRepository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(expectedRepository))) {
  throw new Error("--expected-repo requires a GitHub owner/repository pair");
}

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), "utf8");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function checkVersionSync() {
  const result = spawnSync(process.execPath, ["scripts/version.mjs", "check"], {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || result.stdout.trim() || "Version check failed");
  }
  return JSON.parse(read("package.json")).version;
}

function validateUpdater(updater) {
  assert(updater && typeof updater === "object", "tauri.conf.json is missing plugins.updater");

  if (!updater.active) {
    assert(
      Array.isArray(updater.endpoints) && updater.endpoints.length === 0,
      "Disabled updater must not retain release endpoints",
    );
    assert(!updater.pubkey, "Disabled updater must not retain a signing public key");
    if (requireUpdater) {
      throw new Error(
        "The self-owned updater is disabled. Run `npm run updater:configure -- --repo <owner/repo> --pubkey-file <key.pub>` first.",
      );
    }
    return "disabled";
  }

  assert(updater.dialog === false, "Custom updater must not use the official updater dialog");
  assert(typeof updater.pubkey === "string" && updater.pubkey.length > 32, "Updater public key is missing");
  assert(Array.isArray(updater.endpoints) && updater.endpoints.length === 1, "Updater must have exactly one endpoint");

  const endpoint = new URL(updater.endpoints[0]);
  assert(endpoint.protocol === "https:", "Updater endpoint must use HTTPS");
  assert(endpoint.hostname === "github.com", "Updater endpoint must be hosted on your GitHub Release source");
  assert(
    /^\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\/releases\/latest\/download\/updater\.json$/.test(endpoint.pathname),
    "Updater endpoint must target GitHub Releases latest/download/updater.json",
  );
  return endpoint.toString();
}

function main() {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  assert(config.identifier === "com.dandan812.skillshubcustom", "Custom app identifier was changed");
  assert(config.productName === "Skills Hub Custom", "Custom product name was changed");
  assert(config.bundle?.createUpdaterArtifacts === Boolean(config.plugins?.updater?.active), "Updater artifact setting must match updater activation");

  for (const relativePath of [
    "src-tauri/src/runtime_evidence/mod.rs",
    "src/features/runtime-evidence/RuntimeEvidencePage.tsx",
    "CUSTOMIZATION.md",
  ]) {
    assert(fs.existsSync(path.join(root, relativePath)), `Required custom-owned path is missing: ${relativePath}`);
  }

  const customization = read("CUSTOMIZATION.md");
  assert(customization.includes("custom/main"), "CUSTOMIZATION.md no longer documents custom/main");
  assert(customization.includes("runtime_evidence"), "CUSTOMIZATION.md no longer documents the runtime extension boundary");

  const version = checkVersionSync();
  assert(/^\d+\.\d+\.\d+-custom\.\d+$/.test(version), "Custom releases must use <upstream>-custom.<revision> versions");

  if (releaseTag) {
    assert(releaseTag === `v${version}`, `Release tag ${releaseTag} does not match application version ${version}`);
  }

  const updaterState = validateUpdater(config.plugins?.updater);
  if (expectedRepository) {
    const expectedEndpoint = `https://github.com/${expectedRepository}/releases/latest/download/updater.json`;
    assert(updaterState === expectedEndpoint, `Updater endpoint must match the release repository: ${expectedEndpoint}`);
  }
  console.log(`Custom boundary check passed (version ${version}; updater ${updaterState}).`);
}

try {
  main();
} catch (error) {
  console.error(`Custom boundary check failed: ${error.message}`);
  process.exit(1);
}
