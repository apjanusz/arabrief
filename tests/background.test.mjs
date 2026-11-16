import test, { afterEach } from "node:test";
import assert from "node:assert/strict";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { backgroundLifecycle } from "../src/platform/tauri/background.ts";

const flush = async () => { for (let i=0;i<5;i++) await new Promise(resolve=>setImmediate(resolve)); };
const launch = { closing:false, startScheduler:false, recoveryMessage:null };
function fixture(state=launch) {
  globalThis.window={crypto:globalThis.crypto}; globalThis.isTauri=true;
  const calls=[];
  mockIPC((command,args)=>{calls.push({command,args});if(command==="background_connect")return state;}, {shouldMockEvents:true});
  return calls;
}
afterEach(()=>{clearMocks();delete globalThis.window;delete globalThis.isTauri;});

test("connecting never starts playback; tray actions and native ticks reach the existing scheduler",async()=>{
  const calls=fixture(); const actions=[];
  const connection=await backgroundLifecycle.connect({toggle:async()=>{actions.push("toggle");},quit:async()=>{actions.push("quit");},wake:()=>actions.push("wake")});
  assert.deepEqual(connection.launch,launch); assert.deepEqual(actions,[]);
  await emit("background-action","invalid"); await emit("background-tick",null); await emit("background-action","toggle");
  assert.deepEqual(actions,["wake","toggle"]);
  await backgroundLifecycle.update({active:true,canStart:false});
  assert.deepEqual(calls.at(-1),{command:"background_status",args:{status:{active:true,canStart:false}}});
  connection.dispose(); await flush(); assert.equal(window.__TAURI_INTERNALS__.callbacks.size,0);
});

test("quit cancels once, ignores further toggles and acknowledges only after restoration",async()=>{
  const calls=fixture(); let finish; const pending=new Promise(resolve=>{finish=resolve;}); let quits=0, toggles=0;
  const connection=await backgroundLifecycle.connect({toggle:async()=>{toggles++;},quit:async()=>{quits++;await pending;},wake:()=>{}});
  await emit("background-action","quit"); await emit("background-action","quit"); await emit("background-action","toggle");
  assert.equal(quits,1); assert.equal(toggles,0); assert.equal(calls.some(c=>c.command==="background_stopped"),false);
  finish(); await flush(); assert.equal(calls.filter(c=>c.command==="background_stopped").length,1);
  connection.dispose();
});

test("handshake catches quit requested before listeners were ready",async()=>{
  const calls=fixture({...launch,closing:true}); let quits=0;
  const connection=await backgroundLifecycle.connect({toggle:async()=>assert.fail(),quit:async()=>{quits++;},wake:()=>{}});
  await flush(); assert.equal(quits,1); assert.equal(calls.filter(c=>c.command==="background_stopped").length,1);
  connection.dispose();
});

test("invalid IPC response rejects and removes all listeners",async()=>{
  fixture({...launch,startScheduler:"false"});
  await assert.rejects(backgroundLifecycle.connect({toggle:async()=>{},quit:async()=>{},wake:()=>{}}),/Nieprawidłowa/);
  assert.equal(window.__TAURI_INTERNALS__.callbacks.size,0);
});

test("browser preview registers no native background services",async()=>{
  fixture();globalThis.isTauri=false;
  assert.equal(await backgroundLifecycle.connect({toggle:async()=>assert.fail(),quit:async()=>assert.fail(),wake:()=>assert.fail()}),null);
});

test("invalid scheduler status is rejected before crossing IPC",async()=>{
  const calls=fixture();
  await assert.rejects(backgroundLifecycle.update({active:true,canStart:true}),/Nieprawidłowy/);
  await assert.rejects(backgroundLifecycle.update({active:"true",canStart:false}),/Nieprawidłowy/);
  assert.equal(calls.length,0);
});
