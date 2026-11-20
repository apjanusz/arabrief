import { useCopy } from "../i18n";
import { useState } from "react";
import type { Workspace } from "../useAraBrief";
import { Icon } from "../components/Icon";

export function Interests({ app }: { app: Workspace }) {
  const copy = useCopy();
  const [topic, setTopic] = useState("");
  const [error, setError] = useState("");
  async function add(event: React.FormEvent) {
    event.preventDefault(); if (!app.settings) return;
    const value = topic.trim(); setError("");
    if (!value || new TextEncoder().encode(value).length > 160 || /[\r\n\t]/.test(value)) { setError(copy("ui_0a3930ffbd")); return; }
    if (app.settings.topics.some(existing => existing.toLocaleLowerCase("pl") === value.toLocaleLowerCase("pl"))) { setError(copy("ui_a793a726f4")); return; }
    if (app.settings.topics.length >= 30) { setError(copy("ui_990d037480")); return; }
    try { await app.save({ ...app.settings, topics: [...app.settings.topics, value] }); setTopic(""); } catch { /* Global message shows the persistence error. */ }
  }
  async function remove(value: string) {
    if (!app.settings) return;
    try { await app.save({ ...app.settings, topics: app.settings.topics.filter(t => t !== value) }); } catch { /* Reported by workspace. */ }
  }
  return <div className="interests-layout">
    <section className="topic-editor"><span className="eyebrow"> {copy("ui_aa95cfe466")} </span><h2> {copy("ui_e73c107b9b")} <br/><span> {copy("ui_e8d0486947")} </span></h2><p className="muted"> {copy("ui_680d4b2096")} </p>
      <form className="topic-form" onSubmit={event => void add(event)}><label htmlFor="new-topic"> {copy("ui_faecb08125")} </label><input id="new-topic" value={topic} onChange={e=>{setTopic(e.target.value);setError("");}} placeholder={copy("ui_de53f3d9f6")} aria-describedby={error?"topic-error":"topic-hint"} aria-invalid={!!error} disabled={!app.settings || app.saving || app.busy} autoComplete="off"/><button className="button button-primary" disabled={!app.settings || !topic.trim() || app.saving || app.busy}><Icon name="plus" size={18}/>{app.saving?copy("ui_177003e983"):copy('ui_f9f8079ddf')}</button></form>
      {error && <p id="topic-error" role="alert" className="warning-text small">{copy(error)}</p>}<p id="topic-hint" className="footnote"> {copy("ui_bbe82db82c")} </p>
      <aside className="note"><Icon name="spark" size={20}/><p> {copy("ui_117ab5623a")} </p></aside>
    </section>
    <section className="panel topics-library"><div className="section-heading"><h3> {copy("ui_82ffeff9ab")} </h3><span className="badge">{app.settings ? `${app.settings.topics.length} / 30` : copy('ui_7a06b96a03')}</span></div>
      {app.settings?.topics.length ? <ul className="interest-list">{app.settings.topics.map((value,index)=><li key={value}><span className="topic-number">{String(index+1).padStart(2,"0")}</span><div><strong>{value}</strong><span className="small muted"> {copy("ui_16b2a87a64")} </span></div><button className="icon-button" disabled={app.saving || app.busy} aria-label={copy("ui_1fdfd79697").replace('{0}', String(value))} onClick={()=>void remove(value)}><Icon name="close" size={17}/></button></li>)}</ul> : <div className="empty-state spacious"><span className="empty-icon"><Icon name="topics" size={30}/></span><h3>{app.loading?copy("ui_5ea8b01653"):!app.settings?copy("ui_fad168175a"):copy("ui_bede1b1cf3")}</h3><p>{!app.settings?copy("ui_b3d737ecfb"):copy("ui_9e0c1a26dc")}</p></div>}
      <p className="library-footnote"><Icon name="check" size={14}/> {copy("ui_3092410560")} </p>
    </section>
    <p className="interests-disclaimer footnote"> {copy("ui_744423543c")} </p>
  </div>;
}
