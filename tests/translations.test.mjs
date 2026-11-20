import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import ts from "typescript";
import catalog from "../src/app/catalog.json" with { type: "json" };
import { translate, localizeMessage } from "../src/app/localization.ts";

test("every catalog entry is available in both languages", () => {
  for (const [key, entry] of Object.entries(catalog)) {
    assert.ok(entry.pl.trim(), key);
    assert.ok(entry.en.trim(), key);
    assert.equal(translate(key, "pl"), entry.pl);
    assert.equal(translate(key, "en"), entry.en);
    assert.deepEqual([...entry.pl.matchAll(/\{[^}]*\}/g)].map(m=>m[0]).sort(), [...entry.en.matchAll(/\{[^}]*\}/g)].map(m=>m[0]).sort(), key);
  }
});

test("stored messages, nested native errors and technical values switch language", () => {
  assert.equal(localizeMessage("Zapisano zmiany.", "en"), "Changes saved.");
  assert.equal(localizeMessage("Changes saved.", "pl"), "Zapisano zmiany.");
  assert.equal(localizeMessage("Przywracanie muzyki: Nieprawidłowy identyfikator odtwarzania", "en"), "Restoring music: Invalid playback identifier");
  assert.equal(localizeMessage("Session expired", "pl"), "Sesja wygasła");
  assert.equal(localizeMessage("Process 123 (name unavailable)", "pl"), "Proces 123 (nazwa niedostępna)");
  assert.equal(localizeMessage("HTTP 503, request-id=abc123", "en"), "HTTP 503, request-id=abc123");
  assert.equal(translate("locale", "en"), "en-US");
});

test("components use existing catalog keys and contain no inline Polish UI copy", () => {
  function walk(dir) {
    for (const entry of readdirSync(dir, {withFileTypes:true})) {
      const file = `${dir}/${entry.name}`;
      if (entry.isDirectory()) walk(file);
      else if (file.endsWith(".tsx") && !file.endsWith("i18n.tsx")) {
        const source = ts.createSourceFile(file, readFileSync(file,"utf8"), ts.ScriptTarget.Latest, true);
        function visit(node) {
          if (ts.isStringLiteral(node) || ts.isJsxText(node)) {
            assert.doesNotMatch(node.text, /[ąćęłńóśźżĄĆĘŁŃÓŚŹŻ]/, `${file}: ${node.text}`);
            if (/^ui_[a-f0-9]+$/.test(node.text)) assert.ok(catalog[node.text], `${file}: ${node.text}`);
          }
          ts.forEachChild(node, visit);
        }
        visit(source);
      }
    }
  }
  walk("src");
});
