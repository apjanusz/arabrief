import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const versionPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

function tomlVersion(text, section, name) {
  const block = text.split(section).slice(1).find(part => !name || new RegExp(`^\\s*name\\s*=\\s*"${name}"\\s*$`, "m").test(part.split(/\n\s*\[/)[0]));
  const match = block?.split(/\n\s*\[/)[0].match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);
  if (!match) throw new Error(`Cannot read version from ${section}${name ? ` ${name}` : ""}.`);
  return match[1];
}

export function checkVersions(files, tag) {
  const versions = {
    "package.json": files.package.version,
    "src-tauri/tauri.conf.json": files.tauri.version,
    "src-tauri/Cargo.toml": tomlVersion(files.cargo, "[package]"),
    "package-lock.json": files.npmLock.version,
    "package-lock.json packages[root]": files.npmLock.packages?.[""]?.version,
    "src-tauri/Cargo.lock": tomlVersion(files.cargoLock, "[[package]]", "ara-brief"),
  };
  const version = versions["package.json"];
  if (typeof version !== "string" || !versionPattern.test(version)) throw new Error("Release version must use X.Y.Z, without a prefix, leading zeros or prerelease suffix.");
  for (const [file, actual] of Object.entries(versions)) {
    if (actual !== version) throw new Error(`${file}: expected ${version}, found ${String(actual)}.`);
  }
  if (tag !== undefined && (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag) || tag !== `v${version}`)) {
    throw new Error(`Tag must be v${version}; received ${String(tag)}.`);
  }
  return version;
}

export function readVersionFiles(root = process.cwd()) {
  const read = path => readFileSync(resolve(root, path), "utf8").replace(/^\uFEFF/, "");
  return {
    package: JSON.parse(read("package.json")),
    tauri: JSON.parse(read("src-tauri/tauri.conf.json")),
    cargo: read("src-tauri/Cargo.toml"),
    npmLock: JSON.parse(read("package-lock.json")),
    cargoLock: read("src-tauri/Cargo.lock"),
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const args = process.argv.slice(2);
    if (args.length && (args.length !== 2 || args[0] !== "--tag")) throw new Error("Usage: node scripts/check-release-version.mjs [--tag vX.Y.Z]");
    console.log(`Release versions OK: ${checkVersions(readVersionFiles(), args[1])}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
