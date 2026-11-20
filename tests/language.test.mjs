import test from "node:test";
import assert from "node:assert/strict";
import { isLanguagePreference, resolveLanguage } from "../src/domain/language.ts";
import { isNewsRegionPreference, defaultNewsRegion } from "../src/domain/newsRegion.ts";
import { settingsRepository } from "../src/platform/tauri/settings.ts";

test("settings adapter rejects short intervals before invoking native code", async () => {
  await assert.rejects(settingsRepository.save({autoStart:false,startSchedulerOnLaunch:false,intervalMs:60000}), /Minimum: 5 minutes/);
});

test("country choices are explicit and defaults depend only on Windows language", () => {
  assert.equal(defaultNewsRegion("pl"), "PL");
  assert.equal(defaultNewsRegion("en"), "US");
  for (const country of ["PL","US","GB","RO","DE","JP","BR"]) assert.ok(isNewsRegionPreference(country));
  for (const value of [null,undefined,"auto","XX","",1]) assert.equal(isNewsRegionPreference(value),false);
});

test("automatic follows Windows language; explicit preference takes precedence", () => {
  for (const system of ["pl", "en"]) {
    assert.equal(resolveLanguage("auto", system), system);
    assert.equal(resolveLanguage("pl", system), "pl");
    assert.equal(resolveLanguage("en", system), "en");
  }
  for (const value of ["auto", "pl", "en"]) assert.equal(isLanguagePreference(value), true);
  for (const value of [undefined, null, "de", "", 1]) assert.equal(isLanguagePreference(value), false);
});
