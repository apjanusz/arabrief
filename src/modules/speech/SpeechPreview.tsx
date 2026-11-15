import { useEffect, useRef, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
import type { SpeechAsset } from "../../domain/models";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { SpeechProvider } from "./SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";

const voices = ["alloy", "ash", "ballad", "coral", "echo", "sage", "shimmer", "verse", "marin", "cedar"];
export function SpeechPreview({ broadcastId, provider, player, settings }: { broadcastId: string; provider: SpeechProvider; player: AudioPlayer; settings: SettingsRepository }) {
  const [voice, setVoice] = useState("alloy");
  const [asset, setAsset] = useState<SpeechAsset | null>(null);
  const [busy, setBusy] = useState(false);
  const [playing, setPlaying] = useState(false);
  const job = useRef<string | null>(null);
  const cancelled = useRef(false);
  useEffect(() => {
    let mounted = true;
    void settings.load().then(value => { if (mounted && voices.includes(value.openAiVoice)) setVoice(value.openAiVoice); }).catch(() => {});
    return () => { mounted = false; cancelled.current = true; if (job.current) void provider.cancel(job.current).catch(() => {}); };
  }, [settings, provider]);
  async function generate() {
    if (job.current) return;
    cancelled.current = false;
    const id = crypto.randomUUID(); job.current = id;
    setBusy(true); setAsset(null);
    try {
      const current = await settings.load(); await settings.save({ ...current, openAiVoice: voice });
      if (cancelled.current) { notifications.info("Anulowano generowanie audio."); return; }
      const result = await provider.synthesize(broadcastId, voice, id); setAsset(result);
      notifications.success(result.cached ? "Odczytano istniejące nagranie z cache, bez wywołania API." : "Zapisano WAV. Tekst transkrypcji odpowiada scenariuszowi po normalizacji.");
    } catch (cause) { notifications.error(String(cause)); }
    finally { job.current = null; setBusy(false); }
  }
  async function play() {
    if (!asset) return;
    setPlaying(true);
    try { const result = await player.play({ resourceId: asset.resourceId }); if(result.phase === "stopped") notifications.info("Zatrzymano nagranie."); else notifications.success("Zakończono odtwarzanie."); }
    catch (cause) { notifications.error(String(cause)); }
    finally { setPlaying(false); }
  }
  return <section aria-labelledby="speech-heading" className="mt-5 border-t border-slate-700 pt-4">
    <h5 id="speech-heading" className="font-medium">Audio audycji</h5>
    <p className="mt-2 text-xs text-slate-400">Głos wygenerowany przez AI · gpt-realtime-2.1-mini. Bez mikrofonu. Nowe nagranie korzysta z płatnego API. W razie błędu lub niezgodności transkrypcji nie będzie automatycznej ponownej próby.</p>
    <label className="mt-3 block text-sm">Głos <select value={voice} disabled={busy || playing} onChange={e => { setVoice(e.target.value); setAsset(null); }} className="ml-3 rounded-lg border border-slate-600 bg-slate-950 p-2">{voices.map(v => <option key={v} value={v}>{v}</option>)}</select></label>
    <div className="mt-3 flex flex-wrap gap-3">
      <button disabled={busy || playing} onClick={() => void generate()} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? "Generowanie audio…" : "Wygeneruj audio"}</button>
      <button disabled={!busy} onClick={() => { cancelled.current = true; if (job.current) void provider.cancel(job.current).then(() => notifications.info("Zgłoszono anulowanie generowania…")).catch(cause => notifications.error(String(cause))); }} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Anuluj generowanie</button>
      <button disabled={!asset || busy || playing} onClick={() => void play()} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Odtwórz audio</button>
      <button disabled={!playing} onClick={() => void player.stop().catch(cause => notifications.error(String(cause)))} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Zatrzymaj nagranie</button>
    </div>
    {asset && <><p className="mt-2 break-all text-xs text-slate-400">Plik: {asset.resourceId} · {(asset.durationMs / 1000).toFixed(1)} s · segmenty: {asset.segmentCount}</p><details className="mt-2 text-sm"><summary className="cursor-pointer">Transkrypcja wyjścia Realtime</summary><p className="mt-2 whitespace-pre-wrap">{asset.transcript}</p></details></>}
    <p className="mt-3 text-xs text-slate-400">Transkrypcja pochodzi z odpowiedzi modelu, a nie z niezależnego rozpoznawania WAV. Sprawdź nagranie przed wykorzystaniem w audycji.</p>
  </section>;
}
