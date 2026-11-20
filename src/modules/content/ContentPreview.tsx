import { useCopy } from "../../app/i18n";
import { useEffect, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
import type { BroadcastScript, NewsItem } from "../../domain/models";
import type { ContentGenerator } from "./ContentGenerator";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { SpeechProvider } from "../speech/SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";
import { SpeechPreview } from "../speech/SpeechPreview";
import { NoUsableMaterial } from "../scheduler/PreparationRecovery";

export function ContentPreview({ generator, settings, items, speech, player }: { generator: ContentGenerator; settings: SettingsRepository; items: NewsItem[]; speech: SpeechProvider; player: AudioPlayer }) {
  const copy = useCopy();
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [broadcast, setBroadcast] = useState<BroadcastScript | null>(null);
  const [excluded, setExcluded] = useState<string[]>([]);
  useEffect(() => {
    let mounted = true;
    void settings.load().then(value => { if (mounted) setModel(value.textModel); }).catch(() => {});
    return () => { mounted = false; };
  }, [settings]);
  const candidates = items.filter(item => !excluded.includes(item.id)).slice(0,20);
  async function generate() {
    setBusy(true);
    try {
      const preferences = await settings.load();
      await settings.save({ ...preferences, textModel: model.trim() });
      const result = await generator.generate(candidates.map(item => item.id));
      setBroadcast(result);
      setExcluded(previous => [...new Set([...previous,...result.sources.map(source => source.id)])]);
      notifications.success(copy("ui_b178e99389"));
    } catch (cause) { if(cause instanceof NoUsableMaterial)notifications.info(cause.message);else notifications.error(String(cause)); }
    finally { setBusy(false); }
  }
  return (
    <section aria-labelledby="content-heading" className="mt-6 border-t border-slate-700 pt-5">
      <h3 id="content-heading" className="font-medium"> {copy("ui_a2917f632f")} </h3>
      <p className="mt-2 text-sm text-slate-400"> {copy("ui_602eaa1038")} </p>
      <label htmlFor="text-model" className="mt-3 block text-sm"> {copy("ui_ba3f7a6017")} </label>
      <input id="text-model" value={model} disabled={busy} onChange={e => setModel(e.target.value)} className="mt-2 w-full rounded-lg border border-slate-600 bg-slate-950 p-2" />
      <p className="mt-2 text-xs text-slate-400"> {copy("ui_c9f1bee525")} {candidates.length} {copy("ui_1202903bef")} </p>
      <button disabled={busy || candidates.length<1 || !model.trim()} onClick={() => void generate()} className="mt-3 rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? copy('ui_4173d2b514') : copy("ui_47726e1cfd")}</button>
      {broadcast && <article className="mt-5 rounded-lg border border-slate-700 p-4">
        <h4 className="text-lg font-medium">{broadcast.title}</h4>
        <p className="mt-2 text-xs text-slate-400">{new Date(broadcast.createdAt).toLocaleString(copy("locale"))} · {broadcast.model}  {copy("ui_265d9f4e49")} {Math.round(broadcast.estimatedDurationMs/1000)}  {copy("ui_aef7ce5393")} </p>
        <p className="mt-4 whitespace-pre-wrap leading-relaxed">{broadcast.text}</p>
        {broadcast.draft.limitations.length>0 && <ul className="mt-4 list-disc pl-5 text-sm text-amber-200">{broadcast.draft.limitations.map((limitation,index) => <li key={index}>{limitation}</li>)}</ul>}
        <h5 className="mt-4 font-medium"> {copy("ui_849c26d9a0")} </h5>
        <ul className="mt-2 space-y-2 text-sm">{broadcast.sources.map(source => <li key={source.id}>
          {source.title} · {source.source ?? copy("ui_8ee6d4d493")}<p className="break-all text-xs text-teal-300">{source.url}</p>
        </li>)}</ul>
        <details className="mt-4 text-sm"><summary className="cursor-pointer"> {copy("ui_f2b75e5cf0")} </summary>
          {broadcast.draft.segments.map((segment,index) => <div key={index} className="mt-3"><strong>{segment.topic}</strong><ul>{segment.evidence.map((evidence,i) => <li key={i} className="mt-1"> {copy("ui_94e2e16239")} {evidence.source_id}: „{evidence.quote}”</li>)}</ul></div>)}
        </details>
        <p className="mt-3 text-xs text-slate-400"> {copy("ui_b63e623961")} </p>
        <SpeechPreview key={broadcast.id} broadcastId={broadcast.id} provider={speech} player={player} settings={settings} />
      </article>}
    </section>
  );
}
