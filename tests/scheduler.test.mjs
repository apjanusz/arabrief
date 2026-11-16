import test from "node:test";
import assert from "node:assert/strict";
import { AutomaticBroadcastScheduler } from "../src/modules/scheduler/AutomaticBroadcastScheduler.ts";
import { NoUsableMaterial } from "../src/modules/scheduler/PreparationRecovery.ts";

const settings = { intervalMs: 1000, topics: ["Nauka"], preferredAudioSource: { id: "music", name: "Music" }, openAiVoice: "alloy" };
const script = { id: "script", sources: [{ id: "1" }, { id: "2" }] };
const audio = { resourceId: "audio.wav" };
const flush = async () => { for (let i = 0; i < 8; i++) await new Promise(resolve => setImmediate(resolve)); };
function deferred() { let resolve, reject; const promise = new Promise((yes,no)=>{resolve=yes;reject=no;}); return { promise, resolve, reject }; }
function fixture(overrides = {}) {
  let mono = 0, wall = 1_700_000_000_000, serial = 0;
  const timers = new Map(); const events = []; const states = [];
  const clock = { mono: () => mono, wall: () => wall, set: (callback, delay) => { const id=++serial;timers.set(id,{callback,at:mono+delay});return id; },clear: id=>timers.delete(id) };
  async function advance(ms) {
    const target=mono+ms;
    while(true){const due=[...timers].filter(([,v])=>v.at<=target).sort((a,b)=>a[1].at-b[1].at)[0];if(!due)break;wall+=due[1].at-mono;mono=due[1].at;timers.delete(due[0]);due[1].callback();await flush();}
    wall+=target-mono;mono=target;await flush();
  }
  const ports = {
    clock, makeJobId:()=>"job", onStage:()=>{}, onNews:()=>{},onReady:()=>{},onPlayed:()=>events.push("played"),
    onSkipped: reason=>events.push(`skipped:${reason}`),onError: message=>events.push(`error:${message}`),log: event=>events.push(`log:${event}`),
    sourceAvailable:async()=>true,
    news:{fetch:async()=>({items:[{id:"1"},{id:"2"}]}),markUsed:async()=>{events.push("used");}},
    content:{generate:async()=>script},speech:{synthesize:async()=>audio,cancel:async()=>true},
    player:{play:async()=>({phase:"ended"}),stop:async()=>{events.push("stop-player");},getStatus:async()=>({phase:"playing"})},
    ...overrides,
  };
  const scheduler = new AutomaticBroadcastScheduler(ports);
  scheduler.subscribe(state=>states.push(state));
  return { scheduler,events,states,ports,advance,elapseWithoutTimers:ms=>{wall+=ms;mono+=ms;},sleep: ms=>{wall+=ms;scheduler.wake();},resumeElapsed: ms=>{wall+=ms;mono+=ms;scheduler.wake();},timerCount:()=>timers.size };
}
test("native wake runs a hidden schedule when browser timers do not fire, and never restarts after stop",async()=>{
  const f=fixture(); await f.scheduler.start(settings,{script,audio});
  f.elapseWithoutTimers(1000); assert.equal(f.events.includes("played"),false);
  f.scheduler.wake();f.scheduler.wake();await flush();
  assert.equal(f.events.filter(e=>e==="played").length,1);
  await f.scheduler.stop(); f.elapseWithoutTimers(1000);f.scheduler.wake();await flush();
  assert.equal(f.events.filter(e=>e==="played").length,1);assert.equal(f.timerCount(),0);
});
test("prepares immediately, waits full interval, plays once, then starts next full interval",async()=>{
  const f=fixture();await f.scheduler.start(settings);assert.equal(f.scheduler.getState().phase,"preparing");await flush();
  assert.equal(f.scheduler.getState().phase,"waiting");assert.equal(f.scheduler.getState().ready,true);
  await f.advance(999);assert.equal(f.events.includes("played"),false);
  await f.advance(1);assert.equal(f.events.filter(e=>e==="played").length,1);
  assert.ok(f.events.indexOf("played")<f.events.indexOf("used"));
  assert.equal(f.scheduler.getState().cycle,2);assert.equal(f.scheduler.getState().remainingMs,1000);
  assert.ok(f.scheduler.getDiagnostics().some(entry=>entry.event==="play_completed"&&entry.phase==="ducking"));
  await f.scheduler.stop();assert.equal(f.scheduler.getState().phase,"idle");assert.equal(f.timerCount(),0);
});
test("missing recording skips deadline and cancels slow preparation without ducking",async()=>{
  const slow=deferred();let plays=0;const f=fixture({content:{generate:()=>slow.promise},player:{play:async()=>{plays++;return {phase:"ended"};},stop:async()=>{},getStatus:async()=>({phase:"idle"})}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  assert.equal(f.events.includes("skipped:not_ready"),true);assert.equal(plays,0);
  slow.resolve(script);await flush();assert.equal(f.scheduler.getState().cycle,2);
  assert.equal(plays,0);await f.scheduler.stop();
});
test("wake after missed deadline skips stale audio and starts a new interval",async()=>{
  const f=fixture();await f.scheduler.start(settings);await flush();f.resumeElapsed(60_000);await flush();
  assert.equal(f.events.includes("skipped:sleep"),true);assert.equal(f.events.includes("played"),false);
  assert.equal(f.scheduler.getState().cycle,2);await f.scheduler.stop();
});
test("stop during playback waits for native restoration and avoids marking interrupted news",async()=>{
  const playback=deferred();let stopped=0;
  const f=fixture({player:{play:()=>playback.promise,stop:async()=>{stopped++;playback.resolve({phase:"stopped"});},getStatus:async()=>({phase:"restoring"})}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  const end=f.scheduler.stop();await end;
  assert.equal(stopped,1);assert.equal(f.scheduler.getState().phase,"idle");
  assert.equal(f.events.includes("used"),false);assert.equal(f.timerCount(),0);
});
test("native playback error attempts restoration and schedules next cycle",async()=>{
  let restores=0;const f=fixture({player:{play:async()=>{throw Error("native failed");},stop:async()=>{restores++;},getStatus:async()=>({phase:"error"})}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  assert.equal(restores,1);assert.equal(f.events.includes("error:native failed"),true);
  assert.equal(f.scheduler.getState().cycle,2);await f.scheduler.stop();
});
test("session enumeration failure before ducking cannot wedge the scheduler",async()=>{
  const f=fixture({sourceAvailable:async()=>{throw Error("session enumeration failed");}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  assert.ok(f.events.includes("error:session enumeration failed"));
  assert.equal(f.scheduler.getState().cycle,2);
  assert.equal(f.events.includes("log:invalid_state_transition"),false);
  await f.scheduler.stop();assert.equal(f.timerCount(),0);
});
test("source disappearing prevents playback and stops automatic paid preparation",async()=>{
  const f=fixture({sourceAvailable:async()=>false});await f.scheduler.start(settings);await flush();await f.advance(1000);
  assert.equal(f.events.includes("skipped:source_unavailable"),true);
  assert.equal(f.scheduler.getState().phase,"error");assert.equal(f.timerCount(),0);
});
test("prepared but not yet played audio is reused at the first deadline",async()=>{
  let fetches=0;const f=fixture({news:{fetch:async()=>{fetches++;return {items:[{id:"1"},{id:"2"}]};},markUsed:async()=>{}}});
  await f.scheduler.start(settings,{script,audio});await flush();
  assert.equal(f.scheduler.getState().phase,"waiting");assert.equal(fetches,0);
  await f.advance(1000);assert.equal(f.events.includes("played"),true);assert.equal(fetches,1);
  await f.scheduler.stop();
});
test("wake skips a missed deadline even when monotonic time continued during sleep",async()=>{
  const f=fixture();await f.scheduler.start(settings);await flush();f.sleep(60_000);await flush();
  assert.equal(f.events.includes("skipped:sleep"),true);await f.scheduler.stop();
});
test("native status moves through ducking, playing and restoring",async()=>{
  const playback=deferred();let phase="ducking";
  const f=fixture({player:{play:()=>playback.promise,stop:async()=>{},getStatus:async()=>({phase})}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  await new Promise(resolve=>setTimeout(resolve,280));phase="playing";
  await new Promise(resolve=>setTimeout(resolve,280));phase="restoring";
  await new Promise(resolve=>setTimeout(resolve,280));playback.resolve({phase:"ended"});await flush();
  assert.ok(f.states.some(s=>s.phase==="ducking"));
  assert.ok(f.states.some(s=>s.phase==="playing"));
  assert.ok(f.states.some(s=>s.phase==="restoring"));
  await f.scheduler.stop();
});
test("stop reports cancellation failure after waiting for work to settle",async()=>{
  const synthesis=deferred();const f=fixture({speech:{synthesize:()=>synthesis.promise,cancel:async()=>{throw Error("cancel failed");}}});
  await f.scheduler.start(settings);await flush();
  const stopping=f.scheduler.stop();synthesis.reject(Error("cancelled"));
  await assert.rejects(stopping,/cancel failed/);
  assert.equal(f.scheduler.getState().phase,"idle");assert.equal(f.timerCount(),0);
});

test("all rejected stories wait normally without an error toast and next cycle succeeds",async()=>{
  let attempts=0;const f=fixture({content:{generate:async()=>{if(++attempts===1)throw new NoUsableMaterial("Wszystkie wiadomości były powtórzone.");return script;}}});
  await f.scheduler.start(settings);await flush();
  assert.equal(f.scheduler.getState().phase,"waiting");assert.equal(f.scheduler.getState().ready,false);
  assert.ok(!f.events.some(event=>event.startsWith("error:")));
  await f.advance(1000);assert.ok(f.events.includes("skipped:no_material"));
  assert.equal(f.scheduler.getState().cycle,2);assert.equal(f.scheduler.getState().ready,true);
  await f.advance(1000);assert.ok(f.events.includes("played"));await f.scheduler.stop();
});

test("late speech failure resumes same script next cycle and does not block future play",async()=>{
  let scripts=0,speech=0;const f=fixture({content:{generate:async()=>{scripts++;return script;}},speech:{synthesize:async()=>{if(++speech===1)throw Error("TTS failed");return audio;},cancel:async()=>true}});
  await f.scheduler.start(settings);await flush();assert.equal(f.scheduler.getState().phase,"error");
  await f.advance(1000);assert.equal(f.scheduler.getState().ready,true);assert.equal(scripts,1);
  await f.advance(1000);assert.ok(f.events.includes("played"));await f.scheduler.stop();
});

test("recovered complete recording bypasses failed services without schedule error",async()=>{
  const f=fixture({recovery:{restore:async()=>({script,audio}),claim:async()=>{},complete:async()=>{}},news:{fetch:async()=>{throw Error("RSS must not run");},markUsed:async()=>{}}});
  await f.scheduler.start(settings);await flush();assert.equal(f.scheduler.getState().ready,true);
  f.ports.news.fetch=async()=>({items:[],topics:[]});
  await f.advance(1000);assert.ok(f.events.includes("played"));assert.ok(!f.events.some(event=>event.startsWith("error:")));
  await f.scheduler.stop();
});

test("history write failure after playback is not a schedule failure",async()=>{
  const f=fixture({news:{fetch:async()=>({items:[{id:"1"}]}),markUsed:async()=>{throw Error("history unavailable");}}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  assert.ok(f.events.includes("played"));assert.ok(!f.events.some(event=>event.startsWith("error:")));
  assert.equal(f.scheduler.getState().cycle,2);await f.scheduler.stop();
});

test("pause freezes the remaining interval and ready recording through wake without touching music",async()=>{
  let fetches=0,plays=0;
  const f=fixture({news:{fetch:async()=>{fetches++;return {items:[{id:"1"}]};},markUsed:async()=>{}},
    player:{play:async request=>{plays++;assert.equal(request.resourceId,audio.resourceId);return {phase:"ended"};},stop:async()=>{},getStatus:async()=>({phase:"playing"}),setPaused:async()=>{assert.fail("waiting must not touch audio");}}});
  await f.scheduler.start(settings,{script,audio});await f.advance(350);
  await Promise.all([f.scheduler.setPaused(true),f.scheduler.setPaused(true)]);
  assert.equal(f.scheduler.getState().remainingMs,650);assert.equal(f.scheduler.getState().nextBroadcastAt,null);
  assert.equal(f.timerCount(),0);
  await f.advance(20_000);f.sleep(60_000);await flush();
  assert.equal(plays,0);assert.equal(fetches,0);assert.equal(f.scheduler.getState().ready,true);
  await f.scheduler.setPaused(false);await f.advance(649);assert.equal(plays,0);
  await f.advance(1);assert.equal(plays,1);assert.equal(fetches,1);
  assert.ok(!f.events.includes("skipped:sleep"));await f.scheduler.stop();
});

test("preparation finishes during pause and keeps its recording for resume",async()=>{
  const synthesis=deferred();let calls=0;
  const f=fixture({speech:{synthesize:()=>{calls++;return synthesis.promise;},cancel:async()=>true}});
  await f.scheduler.start(settings);await flush();await f.advance(200);await f.scheduler.setPaused(true);
  synthesis.resolve(audio);await flush();await f.advance(10_000);
  assert.equal(f.scheduler.getState().phase,"waiting");assert.equal(f.scheduler.getState().paused,true);
  assert.equal(f.scheduler.getState().ready,true);assert.equal(f.scheduler.getState().remainingMs,800);
  assert.ok(!f.events.includes("played"));
  await f.scheduler.setPaused(false);await f.advance(800);assert.ok(f.events.includes("played"));
  assert.equal(calls,2);await f.scheduler.stop();
});

test("repeated playback pauses resume the same native playback and prepare the next only at actual end",async()=>{
  const playback=deferred();const controls=[];let plays=0,fetches=0,key;
  const f=fixture({news:{fetch:async()=>{fetches++;return {items:[{id:"1"}]};},markUsed:async()=>{}},
    player:{play:request=>{plays++;key=request.playbackKey;return playback.promise;},stop:async()=>{},getStatus:async()=>({phase:"playing"}),
      setPaused:async(paused,playbackKey)=>{assert.equal(playbackKey,key);controls.push(paused);return {phase:paused?"paused":"playing"};}}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  for(let i=0;i<3;i++) {
    await Promise.all([f.scheduler.setPaused(true),f.scheduler.setPaused(true)]);
    await f.advance(5000);f.scheduler.wake();await flush();
    assert.equal(f.scheduler.getState().paused,true);assert.equal(plays,1);assert.equal(fetches,1);
    await f.scheduler.setPaused(false);
  }
  assert.deepEqual(controls,[true,false,true,false,true,false]);
  playback.resolve({phase:"ended"});await flush();
  assert.equal(fetches,2);assert.equal(f.scheduler.getState().cycle,2);
  assert.equal(f.scheduler.getState().remainingMs,1000);await f.scheduler.stop();
});

test("pause arriving during source lookup prevents native playback until resume",async()=>{
  const lookup=deferred();let plays=0;
  const f=fixture({sourceAvailable:()=>lookup.promise,player:{play:async()=>{plays++;return {phase:"ended"};},stop:async()=>{},getStatus:async()=>({phase:"idle"})}});
  await f.scheduler.start(settings,{script,audio});await f.advance(1000);
  await f.scheduler.setPaused(true);lookup.resolve(true);await flush();
  assert.equal(plays,0);await f.advance(5000);assert.equal(plays,0);
  await f.scheduler.setPaused(false);await flush();assert.equal(plays,1);
  await f.scheduler.stop();
});

test("playback failure during pause keeps the next cycle frozen and stop clears it",async()=>{
  const playback=deferred();let stops=0;
  const f=fixture({player:{play:()=>playback.promise,stop:async()=>{stops++;},getStatus:async()=>({phase:"playing"}),setPaused:async()=>({phase:"paused"})}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);await f.scheduler.setPaused(true);
  playback.reject(Error("output disconnected"));await flush();await f.advance(5000);
  assert.equal(stops,1);assert.ok(f.events.includes("error:output disconnected"));
  assert.equal(f.scheduler.getState().cycle,2);assert.equal(f.scheduler.getState().paused,true);assert.equal(f.timerCount(),0);
  await f.scheduler.stop();f.scheduler.wake();await flush();
  assert.equal(f.scheduler.getState().paused,false);assert.equal(f.scheduler.getState().phase,"idle");
  assert.ok(!f.events.includes("used"));
});

test("stop during pending native pause waits for cleanup without replay or a new cycle",async()=>{
  const playback=deferred(),pause=deferred();let calls=0;
  const f=fixture({player:{play:()=>{calls++;return playback.promise;},stop:async()=>{playback.resolve({phase:"stopped"});pause.reject(Error("stopped"));},
    getStatus:async()=>({phase:"restoring"}),setPaused:()=>pause.promise}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  const pausing=f.scheduler.setPaused(true);const rejected=assert.rejects(pausing,/stopped/);
  await f.scheduler.stop();await rejected;f.scheduler.wake();await flush();
  assert.equal(calls,1);assert.equal(f.timerCount(),0);assert.equal(f.scheduler.getState().phase,"idle");assert.ok(!f.events.includes("used"));
});

test("actual end racing with pause leaves the following interval frozen",async()=>{
  const playback=deferred(),pause=deferred();
  const f=fixture({player:{play:()=>playback.promise,stop:async()=>{},getStatus:async()=>({phase:"playing"}),setPaused:()=>pause.promise}});
  await f.scheduler.start(settings);await flush();await f.advance(1000);
  const pausing=f.scheduler.setPaused(true);playback.resolve({phase:"ended"});await flush();
  assert.equal(f.events.filter(e=>e==="played").length,1);assert.equal(f.scheduler.getState().cycle,2);
  pause.resolve({phase:"ended"});await pausing;await f.advance(5000);
  assert.equal(f.scheduler.getState().paused,true);assert.equal(f.scheduler.getState().remainingMs,1000);assert.equal(f.timerCount(),0);
  await f.scheduler.setPaused(false);await f.advance(999);assert.equal(f.scheduler.getState().cycle,2);
  await f.scheduler.stop();
});
