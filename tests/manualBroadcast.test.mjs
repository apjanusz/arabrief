import test from "node:test";
import assert from "node:assert/strict";
import { ManualBroadcastRunner } from "../src/modules/scheduler/ManualBroadcastRunner.ts";

const settings = { topics: ["Nauka"], preferredAudioSource: { id: "music-session", name: "Test session" }, openAiVoice: "alloy" };
const script = { id: "broadcast-1", sources: [{ id: "1" }, { id: "2" }] };
const audio = { resourceId: "generated.wav" };
const ended = { phase: "ended" };
function fixture() {
  const events = [];
  const ports = {
    news: { fetch: async () => { events.push("fetch"); return { items: [{ id: "1" }, { id: "2" }] }; }, markUsed: async () => { events.push("used"); } },
    content: { generate: async () => { events.push("generate"); return script; } },
    speech: { synthesize: async () => { events.push("synthesize"); return audio; }, cancel: async () => { events.push("cancel-speech"); return true; } },
    player: { play: async request => { events.push(["play", request]); return ended; }, stop: async () => { events.push("stop"); } },
    onStage: stage => events.push(`stage:${stage}`), onNews: () => {},
    onReady: () => events.push("ready"), onPlayed: () => events.push("played"), makeJobId: () => "test-job",
  };
  return { ports, events, runner: new ManualBroadcastRunner(ports) };
}
const tick = () => new Promise(resolve => setImmediate(resolve));

test("manual pipeline prepares before native playback and marks usage after real end", async () => {
  const { runner, ports, events } = fixture();
  let finish;
  ports.player.play = async request => { events.push(["play", request]); return new Promise(resolve => { finish = resolve; }); };
  const work = runner.run(settings, false);
  await tick();
  assert.ok(events.includes("ready")); assert.ok(!events.includes("used")); assert.ok(!events.includes("played"));
  const request = events.find(item => Array.isArray(item))[1];
  assert.equal(request.musicSessionId, "music-session"); assert.equal(request.resourceId, "generated.wav");
  finish(ended); await work;
  assert.ok(events.indexOf("played") < events.indexOf("used")); assert.equal(events.at(-1), "stage:idle");
});

test("prepare-only never changes playback or usage history", async () => {
  const { runner, events } = fixture(); await runner.run(settings, true);
  assert.ok(events.includes("ready")); assert.ok(!events.some(Array.isArray)); assert.ok(!events.includes("used"));
});

test("stop during non-cancellable content waits then suppresses speech and playback", async () => {
  const { runner, ports, events } = fixture(); let finish;
  ports.content.generate = () => new Promise(resolve => { finish = resolve; });
  const work = runner.run(settings, false); const rejected = assert.rejects(work, /Anulowano/);
  await tick(); let stopped = false; const stop = runner.stop().then(() => { stopped = true; });
  await tick(); assert.equal(stopped, false); finish(script); await stop; await rejected;
  assert.ok(!events.includes("synthesize")); assert.ok(!events.some(Array.isArray)); assert.ok(!events.includes("used"));
});

test("stop waits for native playback cleanup and does not mark interrupted audio used", async () => {
  const { runner, ports, events } = fixture(); let finish;
  ports.player.play = () => new Promise(resolve => { finish = resolve; });
  ports.player.stop = async () => { events.push("stop"); finish({ phase: "stopped" }); };
  const work = runner.run(settings, false); const rejected = assert.rejects(work, /Anulowano/);
  await tick(); await runner.stop(); await rejected;
  assert.ok(events.includes("stop")); assert.ok(!events.includes("used")); assert.ok(!events.includes("played"));
});

test("speech errors never start ducking/playback; concurrent runs are rejected", async () => {
  const { runner, ports, events } = fixture();
  ports.speech.synthesize = async () => { throw new Error("speech failure"); };
  const work = runner.run(settings, false);
  await assert.rejects(runner.run(settings, false), /już/); await assert.rejects(work, /speech failure/);
  assert.ok(!events.some(Array.isArray)); assert.equal(events.at(-1), "stage:idle");
});

test("ready audio is reused without new paid generation; completed playback remains known on history error", async () => {
  const { runner, ports, events } = fixture();
  ports.news.markUsed = async () => { throw new Error("history failure"); };
  await assert.rejects(runner.run(settings, false, { script, audio }), /history failure/);
  assert.ok(!events.includes("generate")); assert.ok(events.includes("played"));
});

test("missing music source is rejected before paid preparation", async () => {
  const { runner, events } = fixture();
  await assert.rejects(runner.run({ ...settings, preferredAudioSource: null }, false), /źródło/);
  assert.ok(!events.includes("fetch"));
});
