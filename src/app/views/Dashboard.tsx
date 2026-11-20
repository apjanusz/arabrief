import { useCopy } from "../i18n";
import type { Workspace } from "../useAraBrief";
import { Icon } from "../components/Icon";

const stages = ["news", "content", "speech", "playing"] as const;
const stageNames = { idle: "ui_ee4c2aeb43", news: "ui_ee5454ca01", content: "ui_899bbf5adf", speech: "ui_7a17f783db", playing: "ui_bbf2984888" };
function countdown(value: number | null) {
  if (value === null) return "—:—";
  const seconds = Math.ceil(value / 1000);
  return [Math.floor(seconds / 3600), Math.floor(seconds % 3600 / 60), seconds % 60].map(part => String(part).padStart(2, "0")).join(":");
}

export function Dashboard({ app, navigate }: { app: Workspace; navigate: (page: "topics" | "settings") => void }) {
  const copy = useCopy();
  const configured = !!app.settings && !!app.source && !!app.settings.topics.length;
  const activeIndex = stages.findIndex(stage => stage === app.stage);
  const activeLabel = app.stopping ? copy('ui_f510066568') : app.pausePending ? copy('ui_c6cd44e5f0') : app.paused ? copy('ui_95a9fe3f70') : app.scheduledActive && app.schedule.phase === "restoring" ? copy("ui_95fc32aa28") : app.scheduledActive && app.schedule.phase === "ducking" ? copy("ui_c212ebe146") : app.scheduledActive && app.schedule.phase === "playing" ? copy('ui_3c7446abc0') : app.testing ? copy("ui_d6d6cb0e24") : copy(stageNames[app.stage]);
  const status = app.loading ? copy("ui_002638871e") : !app.settings ? copy("ui_40b8b37f17") : app.paused || app.pausePending ? activeLabel : app.scheduledActive ? app.schedule.phase === "waiting" ? app.schedule.ready ? copy('ui_0f6561b221') : app.schedule.message ?? copy("ui_7c15d292e8") : app.schedule.phase === "error" ? copy("ui_c12a7f5f10") : activeLabel : app.busy ? activeLabel : app.ready ? copy("ui_7b3bbd1a94") : copy('ui_6bde2c5542');
  return <>
    <div className="studio-grid">
      <section className={`hero panel ${app.busy ? "hero-active" : ""}`}>
        <div className="hero-top"><span className="eyebrow"> {copy("ui_30ff0f8ff2")} </span><span className="edition-label">ARA / BRIEF</span></div>
        <div className="hero-copy">
          <h2> {copy("ui_5aee84b572")} <br/><span> {copy("ui_741a6fbcf6")} </span></h2>
          <p> {copy("ui_40dc3650eb")} <br/> {copy("ui_dc4b9d052e")} </p>
        </div>
        <div className="hero-controls">
          <button className={`button ${app.busy ? "button-danger" : "button-primary"}`} disabled={app.loading || app.saving || app.stopping || (!app.busy && !configured)} onClick={() => void (app.busy ? app.stop() : app.startSchedule())}>
            <Icon name={app.busy ? "stop" : "play"} size={20}/>{app.stopping ? copy("ui_c01c73065d") : app.scheduledActive ? copy("ui_c21620fb29") : app.busy ? copy("ui_965a7eb80f") : copy('ui_d5dcaccc94')}
          </button>
          {app.canPause && <button className="button button-secondary" disabled={app.stopping || app.pausePending} aria-pressed={app.paused} onClick={() => void app.togglePause()}><Icon name={app.paused ? "play" : "pause"} size={20}/>{app.paused ? copy("ui_5567fcf35e") : copy("ui_b410a9a1e6")}</button>}
          <span className="hero-hint">{app.paused ? copy("ui_e3db935123") : app.scheduledActive ? copy("ui_a2d7530804") : copy("ui_92f6303a9f")}</span>
        </div>
        <div className="hero-status" role="status"><span className={`status-dot ${app.busy ? "status-working" : !app.settings ? "status-off" : ""}`}/>{status}</div>
      </section>

      <aside className="studio-rail" aria-label={copy("ui_13ba8edb6c")}>
        <section className="source-card panel">
          <div className="section-heading"><span className="eyebrow"> {copy("ui_d4eb518bd3")} </span><Icon name="music" size={20}/></div>
          <h3>{app.source?.displayName ?? (app.loading ? copy("ui_1d1791615b") : !app.settings ? copy("ui_c04225899d") : app.settings.preferredAudioSource ? copy("ui_d202b6ac45") : copy("ui_78d7a5969d"))}</h3>
          <p className="muted small">{app.source?.processName ?? (app.settings?.preferredAudioSource ? copy("ui_f04358e8ef") : copy("ui_d1396edd21"))}</p>
          {app.source && <p className="source-level"><span className={`status-dot ${app.source.state==="active"?"":"status-off"}`}/>{app.source.state==="active"?copy('ui_10f8647bd8'):copy('ui_b823c3fa57')} · <strong>{Math.round(app.source.volume * 100)}%</strong></p>}
          <button className="text-button" onClick={() => navigate("settings")}>{app.source ? copy("ui_79295d84b9") : copy("ui_819b2287db")}<Icon name="arrow" size={16}/></button>
        </section>
        <section className="schedule-note"><Icon name="clock" size={20}/><div><h3>{app.scheduledActive ? app.paused ? copy('ui_8d74addc23') : copy("ui_b6a9b22c1d") : copy("ui_578759d225")}</h3>{app.scheduledActive && <strong className="schedule-countdown" role="timer">{countdown(app.schedule.remainingMs)}</strong>}<p>{app.scheduledActive ? copy("ui_098fc88a4f").replace('{0}', String(app.settings!.intervalMs / 60000)).replace('{1}', String(app.schedule.ready ? copy('ui_c6f20ba4e4') : app.schedule.phase === "error" || app.schedule.phase === "waiting" ? copy('ui_d913882394') : copy('ui_dfb154029c'))) : copy("ui_201db04224").replace('{0}', String(app.settings ? app.settings.intervalMs / 60000 : 30))}</p></div></section>
      </aside>
    </div>

    {!configured && app.settings && !app.loading && <div className="setup-strip"><Icon name={!app.source ? "music" : "topics"} size={20}/><p>{!app.source ? copy("ui_4f40a78bb0") : copy("ui_a777efbd5b")}</p><button className="text-button" onClick={() => navigate(!app.source ? "settings" : "topics")}>{!app.source ? copy("ui_8cffd7e96b") : copy('ui_f9f8079ddf')}<Icon name="arrow" size={16}/></button></div>}

    <div className="dashboard-grid">
      <section className="panel preparation" aria-labelledby="preparation-title">
        <div className="section-heading"><h3 id="preparation-title"> {copy("ui_b5d10c506a")} </h3><span className={`badge ${app.busy ? "badge-working" : app.ready ? "badge-success" : ""}`}>{app.busy ? copy('ui_4c870f0532') : app.ready ? copy("ui_ab1d623774") : copy('ui_b7cdec34de')}</span></div>
        <h4>{app.ready && !app.busy ? app.ready.script.title : activeLabel}</h4>
        <p className="muted small">{app.stopping && app.stage === "content" ? copy("ui_88766fcf90") : app.scheduledActive && app.schedule.phase === "error" ? copy("ui_7729575dd8") : copy("ui_4c03de338e")}</p>
        <ol className="pipeline" aria-label={copy('ui_2f5bb80cb7')}>{stages.map((stage,index) => {
          const current = app.stage === stage;
          const done = activeIndex > index || (!app.busy && !!app.ready && index < 3);
          return <li key={stage} className={current ? "current" : done ? "complete" : ""} aria-current={current ? "step" : undefined}><span className="step-number">{done ? <Icon name="check" size={14}/> : `0${index+1}`}</span><span>{[copy("ui_e18df7670b"),copy('ui_664284064f'),copy('ui_6d90e89084'),copy("ui_375835919d")][index]}</span></li>;
        })}</ol>
        {app.ready && <div className="ready-summary"><Icon name="check" size={17}/><p>{(app.ready.audio.durationMs/60000).toLocaleString(copy("locale"), { maximumFractionDigits: 1 })} min <span>·</span> {app.ready.script.sources.length}  {copy("ui_1729fd3613")} <span>·</span>  {copy("ui_d98df7df9f")} {app.ready.audio.voice}</p></div>}
        <div className="button-row"><button className="button button-secondary" disabled={!app.settings?.topics.length || app.busy || app.saving} onClick={() => void app.start(true, false)}><Icon name="spark" size={16}/>{app.ready ? copy("ui_7a6b8bc611") : copy('ui_deca4f8a4b')}</button><button className="text-button" disabled={!app.ready || !app.source || app.busy} onClick={() => void app.start(false, true)}><Icon name="play" size={15}/> {copy("ui_a0f2ffdd18")} </button></div>
        <p className="footnote"> {copy("ui_b5d473edc6")} </p>
      </section>

      <section className="last-broadcast" aria-labelledby="last-title">
        <div className="section-heading"><h3 id="last-title"> {copy("ui_cefd22aff9")} </h3><Icon name="radio" size={20}/></div>
        {app.lastPlayed ? <div className="played-item"><span className="eyebrow"> {copy("ui_707a72ac3a")} </span><h4>{app.lastPlayed.broadcast.script.title}</h4><p className="small muted">{new Date(app.lastPlayed.at).toLocaleString(copy("locale"))}<br/>{(app.lastPlayed.broadcast.audio.durationMs/60000).toLocaleString(copy("locale"), { maximumFractionDigits: 1 })} min</p></div> : <div className="empty-state"><span className="empty-icon"><Icon name="radio" size={26}/></span><h4> {copy("ui_3bee5c2927")} </h4><p> {copy("ui_0c4cca90e7")} </p></div>}
        <p className="footnote"> {copy("ui_7947fdba6d")} </p>
        <div className="sound-check"><button className="text-button" disabled={app.busy || !app.settings} onClick={() => void app.testSound()}><Icon name="volume" size={17}/> {copy("ui_ef87818cc8")} </button><p className="footnote"> {copy("ui_3b3af7f38e")} </p></div>
      </section>
    </div>

    <section className="interests-overview"><div className="section-heading"><div><p className="eyebrow"> {copy("ui_815217fb6b")} </p><h3> {copy("ui_e14d492efa")} </h3></div><button className="text-button" onClick={() => navigate("topics")}> {copy("ui_fe14f82d0f")} <Icon name="arrow" size={16}/></button></div><div className="topic-pills">{app.settings?.topics.length ? app.settings.topics.map(topic => <span className="topic-pill" key={topic}>{topic}</span>) : <p className="muted small">{app.loading ? copy("ui_5ea8b01653") : !app.settings ? copy("ui_eb3b9a6cc4") : copy("ui_a18ff2e0fd")}</p>}</div></section>

    {(app.ready || app.news) && <details className="panel source-details"><summary> {copy("ui_d0ede73261")} <span className="muted"> {copy("ui_63d23ac161")} </span></summary>{app.ready && <><h3>{app.ready.script.title}</h3><p className="script-text">{app.ready.script.text}</p>{app.ready.script.draft.limitations.map((text,index) => <p className="warning-text small" key={index}>{text}</p>)}</>}{(app.ready?.script.sources ?? app.news?.items ?? []).map(item => <article className="source-row" key={item.id}><strong>{copy(item.title)}</strong><p className="small muted">{item.source ?? copy("ui_8ee6d4d493")} · {item.publishedAt ? new Date(item.publishedAt).toLocaleString(copy("locale")) : copy('ui_2d14f44ac3')}</p><a className="source-url" href={item.url} target="_blank" rel="noopener noreferrer">{item.url}</a></article>)}{app.news?.topics.filter(t=>t.error).map(t=><p key={t.topic} className="warning-text small">{t.topic}: {copy(t.error)}</p>)}</details>}
  </>;
}
