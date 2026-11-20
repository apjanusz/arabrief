import { useCopy } from "../../app/i18n";
import { useEffect, useRef, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
import type { SpeechAsset } from "../../domain/models";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { SpeechProvider } from "./SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";

const voices = ["alloy", "ash", "ballad", "coral", "echo", "sage", "shimmer", "verse", "marin", "cedar"];
export function SpeechPreview({ broadcastId, provider, player, settings }: { broadcastId: string; provider: SpeechProvider; player: AudioPlayer; settings: SettingsRepository }) {
  const copy = useCopy();
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
    setBusy(true);
    try {
      const current = await settings.load(); await settings.save({ ...current, openAiVoice: voice });
      if (cancelled.current) { notifications.info(copy('ui_6095cd15e5')); return; }
      const result = await provider.synthesize(broadcastId, voice, id); setAsset(result);
      notifications.success(result.cached ? copy("ui_0f4e974ff0") : copy('ui_3d802b4a4c'));
    } catch (cause) { notifications.error(String(cause)); }
    finally { job.current = null; setBusy(false); }
  }
  async function play() {
    if (!asset) return;
    setPlaying(true);
    try { const result = await player.play({ resourceId: asset.resourceId }); if(result.phase === "stopped") notifications.info(copy('ui_fdb529db04')); else notifications.success(copy("ui_1ddff69fb7")); }
    catch (cause) { notifications.error(String(cause)); }
    finally { setPlaying(false); }
  }
  return <section aria-labelledby="speech-heading" className="mt-5 border-t border-slate-700 pt-4">
    <h5 id="speech-heading" className="font-medium"> {copy("ui_644b0e2811")} </h5>
    <p className="mt-2 text-xs text-slate-400"> {copy("ui_c77425d535")} </p>
    <label className="mt-3 block text-sm"> {copy("ui_d478e46b24")} <select value={voice} disabled={busy || playing} onChange={e => { setVoice(e.target.value); setAsset(null); }} className="ml-3 rounded-lg border border-slate-600 bg-slate-950 p-2">{voices.map(v => <option key={v} value={v}>{v}</option>)}</select></label>
    <div className="mt-3 flex flex-wrap gap-3">
      <button disabled={busy || playing} onClick={() => void generate()} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? copy('ui_246786b0ca') : copy('ui_afc0b931b7')}</button>
      <button disabled={!busy} onClick={() => { cancelled.current = true; if (job.current) void provider.cancel(job.current).then(() => notifications.info(copy("ui_00b355a358"))).catch(cause => notifications.error(String(cause))); }} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50"> {copy("ui_bbac7ce1a4")} </button>
      <button disabled={!asset || busy || playing} onClick={() => void play()} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50"> {copy("ui_882569b055")} </button>
      <button disabled={!playing} onClick={() => void player.stop().catch(cause => notifications.error(String(cause)))} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50"> {copy("ui_92380c8468")} </button>
    </div>
    {asset && <><p className="mt-2 break-all text-xs text-slate-400"> {copy("ui_3f8fdd5ca0")} {asset.resourceId} · {(asset.durationMs / 1000).toFixed(1)}  {copy("ui_09e08911de")} {asset.segmentCount}</p><details className="mt-2 text-sm"><summary className="cursor-pointer"> {copy("ui_1b9a3399c6")} </summary><p className="mt-2 whitespace-pre-wrap">{asset.transcript}</p></details></>}
    <p className="mt-3 text-xs text-slate-400"> {copy("ui_95a6f57b99")} </p>
  </section>;
}
