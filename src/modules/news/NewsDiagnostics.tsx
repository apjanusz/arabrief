import { useEffect, useState } from "react";
import type { NewsProvider, NewsResult } from "./NewsProvider";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { ContentGenerator } from "../content/ContentGenerator";
import { ContentPreview } from "../content/ContentPreview";
import type { SpeechProvider } from "../speech/SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";

export function NewsDiagnostics({ provider, settings, generator, speech, player }: { provider: NewsProvider; settings: SettingsRepository; generator: ContentGenerator; speech: SpeechProvider; player: AudioPlayer }) {
  const [topics, setTopics] = useState("");
  const [result, setResult] = useState<NewsResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => {
    let mounted = true;
    void settings.load().then(value => { if (mounted) setTopics(value.topics.join("\n")); }).catch(() => {});
    return () => { mounted = false; };
  }, [settings]);
  const requested = () => topics.split("\n").map(t => t.trim()).filter(Boolean);
  async function fetch() {
    setBusy(true); setMessage(""); setResult(null);
    try { setResult(await provider.fetch({ topics: requested() })); }
    catch (cause) { setMessage(String(cause)); }
    finally { setBusy(false); }
  }
  async function save() {
    setBusy(true);
    try { const current = await settings.load(); await settings.save({ ...current, topics: requested() }); setMessage("Zapisano tematy."); }
    catch (cause) { setMessage(String(cause)); }
    finally { setBusy(false); }
  }
  return (
    <section aria-labelledby="news-heading" className="mt-6 rounded-2xl border border-slate-700 bg-slate-900 p-6">
      <h2 id="news-heading" className="text-lg font-medium">Wiadomości — diagnostyka RSS</h2>
      <p className="mt-2 text-sm text-slate-400">Google News · język polski · Polska. Pobieramy metadane i dostępny opis RSS, bez pełnej treści artykułów.</p>
      <label className="mt-4 block text-sm" htmlFor="news-topics">Tematy — jeden w każdym wierszu (maksymalnie 30)</label>
      <textarea id="news-topics" rows={3} value={topics} disabled={busy} onChange={e => setTopics(e.target.value)} placeholder={"Nauka\nGospodarka"} className="mt-2 w-full rounded-lg border border-slate-600 bg-slate-950 p-3" />
      <div className="mt-3 flex flex-wrap gap-3">
        <button disabled={busy || !topics.trim()} onClick={() => void fetch()} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? "Przetwarzanie…" : "Pobierz wiadomości"}</button>
        <button disabled={busy} onClick={() => void save()} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50">Zapisz tematy</button>
      </div>
      <p role="status" className="mt-3 text-sm text-teal-300">{message}</p>
      {result && <>
        <p role="status" className="mt-3 text-sm">Artykuły dostępne do nowego scenariusza: {result.items.length}. Odpytano tematów: {result.fetchedTopics.length}. Pozostałe wyniki mogą pochodzić z pamięci SQLite.</p>
        <ul className="mt-3 space-y-1 text-xs text-slate-400">{result.topics.map(topic => <li key={topic.topic}>
          {topic.topic} — następne pobranie nie wcześniej niż {new Date(topic.nextFetchAt * 1000).toLocaleString("pl-PL")}
          {topic.error && <span className="text-red-300"> · {topic.error} (wyniki mogą być nieaktualne)</span>}
        </li>)}</ul>
        <ol className="mt-5 space-y-4">{result.items.map(item => <li key={item.id} className="rounded-lg border border-slate-700 p-4">
          <h3 className="font-medium">{item.title}</h3>
          <p className="mt-2 text-sm text-slate-400">{item.source ?? "Brak źródła w RSS"} · {item.publishedAt ? new Date(item.publishedAt).toLocaleString("pl-PL") : "Brak poprawnej daty publikacji"}</p>
          <p className="mt-1 text-xs text-slate-400">Tematy: {item.topics.join(", ")} · Pobrano: {new Date(item.fetchedAt).toLocaleString("pl-PL")}</p>
          <p className="mt-2 break-all text-xs text-teal-300">{item.url}</p>
          <details className="mt-3 text-sm"><summary className="cursor-pointer">Opis i metadane źródłowe RSS</summary>
            <p className="mt-2 text-xs text-slate-400">Opis jest wyświetlany jako tekst. Może zawierać tylko powtórzony nagłówek lub odnośniki; nie jest pełnym artykułem.</p>
            <pre className="mt-2 whitespace-pre-wrap break-words text-xs">{item.descriptionHtml ?? "Brak opisu w RSS."}</pre>
            <p className="mt-2 break-all text-xs">Źródłowy URL: {item.sourceUrl ?? "Brak"}<br />GUID: {item.guid ?? "Brak"}<br />Surowa data: {item.rawPublicationDate ?? "Brak"}<br />Kanał: {item.feedUrl}</p>
          </details>
        </li>)}</ol>
        {result.items.length === 0 && <p className="mt-4 text-sm text-slate-400">Brak niewykorzystanych wyników. Sprawdź statusy tematów; pusty wynik nie oznacza, że pełne artykuły zostały przeanalizowane.</p>}
      </>}
      <p className="mt-4 text-xs text-slate-400">Podgląd nie zapisuje użycia w audycji. Limit: 5 minut na temat i co najmniej minuta pomiędzy partiami nowych zapytań.</p>
      <ContentPreview generator={generator} settings={settings} items={result?.items ?? []} speech={speech} player={player} />
    </section>
  );
}
