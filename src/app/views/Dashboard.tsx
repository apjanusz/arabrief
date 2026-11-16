import type { Workspace } from "../useAraBrief";
import { Icon } from "../components/Icon";

const stages = ["news", "content", "speech", "playing"] as const;
const stageNames = { idle: "Czas na Twój przegląd świata", news: "Szukamy wiadomości dla Ciebie", content: "Układamy opowieść z wiadomości", speech: "Twój scenariusz nabiera głosu", playing: "Teraz na Twoich falach" };
function countdown(value: number | null) {
  if (value === null) return "—:—";
  const seconds = Math.ceil(value / 1000);
  return [Math.floor(seconds / 3600), Math.floor(seconds % 3600 / 60), seconds % 60].map(part => String(part).padStart(2, "0")).join(":");
}

export function Dashboard({ app, navigate }: { app: Workspace; navigate: (page: "topics" | "settings") => void }) {
  const configured = !!app.settings && !!app.source && !!app.settings.topics.length;
  const activeIndex = stages.findIndex(stage => stage === app.stage);
  const activeLabel = app.stopping ? "Zatrzymywanie audycji…" : app.pausePending ? "Zmieniamy stan pauzy…" : app.paused ? "AraBrief wstrzymany" : app.scheduledActive && app.schedule.phase === "restoring" ? "Przywracamy głośność muzyki" : app.scheduledActive && app.schedule.phase === "ducking" ? "Delikatnie ściszamy muzykę" : app.scheduledActive && app.schedule.phase === "playing" ? "Audycja jest odtwarzana" : app.testing ? "Sprawdzamy wyjście dźwięku" : stageNames[app.stage];
  const status = app.loading ? "Łączenie ze studiem" : !app.settings ? "Studio niepołączone" : app.paused || app.pausePending ? activeLabel : app.scheduledActive ? app.schedule.phase === "waiting" ? app.schedule.ready ? "Audycja gotowa. Czekamy na termin." : app.schedule.message ?? "Czekamy na nowe materiały." : app.schedule.phase === "error" ? "Przygotowanie nie powiodło się. Termin zostanie pominięty." : activeLabel : app.busy ? activeLabel : app.ready ? "Nagranie czeka na odsłuch" : "Gotowe na Twoje tematy";
  return <>
    <div className="studio-grid">
      <section className={`hero panel ${app.busy ? "hero-active" : ""}`}>
        <div className="hero-top"><span className="eyebrow">TWÓJ OSOBISTY PRZEGLĄD</span><span className="edition-label">ARA / BRIEF</span></div>
        <div className="hero-copy">
          <h2>Trochę świata.<br/><span>Między utworami.</span></h2>
          <p>Wiadomości, które Cię ciekawią.<br/>Spokojny głos. Twoja muzyka w tle.</p>
        </div>
        <div className="hero-controls">
          <button className={`button ${app.busy ? "button-danger" : "button-primary"}`} disabled={app.loading || app.saving || app.stopping || (!app.busy && !configured)} onClick={() => void (app.busy ? app.stop() : app.startSchedule())}>
            <Icon name={app.busy ? "stop" : "play"} size={20}/>{app.stopping ? "Zatrzymywanie…" : app.scheduledActive ? "Zatrzymaj harmonogram" : app.busy ? "Zatrzymaj audycję" : "Uruchom harmonogram"}
          </button>
          {app.canPause && <button className="button button-secondary" disabled={app.stopping || app.pausePending} aria-pressed={app.paused} onClick={() => void app.togglePause()}><Icon name={app.paused ? "play" : "pause"} size={20}/>{app.paused ? "Wznów" : "Pauza"}</button>}
          <span className="hero-hint">{app.paused ? "Wznów, gdy chcesz wrócić do audycji." : app.scheduledActive ? "Następna audycja powstaje w tle." : "Pierwsze przygotowanie użyje płatnego API OpenAI."}</span>
        </div>
        <div className="hero-status" role="status"><span className={`status-dot ${app.busy ? "status-working" : !app.settings ? "status-off" : ""}`}/>{status}</div>
      </section>

      <aside className="studio-rail" aria-label="Konfiguracja odsłuchu">
        <section className="source-card panel">
          <div className="section-heading"><span className="eyebrow">MUZYKA W TLE</span><Icon name="music" size={20}/></div>
          <h3>{app.source?.displayName ?? (app.loading ? "Wczytywanie źródła…" : !app.settings ? "Czekamy na połączenie" : app.settings.preferredAudioSource ? "Źródło niedostępne" : "Wybierz swoją muzykę")}</h3>
          <p className="muted small">{app.source?.processName ?? (app.settings?.preferredAudioSource ? "Odśwież sesje i wybierz aktywne źródło." : "Połącz sesję odtwarzacza, którego słuchasz.")}</p>
          {app.source && <p className="source-level"><span className={`status-dot ${app.source.state==="active"?"":"status-off"}`}/>{app.source.state==="active"?"Sesja aktywna":"Sesja nieaktywna"} · <strong>{Math.round(app.source.volume * 100)}%</strong></p>}
          <button className="text-button" onClick={() => navigate("settings")}>{app.source ? "Zmień źródło" : "Przejdź do ustawień"}<Icon name="arrow" size={16}/></button>
        </section>
        <section className="schedule-note"><Icon name="clock" size={20}/><div><h3>{app.scheduledActive ? app.paused ? "Odliczanie wstrzymane" : "Do następnej audycji" : "Twój rytm audycji"}</h3>{app.scheduledActive && <strong className="schedule-countdown" role="timer">{countdown(app.schedule.remainingMs)}</strong>}<p>{app.scheduledActive ? `Interwał ${app.settings!.intervalMs / 60000} min · ${app.schedule.ready ? "nagranie gotowe" : app.schedule.phase === "error" || app.schedule.phase === "waiting" ? "brak gotowego nagrania" : "trwa przygotowanie"}` : `Po uruchomieniu pierwsza audycja za ${app.settings ? app.settings.intervalMs / 60000 : 30} min. Potem każdy pełny interwał liczony jest od końca odsłuchu.`}</p></div></section>
      </aside>
    </div>

    {!configured && app.settings && !app.loading && <div className="setup-strip"><Icon name={!app.source ? "music" : "topics"} size={20}/><p>{!app.source ? "Jeszcze jeden krok: wybierz źródło muzyki." : "O czym ma być audycja? Dodaj pierwszy temat."}</p><button className="text-button" onClick={() => navigate(!app.source ? "settings" : "topics")}>{!app.source ? "Wybierz źródło" : "Dodaj temat"}<Icon name="arrow" size={16}/></button></div>}

    <div className="dashboard-grid">
      <section className="panel preparation" aria-labelledby="preparation-title">
        <div className="section-heading"><h3 id="preparation-title">Na warsztacie</h3><span className={`badge ${app.busy ? "badge-working" : app.ready ? "badge-success" : ""}`}>{app.busy ? "W toku" : app.ready ? "Gotowa do odsłuchu" : "Nowa audycja"}</span></div>
        <h4>{app.ready && !app.busy ? app.ready.script.title : activeLabel}</h4>
        <p className="muted small">{app.stopping && app.stage === "content" ? "Czekamy na odpowiedź modelu tekstowego. Kolejne etapy nie zostaną uruchomione." : app.scheduledActive && app.schedule.phase === "error" ? "Nie mamy gotowego nagrania. Termin zostanie pominięty; muzyka będzie grała bez zmian." : "Najpierw przygotujemy całą audycję. Muzyka ściszy się dopiero na czas odsłuchu."}</p>
        <ol className="pipeline" aria-label="Etapy przygotowania audycji">{stages.map((stage,index) => {
          const current = app.stage === stage;
          const done = activeIndex > index || (!app.busy && !!app.ready && index < 3);
          return <li key={stage} className={current ? "current" : done ? "complete" : ""} aria-current={current ? "step" : undefined}><span className="step-number">{done ? <Icon name="check" size={14}/> : `0${index+1}`}</span><span>{["Wiadomości","Scenariusz","Nagranie","Odsłuch"][index]}</span></li>;
        })}</ol>
        {app.ready && <div className="ready-summary"><Icon name="check" size={17}/><p>{(app.ready.audio.durationMs/60000).toLocaleString("pl-PL", { maximumFractionDigits: 1 })} min <span>·</span> {app.ready.script.sources.length} źródła <span>·</span> głos {app.ready.audio.voice}</p></div>}
        <div className="button-row"><button className="button button-secondary" disabled={!app.settings?.topics.length || app.busy || app.saving} onClick={() => void app.start(true, false)}><Icon name="spark" size={16}/>{app.ready ? "Przygotuj nową audycję" : "Przygotuj bez odtwarzania"}</button><button className="text-button" disabled={!app.ready || !app.source || app.busy} onClick={() => void app.start(false, true)}><Icon name="play" size={15}/>Odsłuch testowy</button></div>
        <p className="footnote">Nowe przygotowanie korzysta z płatnego API OpenAI.</p>
      </section>

      <section className="last-broadcast" aria-labelledby="last-title">
        <div className="section-heading"><h3 id="last-title">Ostatnio na antenie</h3><Icon name="radio" size={20}/></div>
        {app.lastPlayed ? <div className="played-item"><span className="eyebrow">ODSŁUCH ZAKOŃCZONY</span><h4>{app.lastPlayed.broadcast.script.title}</h4><p className="small muted">{new Date(app.lastPlayed.at).toLocaleString("pl-PL")}<br/>{(app.lastPlayed.broadcast.audio.durationMs/60000).toLocaleString("pl-PL", { maximumFractionDigits: 1 })} min</p></div> : <div className="empty-state"><span className="empty-icon"><Icon name="radio" size={26}/></span><h4>Tu usłyszysz więcej</h4><p>Po pierwszym odsłuchu pojawi się tutaj Twoja zakończona audycja.</p></div>}
        <p className="footnote">Odsłuchy z bieżącej sesji aplikacji.</p>
        <div className="sound-check"><button className="text-button" disabled={app.busy || !app.settings} onClick={() => void app.testSound()}><Icon name="volume" size={17}/>Test wyjścia dźwięku</button><p className="footnote">Krótkie nagranie testowe · bez API</p></div>
      </section>
    </div>

    <section className="interests-overview"><div className="section-heading"><div><p className="eyebrow">TWOJE CZĘSTOTLIWOŚCI</p><h3>To, co Cię interesuje</h3></div><button className="text-button" onClick={() => navigate("topics")}>Edytuj tematy<Icon name="arrow" size={16}/></button></div><div className="topic-pills">{app.settings?.topics.length ? app.settings.topics.map(topic => <span className="topic-pill" key={topic}>{topic}</span>) : <p className="muted small">{app.loading ? "Wczytywanie tematów…" : !app.settings ? "Twoje tematy pojawią się po połączeniu z aplikacją." : "Jeszcze cisza. Dodaj tematy, a znajdziemy dla Ciebie wiadomości."}</p>}</div></section>

    {(app.ready || app.news) && <details className="panel source-details"><summary>Scenariusz i materiały źródłowe<span className="muted">Rozwiń, aby przeczytać</span></summary>{app.ready && <><h3>{app.ready.script.title}</h3><p className="script-text">{app.ready.script.text}</p>{app.ready.script.draft.limitations.map((text,index) => <p className="warning-text small" key={index}>{text}</p>)}</>}{(app.ready?.script.sources ?? app.news?.items ?? []).map(item => <article className="source-row" key={item.id}><strong>{item.title}</strong><p className="small muted">{item.source ?? "Brak nazwy źródła w RSS"} · {item.publishedAt ? new Date(item.publishedAt).toLocaleString("pl-PL") : "Brak daty"}</p><a className="source-url" href={item.url} target="_blank" rel="noopener noreferrer">{item.url}</a></article>)}{app.news?.topics.filter(t=>t.error).map(t=><p key={t.topic} className="warning-text small">{t.topic}: {t.error}</p>)}</details>}
  </>;
}
