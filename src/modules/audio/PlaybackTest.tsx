import { useEffect, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
import type { AudioPlayer, PlaybackStatus } from "./AudioPlayer";

export function PlaybackTest({ player, sessionId, deviceId, disabled, onBusy }: {
  player: AudioPlayer; sessionId: string; deviceId?: string; disabled: boolean; onBusy: (busy: boolean) => void;
}) {
  const [status, setStatus] = useState<PlaybackStatus | null>(null);
  const [volume, setVolume] = useState(0.7);
  const [pending, setPending] = useState(false);
  const [sameDevice, setSameDevice] = useState(false);
  const busy = pending || !!status && ["preparing", "ducking", "playing", "restoring"].includes(status.phase);
  useEffect(() => { if(status?.error) notifications.error(status.error, { key: "diagnostic-player" }); }, [status?.error]);
  useEffect(() => { onBusy(busy); }, [busy, onBusy]);
  useEffect(() => {
    let mounted = true;
    let fetching = false;
    async function refresh() {
      if (fetching) return;
      fetching = true;
      try { const next = await player.getStatus(); if (mounted) { setStatus(next); setVolume(next.volume); } }
      catch { /* Desktop availability is also checked when play is requested. */ }
      finally { fetching = false; }
    }
    void refresh();
    const timer = setInterval(() => void refresh(), 250);
    return () => { mounted = false; clearInterval(timer); };
  }, [player]);

  async function play(withMusic: boolean) {
    setPending(true);
    try { setStatus(await player.play({ resourceId: "test.wav", musicSessionId: withMusic ? sessionId : undefined, outputDeviceId: sameDevice ? deviceId : undefined })); }
    catch (cause) { notifications.error(String(cause), { key: "diagnostic-player" }); }
    finally { setPending(false); }
  }
  return (
    <section aria-labelledby="playback-heading" className="mt-6 border-t border-slate-700 pt-5">
      <h3 id="playback-heading" className="font-medium">Test nagrania</h3>
      <p className="mt-2 text-sm text-slate-400">Dwusekundowy plik WAV z dwoma tonami. Test audycji ściszy wybraną muzykę i przywróci ją po końcu nagrania.</p>
      <label className="mt-4 block text-sm">Głośność lektora: {Math.round(volume * 100)}%
        <input aria-label="Głośność lektora" type="range" min="0" max="1" step="0.01" value={volume}
          onChange={event => { const value = Number(event.target.value); setVolume(value); void player.setVolume(value).catch(cause => notifications.error(String(cause), { key: "diagnostic-volume" })); }} className="mt-2 block w-full accent-teal-400" />
      </label>
      <label className="mt-3 flex gap-2 text-sm"><input type="checkbox" checked={sameDevice} disabled={busy || !deviceId} onChange={e => setSameDevice(e.target.checked)} />Wyjście wybranej sesji muzycznej (domyślnie: domyślne wyjście Windows)</label>
      <div className="mt-4 flex flex-wrap gap-3">
        <button disabled={busy || disabled} onClick={() => void play(false)} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Odtwórz nagranie</button>
        <button disabled={busy || disabled || !sessionId} onClick={() => void play(true)} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">Test audycji z muzyką</button>
        <button disabled={!busy} onClick={() => void player.stop().catch(cause => notifications.error(String(cause), { key: "diagnostic-player" }))} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Zatrzymaj nagranie</button>
      </div>
      <p role="status" className="mt-3 text-sm text-teal-300">{status ? ({ idle: "Gotowy", preparing: "Przygotowanie", ducking: "Ściszanie muzyki", playing: "Odtwarzanie", paused: "Wstrzymano", restoring: "Przywracanie muzyki", ended: "Nagranie zakończone", stopped: "Nagranie zatrzymane", error: "Błąd odtwarzania" })[status.phase] : "Uruchom aplikację desktopową, aby przetestować dźwięk."}</p>
    </section>
  );
}
