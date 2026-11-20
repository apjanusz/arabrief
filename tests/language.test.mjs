import test from "node:test";
import assert from "node:assert/strict";
import { isLanguagePreference, resolveLanguage } from "../src/domain/language.ts";

test("automatic follows Windows language; explicit preference takes precedence", () => {
  for (const system of ["pl", "en"]) {
    assert.equal(resolveLanguage("auto", system), system);
    assert.equal(resolveLanguage("pl", system), "pl");
    assert.equal(resolveLanguage("en", system), "en");
  }
  for (const value of ["auto", "pl", "en"]) assert.equal(isLanguagePreference(value), true);
  for (const value of [undefined, null, "de", "", 1]) assert.equal(isLanguagePreference(value), false);
});
