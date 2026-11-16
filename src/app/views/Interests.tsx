import { useState } from "react";
import type { Workspace } from "../useAraBrief";
import { Icon } from "../components/Icon";

export function Interests({ app }: { app: Workspace }) {
  const [topic, setTopic] = useState("");
  const [error, setError] = useState("");
  async function add(event: React.FormEvent) {
    event.preventDefault(); if (!app.settings) return;
    const value = topic.trim(); setError("");
    if (!value || new TextEncoder().encode(value).length > 160 || /[\r\n\t]/.test(value)) { setError("Wpisz temat o długości do 160 bajtów, bez znaków sterujących."); return; }
    if (app.settings.topics.some(existing => existing.toLocaleLowerCase("pl") === value.toLocaleLowerCase("pl"))) { setError("Ten temat jest już na Twojej liście."); return; }
    if (app.settings.topics.length >= 30) { setError("Możesz zapisać maksymalnie 30 tematów."); return; }
    try { await app.save({ ...app.settings, topics: [...app.settings.topics, value] }); setTopic(""); } catch { /* Global message shows the persistence error. */ }
  }
  async function remove(value: string) {
    if (!app.settings) return;
    try { await app.save({ ...app.settings, topics: app.settings.topics.filter(t => t !== value) }); } catch { /* Reported by workspace. */ }
  }
  return <div className="interests-layout">
    <section className="topic-editor"><span className="eyebrow">TWOJA CIEKAWOŚĆ NADAJE TON</span><h2>Co Cię<br/><span>porusza?</span></h2><p className="muted">Wielkie odkrycia czy małe lokalne historie? Ty wybierasz tematy. My szukamy wiadomości w polskiej edycji Google News.</p>
      <form className="topic-form" onSubmit={event => void add(event)}><label htmlFor="new-topic">Dodaj zainteresowanie</label><input id="new-topic" value={topic} onChange={e=>{setTopic(e.target.value);setError("");}} placeholder="Temat, o którym chcesz usłyszeć" aria-describedby={error?"topic-error":"topic-hint"} aria-invalid={!!error} disabled={!app.settings || app.saving || app.busy} autoComplete="off"/><button className="button button-primary" disabled={!app.settings || !topic.trim() || app.saving || app.busy}><Icon name="plus" size={18}/>{app.saving?"Zapisywanie…":"Dodaj temat"}</button></form>
      {error && <p id="topic-error" role="alert" className="warning-text small">{error}</p>}<p id="topic-hint" className="footnote">Do 30 tematów. Każda zmiana zapisuje się automatycznie.</p>
      <aside className="note"><Icon name="spark" size={20}/><p>Im bardziej konkretny temat, tym trafniejsze wiadomości. Warto dopisać dziedzinę lub miejsce, które Cię interesuje.</p></aside>
    </section>
    <section className="panel topics-library"><div className="section-heading"><h3>Twoja lista tematów</h3><span className="badge">{app.settings ? `${app.settings.topics.length} / 30` : "Nie wczytano"}</span></div>
      {app.settings?.topics.length ? <ul className="interest-list">{app.settings.topics.map((value,index)=><li key={value}><span className="topic-number">{String(index+1).padStart(2,"0")}</span><div><strong>{value}</strong><span className="small muted">Google News · Polska</span></div><button className="icon-button" disabled={app.saving || app.busy} aria-label={`Usuń temat: ${value}`} onClick={()=>void remove(value)}><Icon name="close" size={17}/></button></li>)}</ul> : <div className="empty-state spacious"><span className="empty-icon"><Icon name="topics" size={30}/></span><h3>{app.loading?"Wczytywanie tematów…":!app.settings?"Twoja lista czeka na połączenie":"Wszystko zaczyna się od ciekawości"}</h3><p>{!app.settings?"Zapisane zainteresowania zobaczysz po połączeniu z aplikacją desktopową.":"Dodaj pierwszy temat w formularzu. Z niego zaczniemy układać Twój przegląd świata."}</p></div>}
      <p className="library-footnote"><Icon name="check" size={14}/>Twoje tematy są zapisywane lokalnie.</p>
    </section>
    <p className="interests-disclaimer footnote">Google News dostarcza artykuły do kolejki. Rozdziały powstają po otwarciu stron wydawców; niedostępne materiały są pomijane.</p>
  </div>;
}
