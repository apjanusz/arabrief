import { useEffect, useRef, useState } from "react";
import type { AppSettings, BroadcastState } from "../domain/models";
import type { AudioSession, AudioController } from "../modules/audio/AudioController";
import type { AudioPlayer, PlaybackStatus } from "../modules/audio/AudioPlayer";
import type { SettingsRepository } from "../modules/settings/SettingsRepository";
import type { NewsProvider, NewsResult } from "../modules/news/NewsProvider";
import type { ContentGenerator } from "../modules/content/ContentGenerator";
import type { SpeechProvider } from "../modules/speech/SpeechProvider";
import { ManualBroadcastRunner } from "../modules/scheduler/ManualBroadcastRunner";
import type { PreparationStage, PreparedBroadcast } from "../modules/scheduler/ManualBroadcastRunner";
import { AutomaticBroadcastScheduler } from "../modules/scheduler/AutomaticBroadcastScheduler";
import { notifications } from "./notifications/notifications";
import { NoUsableMaterial } from "../modules/scheduler/PreparationRecovery";
import type { PreparationRecovery } from "../modules/scheduler/PreparationRecovery";

export interface Services { recovery: PreparationRecovery; settings: SettingsRepository; audio: Pick<AudioController, "listSessions">; player: AudioPlayer; news: NewsProvider; content: ContentGenerator; speech: SpeechProvider }
const activePhases = ["preparing", "ducking", "playing", "restoring"];
export const errorText = (cause: unknown): string => cause instanceof Error ? cause.message : typeof cause === "object" && cause !== null && "message" in cause ? String(cause.message) : String(cause);
export function useAraBrief(services: Services) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [sessions, setSessions] = useState<AudioSession[]>([]);
  const [playback, setPlayback] = useState<PlaybackStatus | null>(null);
  const [stage, setStage] = useState<PreparationStage>("idle");
  const [ready, setReady] = useState<PreparedBroadcast | null>(null);
  const [lastPlayed, setLastPlayed] = useState<{ broadcast: PreparedBroadcast; at: string } | null>(null);
  const [news, setNews] = useState<NewsResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [testing, setTesting] = useState(false);
  const [schedule, setSchedule] = useState<BroadcastState>({ phase: "idle", nextBroadcastAt: null, remainingMs: null, ready: false, message: null, cycle: 0 });
  const stopRequested = useRef(false);
  const runner = useRef<ManualBroadcastRunner | null>(null);
  const scheduler = useRef<AutomaticBroadcastScheduler | null>(null);
  if (!runner.current) runner.current = new ManualBroadcastRunner({ ...services,
    onRecovered: message => notifications.info(message, {key:"preparation-recovery"}),
    onConsumed: () => setReady(null),
    onStage: setStage, onNews: setNews, onReady: setReady,
    onPlayed: broadcast => setLastPlayed({ broadcast, at: new Date().toISOString() }),
    makeJobId: () => crypto.randomUUID(),
  });
  if (!scheduler.current) scheduler.current = new AutomaticBroadcastScheduler({ ...services,
    onRecovered: message => notifications.info(message, {key:"preparation-recovery"}),
    onConsumed: () => setReady(null),
    onStage: setStage, onNews: setNews, onReady: setReady,
    onPlayed: broadcast => setLastPlayed({ broadcast, at: new Date().toISOString() }),
    onSkipped: reason => reason === "no_material" ? notifications.info("Brak nowych, bezpiecznych wiadomości. Muzyka gra dalej; sprawdzimy materiały w kolejnym cyklu.", {key:"no-material"}) : notifications.warning(reason === "sleep" ? "Termin audycji minął podczas uśpienia. Rozpoczęto nowy interwał." : reason === "source_unavailable" ? "Wybrana sesja audio zniknęła. Wybierz źródło i uruchom harmonogram ponownie." : "Nagranie nie było gotowe na czas. Pominięto audycję i rozpoczęto nowy interwał."),
    onError: message => notifications.error(message, { title: "Błąd harmonogramu", key: "schedule-error" }),
    makeJobId: () => crypto.randomUUID(),
    sourceAvailable: async id => { const available = await services.audio.listSessions(); setSessions(available); return available.some(session => session.id === id); },
    log: event => console.info(`[AraBrief scheduler] ${event}`),
  });
  useEffect(() => scheduler.current!.subscribe(setSchedule), []);
  useEffect(() => {
    const check = () => scheduler.current?.wake();
    window.addEventListener("focus", check);
    window.addEventListener("pageshow", check);
    document.addEventListener("visibilitychange", check);
    return () => { window.removeEventListener("focus", check); window.removeEventListener("pageshow", check); document.removeEventListener("visibilitychange", check); };
  }, []);
  useEffect(() => {
    let alive = true;
    void Promise.allSettled([services.settings.load(), services.audio.listSessions()]).then(([preferences, audio]) => {
      if (!alive) return;
      if (preferences.status === "fulfilled") setSettings(preferences.value);
      else notifications.error(errorText(preferences.reason), { title: "Nie można wczytać ustawień", key: "settings-load" });
      if (audio.status === "fulfilled") setSessions(audio.value);
      else if (preferences.status === "fulfilled") notifications.error(errorText(audio.reason), { title: "Nie można pobrać sesji audio", key: "audio-sessions" });
      setLoading(false);
    });
    return () => { alive = false; void runner.current?.stop().catch(() => {}); void scheduler.current?.stop().catch(() => {}); };
  }, [services]);
  useEffect(() => {
    if (!settings) return;
    let alive = true, pending = false;
    let lastError = "";
    const refresh = async () => {
      if (pending) return; pending = true;
      try { const state = await services.player.getStatus(); if (alive) { setPlayback(state); lastError = ""; } }
      catch (cause) {
        const message = errorText(cause);
        if (alive && message !== lastError) notifications.error(message, { title: "Nie można odczytać stanu odtwarzacza", key: "player-status" });
        lastError = message;
      }
      finally { pending = false; }
    };
    void refresh(); const timer = setInterval(() => void refresh(), 600);
    return () => { alive = false; clearInterval(timer); };
  }, [!!settings, services]);
  async function save(next: AppSettings) {
    if (saving) throw new Error("Zapis ustawień już trwa.");
    setSaving(true);
    try {
      await services.settings.save(next);
      if (settings && JSON.stringify([settings.topics, settings.openAiVoice, settings.textModel, settings.broadcastMinDurationMs, settings.broadcastMaxDurationMs]) !== JSON.stringify([next.topics, next.openAiVoice, next.textModel, next.broadcastMinDurationMs, next.broadcastMaxDurationMs])) setReady(null);
      setSettings(next); notifications.success("Zapisano zmiany.");
    }
    catch (cause) { notifications.error(errorText(cause), { title: "Nie zapisano zmian" }); throw cause; }
    finally { setSaving(false); }
  }
  async function refreshSessions() {
    try { setSessions(await services.audio.listSessions()); }
    catch (cause) { notifications.error(errorText(cause), { title: "Nie można pobrać sesji audio", key: "audio-sessions" }); }
  }
  async function reconnect() {
    if (loading || busy) return;
    setLoading(true);
    try { const preferences = await services.settings.load(); setSettings(preferences); await refreshSessions(); }
    catch (cause) { notifications.error(errorText(cause), { title: "Nie można wczytać ustawień", key: "settings-load" }); }
    finally { setLoading(false); }
  }
  const source = sessions.find(session => session.id === settings?.preferredAudioSource?.id) ?? null;
  const nativeBusy = !!playback && activePhases.includes(playback.phase);
  const scheduledActive = schedule.phase !== "idle" && (schedule.phase !== "error" || !!schedule.nextBroadcastAt);
  const busy = running || scheduledActive || nativeBusy || testing;
  async function startSchedule() {
    if (!settings || busy || saving) return;
    if (!source) { notifications.warning("Wybierz dostępną sesję muzyki w ustawieniach."); return; }
    const seed = ready && ready.script.id !== lastPlayed?.broadcast.script.id ? ready : undefined;
    if (!seed) setReady(null);
    try { await scheduler.current!.start(settings, seed); notifications.info(seed ? "Harmonogram uruchomiony. Gotowa audycja czeka na termin." : "Harmonogram uruchomiony. Przygotowujemy pierwszą audycję."); }
    catch (cause) { notifications.error(errorText(cause), { title: "Nie uruchomiono harmonogramu" }); }
  }
  async function start(prepareOnly = false, useReady = true) {
    if (!settings || busy || saving) return;
    if (!prepareOnly && !source) { notifications.warning("Zapisane źródło muzyki jest niedostępne. Wybierz aktualną sesję w ustawieniach."); return; }
    setRunning(true);
    stopRequested.current = false;
    try {
      await runner.current!.run(settings, prepareOnly, useReady ? ready ?? undefined : undefined, prepareOnly && !useReady);
      if (!stopRequested.current) notifications.success(prepareOnly ? "Audycja gotowa do odsłuchu." : "Audycja zakończona.");
    } catch (cause) {
      // The stop action owns the cancellation confirmation; native failures still surface.
      if(cause instanceof NoUsableMaterial) notifications.info(cause.message, {key:"no-material"});
      else if (!(stopRequested.current && errorText(cause).startsWith("Anulowano"))) notifications.error(errorText(cause), { key: "broadcast-error" });
    }
    finally { setRunning(false); setStopping(false); }
  }
  async function stop() {
    stopRequested.current = true;
    setStopping(true);
    try { if (scheduledActive) await scheduler.current!.stop(); else if (running) await runner.current!.stop(); else await services.player.stop(); notifications.info(scheduledActive ? "Zatrzymano harmonogram i przywrócono głośność." : "Zatrzymano audycję."); }
    catch (cause) { notifications.error(errorText(cause), { key: "broadcast-error" }); }
    finally { setStopping(false); }
  }
  async function testSound() {
    if (busy) return; setTesting(true);
    try { const result = await services.player.play({ resourceId: "test.wav" }); if (result.phase === "ended") notifications.success("Test dźwięku zakończony."); }
    catch (cause) { notifications.error(errorText(cause), { key: "broadcast-error" }); }
    finally { setTesting(false); }
  }
  return { settings, sessions, playback, stage, ready, lastPlayed, news, loading, saving, running, stopping, testing, nativeBusy, busy, source, schedule, scheduledActive,
    save, refreshSessions, reconnect, startSchedule, start, stop, testSound };
}
export type Workspace = ReturnType<typeof useAraBrief>;
