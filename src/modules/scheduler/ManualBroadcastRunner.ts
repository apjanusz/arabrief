import type { AppSettings, BroadcastScript, SpeechAsset } from "../../domain/models";
import type { AudioPlayer } from "../audio/AudioPlayer";
import type { NewsProvider, NewsResult } from "../news/NewsProvider";
import type { ContentGenerator } from "../content/ContentGenerator";
import type { SpeechProvider } from "../speech/SpeechProvider";
import { NoUsableMaterial } from "./PreparationRecovery.ts";
import type { PreparationRecovery } from "./PreparationRecovery";

export type PreparationStage = "idle" | "news" | "content" | "speech" | "playing";
export interface PreparedBroadcast { script: BroadcastScript; audio: SpeechAsset }
interface Ports {
  recovery?: PreparationRecovery;
  onRecovered?(message: string): void;
  onConsumed?(broadcast: PreparedBroadcast): void;
  news: NewsProvider; content: ContentGenerator; speech: SpeechProvider; player: AudioPlayer;
  onStage(stage: PreparationStage): void; onNews(result: NewsResult): void;
  onReady(prepared: PreparedBroadcast): void; onPlayed(prepared: PreparedBroadcast): void;
  makeJobId(): string;
  onTiming?(event: { operation: string; elapsedMs: number; status: "ok" | "error" }): void;
}
/** Explicit manual action, not a timer or an implementation of BroadcastScheduler. */
export class ManualBroadcastRunner {
  private active: Promise<void> | null = null;
  private cancelled = false;
  private job: string | null = null;
  private playing = false;
  private pending: BroadcastScript | null = null;
  private fallback: PreparedBroadcast | null = null;
  private settingsKey = "";
  private readonly consumed = new Set<string>();
  private ports: Ports;
  constructor(ports: Ports) { this.ports = ports; }
  private async measured<T>(operation: string, action: () => Promise<T>): Promise<T> {
    const started=performance.now();let status: "ok" | "error"="error";
    try {const result=await action();status="ok";return result;}
    finally {
      const event={operation,elapsedMs:Math.round((performance.now()-started)*1000)/1000,status};
      // Diagnostics must not turn a successful preparation into an error.
      try {if(this.ports.onTiming)this.ports.onTiming(event);else console.info("[perf]",event);} catch { /* isolated observer */ }
    }
  }
  run(settings: AppSettings, prepareOnly: boolean, ready?: PreparedBroadcast, fresh = false): Promise<void> {
    if (this.active) return Promise.reject(new Error("Audycja jest już przygotowywana."));
    this.cancelled = false;
    const work = this.measured(prepareOnly ? "prepare.total" : "broadcast.total",()=>this.execute(settings, prepareOnly, ready, fresh));
    this.active = work.finally(() => { this.active = null; this.job = null; this.playing = false; this.ports.onStage("idle"); });
    return this.active;
  }
  async stop(): Promise<void> {
    this.cancelled = true;
    const work = this.active;
    let failure: unknown;
    if (this.job) try { await this.ports.speech.cancel(this.job); } catch (cause) { failure = cause; }
    if (this.playing) try { await this.ports.player.stop(); } catch (cause) { failure ??= cause; }
    await work?.catch(() => {});
    if (failure) throw failure;
  }
  private check(): void { if (this.cancelled) throw new Error("Anulowano przygotowanie audycji."); }
  private recovered(message:string): void {
    console.info("[recovery]",message);
    try {this.ports.onRecovered?.(message);} catch { /* observer cannot lose good work */ }
  }
  private async execute(settings: AppSettings, prepareOnly: boolean, ready?: PreparedBroadcast, fresh = false): Promise<void> {
    if (!prepareOnly && !settings.preferredAudioSource) throw new Error("Wybierz źródło muzyki w ustawieniach.");
    const key=JSON.stringify([settings.topics,settings.openAiVoice,settings.textModel,settings.broadcastMinDurationMs,settings.broadcastMaxDurationMs,settings.language,settings.newsSourceUrls]);
    if (key!==this.settingsKey) {this.pending=null;this.fallback=null;this.settingsKey=key;}
    if(ready && this.consumed.has(ready.script.id)) throw new Error("Ta audycja została już rozpoczęta. Przygotuj nową.");
    let prepared = ready;
    if (!prepared) {
      if(this.ports.recovery) {
        const restored=await this.ports.recovery.restore();this.check();
        // Native receipts are authoritative across manual and automatic runners.
        this.pending=null;this.fallback=null;
        if(restored && !this.consumed.has(restored.script.id)) {
          if(restored.audio)this.fallback={script:restored.script,audio:restored.audio};
          else this.pending=restored.script;
        }
      }
      if(!fresh && this.fallback) prepared=this.fallback;
    }
    if (!prepared) {
      try {
        const script = await this.prepareScript(settings, fresh);
        this.check(); this.ports.onStage("speech");
        this.job = this.ports.makeJobId();
        const job = this.job;
        const audio = await this.measured("prepare.speech", () => this.ports.speech.synthesize(script.id, settings.openAiVoice, job));
        this.job = null; this.check();
        prepared = { script, audio }; this.pending = null; this.fallback = prepared;
      } catch(cause) {
        if(this.cancelled) console.info("[recovery]",{stage:"preparation",action:"cancelled_preserved_work"});
        this.check();
        console.info("[recovery]",{stage:"preparation",action:this.fallback ? "fallback" : "deferred",reason:cause instanceof Error ? cause.name : "service_failure"});
        if(!this.fallback) throw cause;
        prepared=this.fallback;
        this.recovered("Nowe przygotowanie nie powiodło się. Zachowano i wykorzystano poprzednią gotową, nieodtworzoną audycję.");
      }
    }
    this.check();
    this.ports.onReady(prepared);
    if (prepareOnly) return;
    if (!settings.preferredAudioSource) throw new Error("Wybierz źródło muzyki w ustawieniach.");
    // Persist before playback: a crash must not cause a partially heard story to repeat.
    await this.ports.recovery?.claim(prepared.script.id);
    this.consumed.add(prepared.script.id);this.fallback=null;
    this.ports.onConsumed?.(prepared);
    this.check();
    this.ports.onStage("playing"); this.playing = true;
    const result = await this.ports.player.play({ resourceId: prepared.audio.resourceId, musicSessionId: settings.preferredAudioSource.id });
    this.playing = false;
    // Native play resolves after actual end AND volume restoration, also when stopped.
    if (result.phase !== "ended") { this.check(); throw new Error("Nagranie zostało zatrzymane przed końcem."); }
    this.ports.onPlayed(prepared);
    try {
      await this.ports.recovery?.complete(prepared.script.id);
      await this.ports.news.markUsed(prepared.script.sources.map(source => source.id), prepared.script.id);
    } catch {
      this.recovered("Audycja zakończona. Nie zapisano pełnej historii odsłuchu; zabezpieczenie przed ponownym odtworzeniem pozostaje aktywne.");
    }
  }
  private async prepareScript(settings: AppSettings, fresh: boolean): Promise<BroadcastScript> {
    if (settings.topics.length === 0) throw new Error("Dodaj zainteresowania przed przygotowaniem audycji.");
    if (!fresh && this.pending) {
      this.recovered("Wznowiono zapisany scenariusz bez ponownego generowania tekstu.");
      return this.pending;
    }
    this.ports.onStage("news");
    const news = await this.measured("prepare.news", () => this.ports.news.fetch({ topics: settings.topics }));
    this.ports.onNews(news); this.check();
    if (news.items.length === 0) {
      if (news.topics?.some(topic => topic.error)) throw new Error("Nie udało się pobrać wiadomości z żadnego źródła. Spróbujemy w kolejnym cyklu.");
      throw new NoUsableMaterial();
    }
    this.ports.onStage("content");
    const script = await this.measured("prepare.content", () => this.ports.content.generate(news.items.slice(0, 8).map(item => item.id)));
    this.pending = script;
    return script;
  }
}
