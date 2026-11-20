import { useCopy } from "../../app/i18n";
import { useEffect, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
import type { NewsProvider, NewsResult } from "./NewsProvider";
import type { SettingsRepository } from "../settings/SettingsRepository";
import type { ContentGenerator } from "../content/ContentGenerator";
import { ContentPreview } from "../content/ContentPreview";
import type { SpeechProvider } from "../speech/SpeechProvider";
import type { AudioPlayer } from "../audio/AudioPlayer";

export function NewsDiagnostics({ provider, settings, generator, speech, player }: { provider: NewsProvider; settings: SettingsRepository; generator: ContentGenerator; speech: SpeechProvider; player: AudioPlayer }) {
  const copy = useCopy();
  const [topics, setTopics] = useState("");
  const [result, setResult] = useState<NewsResult | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let mounted = true;
    void settings.load().then(value => { if (mounted) setTopics(value.topics.join("\n")); }).catch(() => {});
    return () => { mounted = false; };
  }, [settings]);
  const requested = () => topics.split("\n").map(t => t.trim()).filter(Boolean);
  async function fetch() {
    setBusy(true); setResult(null);
    try { setResult(await provider.fetch({ topics: requested() })); }
    catch (cause) { notifications.error(String(cause)); }
    finally { setBusy(false); }
  }
  async function save() {
    setBusy(true);
    try { const current = await settings.load(); await settings.save({ ...current, topics: requested() }); notifications.success(copy('ui_8cb766f44d')); }
    catch (cause) { notifications.error(String(cause)); }
    finally { setBusy(false); }
  }
  return (
    <section aria-labelledby="news-heading" className="mt-6 rounded-2xl border border-slate-700 bg-slate-900 p-6">
      <h2 id="news-heading" className="text-lg font-medium"> {copy("ui_53656b67ea")} </h2>
      <p className="mt-2 text-sm text-slate-400"> {copy("ui_4a38ed0e9b")} </p>
      <label className="mt-4 block text-sm" htmlFor="news-topics"> {copy("ui_6d58a44373")} </label>
      <textarea id="news-topics" rows={3} value={topics} disabled={busy} onChange={e => setTopics(e.target.value)} placeholder={copy("topicExamples")} className="mt-2 w-full rounded-lg border border-slate-600 bg-slate-950 p-3" />
      <div className="mt-3 flex flex-wrap gap-3">
        <button disabled={busy || !topics.trim()} onClick={() => void fetch()} className="rounded-lg bg-teal-700 px-4 py-2 disabled:opacity-50">{busy ? copy('ui_66a3e59db0') : copy("ui_553dd7d34b")}</button>
        <button disabled={busy} onClick={() => void save()} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50"> {copy("ui_f2e4bdba25")} </button>
      </div>
      {result && <>
        <p role="status" className="mt-3 text-sm"> {copy("ui_4683ea9e7a")} {result.items.length} {copy("ui_1c0c7026b5")} {result.fetchedTopics.length} {copy("ui_f61b53c1c1")} </p>
        <ul className="mt-3 space-y-1 text-xs text-slate-400">{result.topics.map(topic => <li key={topic.topic}>
          {topic.topic}  {copy("ui_7f94264e03")} {new Date(topic.nextFetchAt * 1000).toLocaleString(copy("locale"))}
          {topic.error && <span className="text-red-300"> · {copy(topic.error)}  {copy("ui_b0a70650c6")} </span>}
        </li>)}</ul>
        <ol className="mt-5 space-y-4">{result.items.map(item => <li key={item.id} className="rounded-lg border border-slate-700 p-4">
          <h3 className="font-medium">{item.title}</h3>
          <p className="mt-2 text-sm text-slate-400">{item.source ?? copy("ui_a2187f85f3")} · {item.publishedAt ? new Date(item.publishedAt).toLocaleString(copy("locale")) : copy('ui_a1e7ff0d60')}</p>
          <p className="mt-1 text-xs text-slate-400"> {copy("ui_72bf7a7c6a")} {item.topics.join(", ")}  {copy("ui_ba8172dc04")} {new Date(item.fetchedAt).toLocaleString(copy("locale"))}</p>
          <p className="mt-2 break-all text-xs text-teal-300">{item.url}</p>
          <details className="mt-3 text-sm"><summary className="cursor-pointer"> {copy("ui_e919fb1cb8")} </summary>
            <p className="mt-2 text-xs text-slate-400"> {copy("ui_8c479bcfde")} </p>
            <pre className="mt-2 whitespace-pre-wrap break-words text-xs">{item.descriptionHtml ?? copy('ui_26dfee75a2')}</pre>
            <p className="mt-2 break-all text-xs"> {copy("ui_dc36ad02dc")} {item.sourceUrl ?? copy('ui_b30f40d8db')}<br />GUID: {item.guid ?? copy('ui_b30f40d8db')}<br /> {copy("ui_c70717b3b0")} {item.rawPublicationDate ?? copy('ui_b30f40d8db')}<br /> {copy("ui_3bbae77158")} {item.feedUrl}</p>
          </details>
        </li>)}</ol>
        {result.items.length === 0 && <p className="mt-4 text-sm text-slate-400"> {copy("ui_eb35320a0e")} </p>}
      </>}
      <p className="mt-4 text-xs text-slate-400"> {copy("ui_9f623dcaef")} </p>
      <ContentPreview generator={generator} settings={settings} items={result?.items ?? []} speech={speech} player={player} />
    </section>
  );
}
