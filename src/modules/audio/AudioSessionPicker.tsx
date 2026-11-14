import { useEffect, useRef, useState } from "react";
import type { AudioController, AudioSession } from "./AudioController";
import type { AudioPlayer } from "./AudioPlayer";
import { PlaybackTest } from "./PlaybackTest";
import { SettingsSummary } from "../settings/SettingsSummary";
import type { SettingsRepository } from "../settings/SettingsRepository";

export function AudioSessionPicker({ reader, player, repository, credentials }: { reader: Pick<AudioController, "listSessions" | "duck" | "restore">; player: AudioPlayer; repository: SettingsRepository; credentials: { save(): Promise<boolean>; check(): Promise<boolean>; delete(): Promise<void> } }) {
  const [sessions, setSessions] = useState<AudioSession[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const reload = useRef<(() => void) | null>(null);
  const [busy, setBusy] = useState(false);
  const [token, setToken] = useState("");
  const [notice, setNotice] = useState("");
  const [playbackBusy, setPlaybackBusy] = useState(false);
  const initializedSelection = useRef(false);
  async function changeVolume(restore: boolean) {
    setBusy(true);
    setError("");
    setNotice(restore ? "Przywracanie głośności…" : "Ściszanie muzyki…");
    try {
      const result = restore ? await reader.restore(token) : await reader.duck(selectedId);
      const outcome = typeof result === "string" ? result : result.outcome;
      setToken(restore ? "" : typeof result === "string" ? "" : result.restorationToken);
      setNotice(outcome === "manual_override" ? "Wykryto zmianę użytkownika — pozostawiono jego głośność." : restore ? "Przywrócono głośność." : "Muzyka ściszona zgodnie z ustawieniami.");
      reload.current?.();
    } catch (cause) {
      setNotice("");
      setError(typeof cause === "object" && cause !== null && "message" in cause ? String(cause.message) : String(cause));
    } finally { setBusy(false); }
  }

  useEffect(() => {
    let cancelled = false;
    async function refresh() {
      setLoading(true);
      setError("");
      try {
        const next = await reader.listSessions();
        const preferences = !initializedSelection.current ? await repository.load() : null;
        if (cancelled) return;
        setSessions(next);
        setSelectedId(id => {
          const preferred = id || preferences?.preferredAudioSource?.id || "";
          return next.some(session => session.id === preferred) ? preferred : "";
        });
        initializedSelection.current = true;
      } catch (cause) {
        if (cancelled) return;
        setSessions([]);
        setSelectedId("");
        setError(cause instanceof Error ? cause.message :
          typeof cause === "object" && cause !== null && "message" in cause
            ? String(cause.message) : "Nie udało się pobrać sesji audio.");
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    reload.current = () => { void refresh(); };
    void refresh();
    return () => { cancelled = true; reload.current = null; };
  }, [reader, repository]);

  return (
    <section aria-labelledby="audio-heading" aria-busy={loading} className="mt-6 rounded-2xl border border-slate-700 bg-slate-900 p-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <h2 id="audio-heading" className="text-lg font-medium">Źródło muzyki</h2>
        <button type="button" disabled={loading || busy} onClick={() => reload.current?.()}
          className="rounded-lg border border-slate-600 px-4 py-2 hover:bg-slate-800 focus-visible:outline-2 focus-visible:outline-teal-400 disabled:opacity-50">
          {loading ? "Odświeżanie…" : "Odśwież listę"}
        </button>
      </div>
      {error && <p role="alert" className="mt-4 text-red-300">{error}</p>}
      {!loading && !error && sessions.length === 0 && <p role="status" className="mt-4">Brak sesji. Włącz muzykę i odśwież listę.</p>}
      <fieldset disabled={loading || busy || playbackBusy || !!token} className="mt-4 space-y-3">
        <legend className="sr-only">Wybierz jedną sesję audio</legend>
        {sessions.map(session => (
          <label key={session.id} className="flex cursor-pointer gap-3 rounded-lg border border-slate-700 p-3">
            <input type="radio" name="audio-session" checked={selectedId === session.id} onChange={() => setSelectedId(session.id)} className="accent-teal-400" />
            <span className="min-w-0">
              <span className="block font-medium">{session.displayName}</span>
              <span className="block text-sm text-slate-400">{session.processName ?? "Nazwa niedostępna"} · PID {session.pid} · {Math.round(session.volume * 100)}% · {session.state === "active" ? "Aktywna" : "Nieaktywna"}</span>
              <span className="block break-all text-xs text-slate-500">Urządzenie: {session.deviceId}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <div className="mt-5 flex flex-wrap gap-3">
        <button type="button" disabled={busy || playbackBusy || loading || !selectedId || !!token} onClick={() => void changeVolume(false)} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">Ścisz muzykę</button>
        <button type="button" disabled={busy || playbackBusy} onClick={() => void changeVolume(true)} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Przywróć głośność</button>
      </div>
      <p role="status" className="mt-3 text-sm text-teal-300">{notice}</p>
      <p className="mt-4 text-sm text-slate-400">Lista jest migawką. Ręczna zmiana poziomu w mikserze Windows ma pierwszeństwo przed przywróceniem.</p>
      <PlaybackTest player={player} sessionId={selectedId} deviceId={sessions.find(session => session.id === selectedId)?.deviceId} disabled={busy || !!token} onBusy={setPlaybackBusy} />
      <SettingsSummary repository={repository} credentials={credentials} player={player} selected={sessions.find(session => session.id === selectedId) ? { id: selectedId, name: sessions.find(session => session.id === selectedId)!.displayName } : null} disabled={busy || playbackBusy} />
    </section>
  );
}
