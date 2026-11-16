import test from "node:test";
import assert from "node:assert/strict";
import { ManualBroadcastRunner } from "../src/modules/scheduler/ManualBroadcastRunner.ts";
import { NoUsableMaterial } from "../src/modules/scheduler/PreparationRecovery.ts";

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

test("partial chapter audio consumes only articles actually included in playback",async()=>{
  const f=fixture();let prepared,used;
  const texts=["Pierwszy rozdział.","Drugi rozdział.","Trzeci rozdział."];
  f.ports.content.generate=async()=>({id:"partial",text:texts.join("\n\n"),sources:[{id:"1"},{id:"2"},{id:"3"}],draft:{segments:texts.map((text,index)=>({text,evidence:[{source_id:String(index+1)}]})),limitations:[],insufficient_material:false}});
  f.ports.speech.synthesize=async()=>({resourceId:"partial.wav",durationMs:2000,chapters:[{articleId:"1",position:0,startMs:0,endMs:1000},{articleId:"3",position:2,startMs:1000,endMs:2000}]});
  f.ports.onReady=value=>{prepared=value;};f.ports.news.markUsed=async ids=>{used=ids;};
  await f.runner.run(settings,false);
  assert.deepEqual(used,["1","3"]);assert.equal(prepared.script.text,[texts[0],texts[2]].join("\n\n"));assert.equal(prepared.script.draft.insufficient_material,true);
});

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
test("timings identify stages and ready audio bypasses all preparation services", async () => {
  const { runner, ports, events }=fixture();const timings=[];
  ports.onTiming=entry=>timings.push(entry);
  await runner.run(settings,true);
  assert.deepEqual(timings.map(entry=>entry.operation),["prepare.news","prepare.content","prepare.speech","prepare.total"]);
  assert.ok(timings.every(entry=>entry.status==="ok"&&entry.elapsedMs>=0));
  timings.length=0;events.length=0;
  await runner.run(settings,false,{script,audio});
  assert.deepEqual(timings.map(entry=>entry.operation),["broadcast.total"]);
  assert.ok(!events.includes("fetch")&&!events.includes("generate")&&!events.includes("synthesize"));
});
test("a broken timing observer cannot fail a prepared broadcast", async () => {
  const {runner,ports,events}=fixture();ports.onTiming=()=>{throw Error("observer");};
  await runner.run(settings,true);assert.ok(events.includes("ready"));
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
  await runner.run(settings, false, { script, audio });
  assert.ok(!events.includes("generate")); assert.ok(events.includes("played"));
  await assert.rejects(runner.run(settings,false,{script,audio}),/już rozpoczęta/);
});

test("missing music source is rejected before paid preparation", async () => {
  const { runner, events } = fixture();
  await assert.rejects(runner.run({ ...settings, preferredAudioSource: null }, false), /źródło/);
  assert.ok(!events.includes("fetch"));
});

test("one healthy article survives another topic failing; no news avoids paid calls",async()=>{
  const f=fixture();
  f.ports.news.fetch=async()=>({items:[{id:"1"}],topics:[{topic:"Sport",error:"timeout"}]});
  let ids;f.ports.content.generate=async input=>{ids=input;return script;};
  await f.runner.run(settings,true);assert.deepEqual(ids,["1"]);assert.ok(f.events.includes("ready"));
  const empty=fixture();empty.ports.news.fetch=async()=>({items:[],topics:[]});
  await assert.rejects(empty.runner.run(settings,true),NoUsableMaterial);
  assert.ok(!empty.events.includes("generate")&&!empty.events.includes("synthesize"));
});

test("a failed TTS attempt resumes the saved script instead of spending on text again",async()=>{
  const f=fixture();let calls=0;
  f.ports.speech.synthesize=async()=>{if(++calls===1)throw Error("failed part");return audio;};
  await assert.rejects(f.runner.run(settings,true),/failed part/);
  await f.runner.run(settings,true);
  assert.equal(f.events.filter(e=>e==="generate").length,1);
  assert.equal(f.events.filter(e=>e==="fetch").length,1);
  assert.equal(calls,2);assert.ok(f.events.includes("ready"));
});

test("fresh preparation failure preserves unused fallback and never replays it",async()=>{
  const f=fixture();const messages=[];f.ports.onRecovered=message=>messages.push(message);
  await f.runner.run(settings,true);
  f.ports.news.fetch=async()=>{throw Error("RSS unavailable");};
  await f.runner.run(settings,true,undefined,true);
  assert.ok(messages.some(message=>message.includes("poprzednią gotową")));
  await f.runner.run(settings,false);
  assert.equal(f.events.filter(e=>Array.isArray(e)).length,1);
  await assert.rejects(f.runner.run(settings,false),/RSS unavailable/);
  assert.equal(f.events.filter(e=>Array.isArray(e)).length,1);
});

test("native recovery restores pending script or complete audio after UI restart",async()=>{
  for(const asset of [null,audio]){
    const f=fixture();const events=[];
    f.ports.recovery={restore:async()=>({script,audio:asset}),claim:async()=>events.push("claim"),complete:async()=>events.push("complete")};
    await f.runner.run(settings,false);
    assert.ok(!f.events.includes("fetch")&&!f.events.includes("generate"));
    assert.equal(f.events.includes("synthesize"),asset===null);
    assert.deepEqual(events,["claim","complete"]);
  }
});

test("claim failure prevents playback and cancellation never starts fallback",async()=>{
  const f=fixture();f.ports.recovery={restore:async()=>({script,audio}),claim:async()=>{throw Error("already played");},complete:async()=>{}};
  await assert.rejects(f.runner.run(settings,false),/already played/);assert.ok(!f.events.some(Array.isArray));
  const cancelled=fixture();await cancelled.runner.run(settings,true);
  let finish;cancelled.ports.news.fetch=()=>new Promise(resolve=>{finish=resolve;});
  const work=cancelled.runner.run(settings,false,undefined,true);const rejected=assert.rejects(work,/Anulowano/);
  await tick();const stopping=cancelled.runner.stop();finish({items:[]});await stopping;await rejected;
  assert.ok(!cancelled.events.some(Array.isArray));
});

test("authoritative native recovery prevents another runner's consumed fallback",async()=>{
  const f=fixture();let saved={script,audio};
  f.ports.recovery={restore:async()=>saved,claim:async()=>{},complete:async()=>{}};
  await f.runner.run(settings,true);saved=null;
  f.ports.news.fetch=async()=>({items:[]});
  await assert.rejects(f.runner.run(settings,false),NoUsableMaterial);
  assert.ok(!f.events.some(Array.isArray));
});
