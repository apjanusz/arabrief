import test from "node:test";
import assert from "node:assert/strict";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createServer } from "vite";

test("studio, interests and settings render in each selected language", async () => {
  const server = await createServer({ server: { middlewareMode: true }, appType: "custom" });
  try {
    const { LanguageProvider } = await server.ssrLoadModule("/src/app/i18n.tsx");
    const { Dashboard } = await server.ssrLoadModule("/src/app/views/Dashboard.tsx");
    const { Interests } = await server.ssrLoadModule("/src/app/views/Interests.tsx");
    const { Settings } = await server.ssrLoadModule("/src/app/views/Settings.tsx");
    const app = {
      settings: { newsRegion: "US", version: 1, uiLanguage: "en", preferredAudioSource: null, intervalMs: 1800000, duckVolume: .15, fadeOutMs: 3000, fadeInMs: 3000, narratorVolume: .7, broadcastMinDurationMs: 120000, broadcastMaxDurationMs: 180000, topics: [], openAiVoice: "marin", textModel: "gpt-5-mini", autoStart: false, startSchedulerOnLaunch: false, language: "pl", newsSourceUrls: [] },
      sessions: [], stage: "idle", loading: false, busy: false, saving: false,
      schedule: {phase:"idle", remainingMs:null},
    };
    for (const language of ["pl", "en"]) {
      const render = Component => renderToStaticMarkup(React.createElement(LanguageProvider, {preference:language}, React.createElement(Component, {app, navigate:()=>{}, credentials:{}})));
      const studio = render(Dashboard);
      const settings = render(Settings);
      const interests = render(Interests);
      for (const html of [studio, settings, interests]) {
        assert.doesNotMatch(html, /ui_[a-f0-9]{10}/);
        if (language === "en") assert.doesNotMatch(html, /[ąćęłńóśźżĄĆĘŁŃÓŚŹŻ]/);
      }
      assert.ok(studio.includes(language === "en" ? "Between tracks." : "Między utworami."));
      assert.ok(settings.includes(language === "en" ? "Save settings" : "Zapisz ustawienia"));
      assert.ok(settings.includes(language === "en" ? "Google News region" : "Region wiadomości Google News"));
      assert.ok(settings.includes(language === "en" ? "United Kingdom" : "Wielka Brytania"));
      assert.ok(settings.includes(language === "en" ? "Romania" : "Rumunia"));
      assert.ok(settings.includes("<datalist"));
      assert.doesNotMatch(settings.match(/<datalist[\s\S]*?<\/datalist>/)?.[0] ?? "", /value="auto"/);
      assert.ok(interests.includes(language === "en" ? "Add topic" : "Dodaj temat"));
    }
  } finally { await server.close(); }
});
