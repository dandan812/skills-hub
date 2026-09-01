#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const args = process.argv.slice(2);
const values = new Map();
let dryRun = false;

for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (argument === "--dry-run") {
    dryRun = true;
    continue;
  }
  if (argument === "--repo" || argument === "--pubkey-file") {
    const value = args[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`${argument} requires a value`);
    }
    values.set(argument, value);
    index += 1;
    continue;
  }
  if (argument === "--help" || argument === "-h") {
    console.log("Usage: node scripts/configure-updater.mjs --repo <owner/repo> --pubkey-file <path-to-public-key> [--dry-run]");
    process.exit(0);
  }
  throw new Error(`Unknown argument: ${argument}`);
}

const repository = values.get("--repo");
const publicKeyFile = values.get("--pubkey-file");
if (!repository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
  throw new Error("--repo must be a GitHub owner/repository pair");
}
if (!publicKeyFile) {
  throw new Error("--pubkey-file is required; never put a private signing key in the repository");
}

const resolvedKeyPath = path.resolve(root, publicKeyFile);
if (!fs.existsSync(resolvedKeyPath)) {
  throw new Error(`Public key file does not exist: ${resolvedKeyPath}`);
}

const keyInput = fs.readFileSync(resolvedKeyPath, "utf8").trim();
if (/private key/i.test(keyInput)) {
  throw new Error("The supplied file appears to be a private key. Supply the generated .pub file instead.");
}

let publicKey;
if (keyInput.startsWith("untrusted comment:")) {
  publicKey = Buffer.from(`${keyInput}\n`, "utf8").toString("base64");
} else {
  const decoded = Buffer.from(keyInput, "base64").toString("utf8").trim();
  if (!decoded.startsWith("untrusted comment:")) {
    throw new Error("Public key must be a Tauri signer .pub file or its base64 configuration value");
  }
  publicKey = keyInput.replace(/\s+/g, "");
}

const configPath = path.join(root, "src-tauri", "tauri.conf.json");
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
config.bundle = { ...config.bundle, createUpdaterArtifacts: true };
config.plugins = {
  ...config.plugins,
  updater: {
    active: true,
    dialog: false,
    endpoints: [`https://github.com/${repository}/releases/latest/download/updater.json`],
    pubkey: publicKey,
  },
};

if (dryRun) {
  console.log(`Would enable signed updates from https://github.com/${repository}/releases/latest/download/updater.json`);
  process.exit(0);
}

fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
console.log(`Configured signed app updates from your GitHub Releases: ${repository}`);
console.log("Next: commit this public configuration, then set TAURI_SIGNING_PRIVATE_KEY (and, if used, TAURI_SIGNING_PRIVATE_KEY_PASSWORD) as GitHub Actions secrets.");
