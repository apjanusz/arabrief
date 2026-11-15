import type { AppSettings, BroadcastScript, SpeechAsset } from "../../domain/models";
import type { AudioPlayer } from "../audio/AudioPlayer";
import type { NewsProvider, NewsResult } from "../news/NewsProvider";
import type { ContentGenerator } from "../content/ContentGenerator";
import type { SpeechProvider } from "../speech/SpeechProvider";

export type PreparationStage = "idle" | "news" | "content" | "speech" | "playing";
export interface PreparedBroadcast { script: BroadcastScript; audio: SpeechAsset }
interface Ports {
  news: NewsProvider; content: ContentGenerator; speech: SpeechProvider; player: AudioPlayer;
  onStage(stage: PreparationStage): void; onNews(result: NewsResult): void;
  onReady(prepared: PreparedBroadcast): void; onPlayed(prepared: PreparedBroadcast): void;
  makeJobId(): string;
}
/** Explicit manual action, not a timer or an implementation of BroadcastScheduler. */
export class ManualBroadcastRunner {
  private active: Promise<void> | null = null;
  private cancelled = false;
  private job: string | null = null;
  private playing = false;
  private ports: Ports;
  constructor(ports: Ports) { this.ports = ports; }
  run(settings: AppSettings, prepareOnly: boolean, ready?: PreparedBroadcast): Promise<void> {
    if (this.active) return Promise.reject(new Error("Audycja jest już przygotowywana."));
    this.cancelled = false;
    const work = this.execute(settings, prepareOnly, ready);
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
  private async execute(settings: AppSettings, prepareOnly: boolean, ready?: PreparedBroadcast): Promise<void> {
    if (!prepareOnly && !settings.preferredAudioSource) throw new Error("Wybierz źródło muzyki w ustawieniach.");
    let prepared = ready;
    if (!prepared) {
      if (settings.topics.length === 0) throw new Error("Dodaj zainteresowania przed przygotowaniem audycji.");
      this.ports.onStage("news");
      const news = await this.ports.news.fetch({ topics: settings.topics });
      this.ports.onNews(news); this.check();
      if (news.items.length < 2) throw new Error("Brakuje co najmniej dwóch nowych wiadomości. Sprawdź źródła i limity RSS.");
      this.ports.onStage("content");
      const script = await this.ports.content.generate(news.items.slice(0, 8).map(item => item.id));
      this.check(); this.ports.onStage("speech");
      this.job = this.ports.makeJobId();
      const audio = await this.ports.speech.synthesize(script.id, settings.openAiVoice, this.job);
      this.job = null; this.check();
      prepared = { script, audio }; this.ports.onReady(prepared);
    }
    this.check();
    if (prepareOnly) return;
    if (!settings.preferredAudioSource) throw new Error("Wybierz źródło muzyki w ustawieniach.");
    this.ports.onStage("playing"); this.playing = true;
    const result = await this.ports.player.play({ resourceId: prepared.audio.resourceId, musicSessionId: settings.preferredAudioSource.id });
    this.playing = false;
    // Native play resolves after actual end AND volume restoration, also when stopped.
    if (result.phase !== "ended") { this.check(); throw new Error("Nagranie zostało zatrzymane przed końcem."); }
    this.ports.onPlayed(prepared);
    await this.ports.news.markUsed(prepared.script.sources.map(source => source.id), prepared.script.id);
  }
}
