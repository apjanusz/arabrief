import type { AppSettings, BroadcastPhase, BroadcastState } from "../../domain/models";
import type { AudioPlayer } from "../audio/AudioPlayer";
import type { NewsProvider, NewsResult } from "../news/NewsProvider";
import type { ContentGenerator } from "../content/ContentGenerator";
import type { SpeechProvider } from "../speech/SpeechProvider";
import { ManualBroadcastRunner } from "./ManualBroadcastRunner.ts";
import type { PreparedBroadcast, PreparationStage } from "./ManualBroadcastRunner";
import type { BroadcastScheduler } from "./BroadcastScheduler";
import { NoUsableMaterial } from "./PreparationRecovery.ts";
import type { PreparationRecovery } from "./PreparationRecovery";

interface Clock {
  mono(): number;
  wall(): number;
  set(callback: () => void, delayMs: number): ReturnType<typeof setTimeout>;
  clear(handle: ReturnType<typeof setTimeout>): void;
}
interface Ports {
  recovery?: PreparationRecovery;
  onRecovered?(message: string): void;
  onConsumed?(broadcast: PreparedBroadcast): void;
  news: NewsProvider; content: ContentGenerator; speech: SpeechProvider; player: AudioPlayer;
  makeJobId(): string;
  onStage(stage: PreparationStage): void;
  onNews(result: NewsResult): void;
  onReady(broadcast: PreparedBroadcast): void;
  onPlayed(broadcast: PreparedBroadcast): void;
  onSkipped(reason: "not_ready" | "sleep" | "source_unavailable" | "no_material"): void;
  onError(message: string): void;
  log(event: string): void;
  sourceAvailable(sessionId: string): Promise<boolean>;
  clock?: Clock;
}
export interface SchedulerDiagnostic { at: string; event: string; phase: BroadcastPhase; cycle: number }

const browserClock: Clock = {
  mono: () => performance.now(), wall: () => Date.now(),
  set: (callback, delay) => setTimeout(callback, delay), clear: handle => clearTimeout(handle),
};
const errorMessage = (cause: unknown) => cause instanceof Error ? cause.message : String(cause);
const isPlaying = (phase: BroadcastPhase) => phase === "ducking" || phase === "playing" || phase === "restoring";
const transitions: Record<BroadcastPhase, readonly BroadcastPhase[]> = {
  idle: ["preparing", "waiting"],
  preparing: ["waiting", "error", "idle", "preparing"],
  waiting: ["ducking", "error", "idle", "preparing"],
  ducking: ["playing", "restoring", "error", "idle", "preparing"],
  playing: ["restoring", "error", "idle", "preparing"],
  restoring: ["error", "idle", "preparing"],
  error: ["preparing", "waiting", "idle"],
};

/** One timer, one preparation and one native playback. Rust owns ducking and restoration. */
export class AutomaticBroadcastScheduler implements BroadcastScheduler {
  private readonly ports: Ports;
  private readonly diagnostics: SchedulerDiagnostic[] = [];
  private readonly clock: Clock;
  private readonly runner: ManualBroadcastRunner;
  private readonly listeners = new Set<(state: BroadcastState) => void>();
  private state: BroadcastState = { phase: "idle", nextBroadcastAt: null, remainingMs: null, ready: false, message: null, cycle: 0 };
  private settings: AppSettings | null = null;
  private prepared: PreparedBroadcast | null = null;
  private generation = 0;
  private dueMono = 0;
  private dueWall = 0;
  private observedMono = 0;
  private observedWall = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private preparation: Promise<void> | null = null;
  private playback: Promise<void> | null = null;
  private stopping: Promise<void> | null = null;
  private statusPoll: ReturnType<typeof setInterval> | null = null;
  private polling = false;
  private ticking = false;
  private noMaterial = false;
  constructor(ports: Ports) {
    this.ports = ports;
    this.clock = ports.clock ?? browserClock;
    this.runner = new ManualBroadcastRunner({ ...ports,
      onReady: broadcast => { this.prepared = broadcast; ports.onReady(broadcast); },
    });
  }
  getState(): BroadcastState { return this.state; }
  getDiagnostics(): readonly SchedulerDiagnostic[] { return [...this.diagnostics]; }
  private log(event: string) {
    this.diagnostics.push({ at: new Date(this.clock.wall()).toISOString(), event, phase: this.state.phase, cycle: this.state.cycle });
    if (this.diagnostics.length > 100) this.diagnostics.shift();
    this.ports.log(event);
  }
  subscribe(listener: (state: BroadcastState) => void) { this.listeners.add(listener); listener(this.state); return () => { this.listeners.delete(listener); }; }
  private emit(patch: Partial<BroadcastState>) {
    if (patch.phase && patch.phase !== this.state.phase && !transitions[this.state.phase].includes(patch.phase)) {
      this.log("invalid_state_transition");
      throw new Error(`Niedozwolone przejście harmonogramu: ${this.state.phase} → ${patch.phase}.`);
    }
    this.state = { ...this.state, ...patch };
    this.listeners.forEach(listener => listener(this.state));
  }
  private clearTimer() { if (this.timer !== null) { this.clock.clear(this.timer); this.timer = null; } }
  async start(settings: AppSettings, ready?: PreparedBroadcast) {
    if (this.settings || this.stopping) throw new Error("Harmonogram już działa lub jest zatrzymywany.");
    if (!settings.preferredAudioSource) throw new Error("Wybierz źródło muzyki.");
    if (!settings.topics.length) throw new Error("Dodaj zainteresowania przed uruchomieniem harmonogramu.");
    if (!Number.isSafeInteger(settings.intervalMs) || settings.intervalMs < 1_000) throw new Error("Nieprawidłowy interwał audycji.");
    this.settings = settings;
    this.log("start");
    this.beginCycle(ready);
  }
  private beginCycle(seed?: PreparedBroadcast) {
    const settings = this.settings;
    if (!settings) return;
    const generation = ++this.generation;
    this.prepared = seed ?? null;
    this.noMaterial = false;
    this.dueMono = this.clock.mono() + settings.intervalMs;
    this.dueWall = this.clock.wall() + settings.intervalMs;
    this.observedMono = this.clock.mono(); this.observedWall = this.clock.wall();
    this.emit({ phase: seed ? "waiting" : "preparing", nextBroadcastAt: new Date(this.dueWall).toISOString(), remainingMs: settings.intervalMs, ready: !!seed, message: null, cycle: this.state.cycle + 1 });
    this.log("cycle_started");
    if (seed) { this.log("existing_audio_ready"); this.arm(); return; }
    this.preparation = this.runner.run(settings, true).then(() => {
      if (generation !== this.generation || !this.settings) return;
      this.emit({ phase: "waiting", ready: !!this.prepared });
      this.log("audio_ready");
    }).catch(cause => {
      if (generation !== this.generation || !this.settings) return;
      const message = errorMessage(cause);
      if(cause instanceof NoUsableMaterial) {
        this.noMaterial=true;
        this.emit({phase:"waiting",message,ready:false});
        this.log("preparation_no_usable_material_next_cycle_pending");
        return;
      }
      this.emit({ phase: "error", message, ready: false });
      this.log("preparation_failed");
      this.ports.onError(message);
    }).finally(() => { if (generation === this.generation) this.preparation = null; });
    this.arm();
  }
  private arm() {
    this.clearTimer();
    if (!this.settings || this.playback || this.stopping) return;
    const remaining = Math.max(0, this.dueMono - this.clock.mono());
    this.timer = this.clock.set(() => { this.timer = null; void this.tick(); }, Math.min(1_000, remaining));
  }
  wake() { if (this.settings && !this.playback && !this.stopping) void this.tick(); }
  private async tick() {
    if (this.ticking) return;
    this.ticking = true;
    try {
    const settings = this.settings;
    if (!settings || this.playback || this.stopping) return;
    const nowMono = this.clock.mono(), nowWall = this.clock.wall();
    const slept = nowWall - this.observedWall - (nowMono - this.observedMono) > 5_000;
    this.observedMono = nowMono; this.observedWall = nowWall;
    if ((slept || nowWall - this.dueWall > 5_000) && nowWall >= this.dueWall) {
      this.log("sleep_deadline_skipped");
      await this.skip("sleep");
      return;
    }
    const remaining = Math.max(0, this.dueMono - nowMono);
    this.emit({ remainingMs: remaining });
    if (remaining > 0) { this.arm(); return; }
    this.clearTimer();
    if (!this.prepared) { await this.skip(this.noMaterial ? "no_material" : "not_ready"); return; }
    const prepared = this.prepared;
    this.prepared = null;
    this.playback = this.play(prepared, settings);
    await this.playback;
    this.playback = null;
    if (this.settings && !this.stopping) this.beginCycle();
    } finally { this.ticking = false; }
  }
  private async skip(reason: "not_ready" | "sleep" | "no_material") {
    this.log(`cycle_skipped_${reason}`);
    ++this.generation;
    this.prepared = null;
    this.ports.onSkipped(reason);
    try { await this.runner.stop(); }
    catch { this.log("preparation_cancel_failed"); }
    if (this.settings && !this.stopping) this.beginCycle();
  }
  private async play(prepared: PreparedBroadcast, settings: AppSettings) {
    try {
      const sourceId = settings.preferredAudioSource!.id;
      if (!(await this.ports.sourceAvailable(sourceId))) {
        if (!this.settings || this.stopping) return;
        this.log("source_unavailable"); this.ports.onSkipped("source_unavailable");
        this.settings = null;
        this.emit({ phase: "error", nextBroadcastAt: null, remainingMs: null, ready: false, message: "Wybrana sesja audio zniknęła. Wybierz aktualne źródło i uruchom ponownie." });
        return;
      }
      if (!this.settings || this.stopping) return;
      this.emit({ phase: "ducking", remainingMs: 0, ready: false });
      this.log("play_requested");
      this.statusPoll = setInterval(() => void this.pollStatus(), 250);
      await this.runner.run(settings, false, prepared);
      this.log("play_completed");
    } catch (cause) {
      if (!this.settings || this.stopping) return;
      // Native play restores on every exit path; stop is an additional idempotent recovery attempt.
      this.emit({ phase: "restoring" });
      try { await this.ports.player.stop(); } catch { this.log("restore_attempt_failed"); }
      const message = errorMessage(cause);
      this.emit({ phase: "error", message });
      this.log("play_failed"); this.ports.onError(message);
    } finally {
      if (this.statusPoll !== null) { clearInterval(this.statusPoll); this.statusPoll = null; }
    }
  }
  private async pollStatus() {
    if (this.polling || !this.settings) return;
    this.polling = true;
    try {
      const status = await this.ports.player.getStatus();
      if (this.settings && !this.stopping && isPlaying(status.phase as BroadcastPhase)) this.emit({ phase: status.phase as BroadcastPhase });
    } catch { this.log("status_unavailable"); }
    finally { this.polling = false; }
  }
  stop(): Promise<void> {
    if (this.stopping) return this.stopping;
    if (!this.settings) return Promise.resolve();
    this.settings = null; ++this.generation;
    this.clearTimer();
    this.log("stop_requested");
    this.stopping = (async () => {
      let failure: unknown;
      try {
        try { await this.runner.stop(); } catch (cause) { failure = cause; this.log("stop_recovery_failed"); }
        await this.preparation?.catch(() => {});
        await this.playback?.catch(() => {});
      }
      finally {
        if (this.statusPoll !== null) { clearInterval(this.statusPoll); this.statusPoll = null; }
        this.prepared = null;
        this.emit({ phase: "idle", nextBroadcastAt: null, remainingMs: null, ready: false, message: null });
        this.ports.onStage("idle"); this.log("stopped"); this.stopping = null;
      }
      if (failure) throw failure;
    })();
    return this.stopping;
  }
}
