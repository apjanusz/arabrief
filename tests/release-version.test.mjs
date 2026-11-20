import test from "node:test";
import assert from "node:assert/strict";
import { checkVersions, readVersionFiles } from "../scripts/check-release-version.mjs";

test("repository versions match each other and their release tag", () => {
  const files = readVersionFiles();
  assert.equal(checkVersions(files, `v${files.package.version}`), files.package.version);
});

test("a mismatched version in any manifest or lockfile prevents release", () => {
  const mutations = [
    files => { files.package.version = "99.0.0"; },
    files => { files.tauri.version = "99.0.0"; },
    files => { files.cargo = files.cargo.replace(/version = "[^"]+"/, 'version = "99.0.0"'); },
    files => { files.npmLock.version = "99.0.0"; },
    files => { files.npmLock.packages[""].version = "99.0.0"; },
    files => { files.cargoLock = files.cargoLock.replace(/(name = "ara-brief"\s+version = ")[^"]+/, "$199.0.0"); },
  ];
  for (const mutate of mutations) {
    const files = readVersionFiles(); mutate(files);
    assert.throws(() => checkVersions(files));
  }
});

test("invalid tags and tags for another version cannot publish", () => {
  for (const tag of ["", "0.1.0", "v99.0.0", "v0.1.0-beta.1", "v01.1.0", "v0.1.0/extra", "v0.1.0\n", "v0.1.0; echo unexpected"]) {
    assert.throws(() => checkVersions(readVersionFiles(), tag));
  }
});
