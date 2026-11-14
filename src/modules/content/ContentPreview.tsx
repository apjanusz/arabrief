import { useEffect, useState } from "react";
import type { BroadcastScript, NewsItem } from "../../domain/models";
import type { ContentGenerator } from "./ContentGenerator";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { SpeechProvider } from "../speech/SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";
import { SpeechPreview } from "../speech/SpeechPreview";

export function ContentPreview({ generator, settings, items, speech, player }: { generator: ContentGenerator; settings: SettingsRepository; items: NewsItem[]; speech: SpeechProvider; player: AudioPlayer }) {
  const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [broadcast, setBroadcast] = useState<BroadcastScript | null>(null);
  const [excluded, setExcluded] = useState<string[]>([]);
  useEffect(() => {
    let mounted = true;
    void settings.load().then(value => { if (mounted) setModel(value.textModel); }).catch(() => {});
    return () => { mounted = false; };
  }, [settings]);
  const candidates = items.filter(item => !excluded.includes(item.id)).slice(0,8);
  async function generate() {
    setBusy(true); setError(""); setBroadcast(null);
    try {
      const preferences = await settings.load();
      await settings.save({ ...preferences, textModel: model.trim() });
      const result = await generator.generate(candidates.map(item => item.id));
      setBroadcast(result);
      setExcluded(previous => [...new Set([...previous,...result.sources.map(source => source.id)])]);
    } catch (cause) { setError(String(cause)); }
    finally { setBusy(false); }
  }
  return (
    <section aria-labelledby="content-heading" className="mt-6 border-t border-slate-700 pt-5">
      <h3 id="content-heading" className="font-medium">Scenariusz testowej audycji</h3>
      <p className="mt-2 text-sm text-slate-400">Do OpenAI zostanie wysłanych do 8 najnowszych wiadomości z powyższej listy oraz kontekst poprzednich scenariuszy. Generowanie korzysta z płatnego API. Klucz ustawisz w natywnym magazynie Windows.</p>
      <label htmlFor="text-model" className="mt-3 block text-sm">Model tekstowy (Responses API i Structured Outputs)</label>
      <input id="text-model" value={model} disabled={busy} onChange={e => setModel(e.target.value)} className="mt-2 w-full rounded-lg border border-slate-600 bg-slate-950 p-2" />
      <p className="mt-2 text-xs text-slate-400">Materiały: {candidates.length}. Scenariusz będzie po polsku, orientacyjnie 2–3 minuty. Przy samych nagłówkach może być krótszy.</p>
      <button disabled={busy || candidates.length<2 || !model.trim()} onClick={() => void generate()} className="mt-3 rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? "Generowanie scenariusza…" : "Wygeneruj testową audycję"}</button>
      {error && <p role="alert" className="mt-3 text-sm text-red-300">{error}</p>}
      {broadcast && <article className="mt-5 rounded-lg border border-slate-700 p-4">
        <h4 className="text-lg font-medium">{broadcast.title}</h4>
        <p className="mt-2 text-xs text-slate-400">{new Date(broadcast.createdAt).toLocaleString("pl-PL")} · {broadcast.model} · szacunkowo {Math.round(broadcast.estimatedDurationMs/1000)} s przy 140 słowach/min (bez TTS)</p>
        <p className="mt-4 whitespace-pre-wrap leading-relaxed">{broadcast.text}</p>
        {broadcast.draft.limitations.length>0 && <ul className="mt-4 list-disc pl-5 text-sm text-amber-200">{broadcast.draft.limitations.map((limitation,index) => <li key={index}>{limitation}</li>)}</ul>}
        <h5 className="mt-4 font-medium">Powiązane źródła</h5>
        <ul className="mt-2 space-y-2 text-sm">{broadcast.sources.map(source => <li key={source.id}>
          {source.title} · {source.source ?? "Brak nazwy źródła w RSS"}<p className="break-all text-xs text-teal-300">{source.url}</p>
        </li>)}</ul>
        <details className="mt-4 text-sm"><summary className="cursor-pointer">Cytaty źródłowe dla fragmentów scenariusza</summary>
          {broadcast.draft.segments.map((segment,index) => <div key={index} className="mt-3"><strong>{segment.topic}</strong><ul>{segment.evidence.map((evidence,i) => <li key={i} className="mt-1">Źródło {evidence.source_id}: „{evidence.quote}”</li>)}</ul></div>)}
        </details>
        <p className="mt-3 text-xs text-slate-400">Zapisano scenariusz i źródła w SQLite. Nie odtworzono audycji ani nie oznaczono jej jako wyemitowanej.</p>
        <SpeechPreview key={broadcast.id} broadcastId={broadcast.id} provider={speech} player={player} settings={settings} />
      </article>}
    </section>
  );
}
