import { useEffect, useId, useState } from "react";
import type { ReactNode, ComponentProps } from "react";
import type { AppSettings } from "../../domain/models";
import type { CredentialStore, KeyValidation } from "../../modules/settings/CredentialStore";
import type { Workspace } from "../useAraBrief";
import { errorText } from "../useAraBrief";
import { Icon, BrandMark } from "../components/Icon";
import { notifications } from "../notifications/notifications";

const voices = ["alloy", "ash", "ballad", "coral", "echo", "sage", "shimmer", "verse", "marin", "cedar"];
function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return <label className="field"><span>{label}</span>{children}{hint && <small>{hint}</small>}</label>;
}
function NumberField({ label, value, min, max, step=1, unit, change }: { label:string; value:number; min:number; max:number; step?:number; unit:string; change:(value:number)=>void }) {
  return <Field label={label}><span className="unit-input"><input required type="number" min={min} max={max} step={step} value={Number.isFinite(value)?value:""} onChange={e=>change(e.target.value===""?NaN:Number(e.target.value))}/><span>{unit}</span></span></Field>;
}
function VolumeField({ label, value, hint, change }: { label:string; value:number; hint:string; change:(value:number)=>void }) {
  const id = useId();
  return <div className="field volume-field"><div className="range-heading"><label htmlFor={id}>{label}</label><output htmlFor={id}>{Math.round(value*100)}<small>%</small></output></div><input id={id} aria-describedby={`${id}-hint`} aria-valuetext={`${Math.round(value*100)} procent`} type="range" min="0" max="1" step="0.01" value={value} onChange={e=>change(Number(e.target.value))}/><small id={`${id}-hint`}>{hint}</small></div>;
}
function SettingsSection({ id, icon, title, description, disabled, children }: { id:string; icon:ComponentProps<typeof Icon>["name"]; title:string; description:string; disabled?:boolean; children:ReactNode }) {
  return <fieldset id={id} disabled={disabled} className="settings-section"><legend className="sr-only">{title}</legend><div className="settings-section-intro"><span className="icon-chip"><Icon name={icon}/></span><h2>{title}</h2><p>{description}</p></div><div className="settings-section-body">{children}</div></fieldset>;
}

export function Settings({ app, credentials }: { app: Workspace; credentials: CredentialStore }) {
  const [draft,setDraft] = useState<AppSettings | null>(app.settings);
  const [formError,setFormError] = useState("");
  const [refreshing,setRefreshing] = useState(false);
  useEffect(()=>setDraft(app.settings),[app.settings]);
  const locked = app.busy || app.saving || !draft;
  const update = <K extends keyof AppSettings>(key:K,value:AppSettings[K]) => setDraft(current=>current?{...current,[key]:value}:null);
  async function submit(event:React.FormEvent) {
    event.preventDefault(); if (!draft) return; setFormError("");
    if (draft.broadcastMinDurationMs>draft.broadcastMaxDurationMs) { setFormError("Minimalna długość audycji nie może przekraczać maksymalnej."); return; }
    try { await app.save(draft); } catch { /* Workspace displays native validation errors. */ }
  }
  const selected = app.sessions.find(s=>s.id===draft?.preferredAudioSource?.id);
  const dirty = !!draft && JSON.stringify(draft) !== JSON.stringify(app.settings);
  if (!draft) return <section className="panel empty-state connection-state" aria-busy={app.loading}><BrandMark/><span className="eyebrow">TWOJE PREFERENCJE</span><h2>{app.loading ? "Łączymy się z Twoim studiem…" : "Potrzebujemy połączenia"}</h2><p>Ustawienia czekają w aplikacji desktopowej AraBrief. Po połączeniu zobaczysz tu swoje źródło muzyki, głos i preferencje audycji.</p><button className="button button-secondary" disabled={app.loading} onClick={()=>void app.reconnect()}><Icon name="refresh" size={16}/>{app.loading?"Łączenie…":"Ponów połączenie"}</button></section>;
  return <>
    <nav className="settings-nav" aria-label="Sekcje ustawień">{[["music-settings","Źródło muzyki"],["sound-settings","Dźwięk"],["broadcast-settings","Audycja"],["voice-settings","Głos i model"],["api-settings","Klucz API"]].map(([id,label])=><a href={`#${id}`} key={id}>{label}<Icon name="arrow" size={13}/></a>)}</nav>
    <form onSubmit={e=>void submit(e)} className="settings-form">
      <SettingsSection id="music-settings" icon="music" title="Twoja muzyka" description="Wybierz odtwarzacz, który będzie towarzyszył audycji." disabled={locked}>
        <div className="field-toolbar"><span className="eyebrow">SESJE WINDOWS</span><button type="button" className="text-button" disabled={refreshing || app.busy} onClick={()=>{setRefreshing(true);void app.refreshSessions().finally(()=>setRefreshing(false));}}><Icon name="refresh" size={15}/>{refreshing?"Odświeżanie…":"Odśwież listę"}</button></div>
        <Field label="Źródło muzyki"><select value={draft.preferredAudioSource?.id??""} onChange={e=>{const source=app.sessions.find(s=>s.id===e.target.value);update("preferredAudioSource",source?{id:source.id,name:source.displayName}:null);}}><option value="">Wybierz sesję audio</option>{draft.preferredAudioSource && !selected && <option value={draft.preferredAudioSource.id}>{draft.preferredAudioSource.name} — sesja niedostępna</option>}{app.sessions.map(source=><option key={source.id} value={source.id}>{source.displayName} · {source.processName??`PID ${source.pid}`} · {source.deviceId.slice(-14)}</option>)}</select></Field>
        {selected ? <div className="session-meta"><span className={`status-dot ${selected.state==="active"?"":"status-off"}`}/><div>{selected.state==="active"?"Sesja aktywna":"Sesja nieaktywna"} <span>·</span> głośność {Math.round(selected.volume*100)}%<details><summary>Informacje o sesji</summary><p>PID {selected.pid}<br/>{selected.deviceId}</p></details></div></div> : <div className="inline-note"><Icon name="music" size={17}/><p>Włącz muzykę w swoim odtwarzaczu i odśwież listę. Po ponownym uruchomieniu odtwarzacza wybierz nową sesję.</p></div>}
      </SettingsSection>

      <SettingsSection id="sound-settings" icon="volume" title="Dźwięk i przejścia" description="Muzyka zostaje w tle. Ustaw proporcje, które lubisz." disabled={locked}>
        <div className="form-grid"><VolumeField label="Muzyka podczas audycji" value={draft.duckVolume} change={v=>update("duckVolume",v)} hint="Procent pierwotnej głośności wybranej sesji."/><VolumeField label="Głośność lektora" value={draft.narratorVolume} change={v=>update("narratorVolume",v)} hint="Niezależny poziom odtwarzanego nagrania."/><NumberField label="Czas ściszania" min={0} max={30} step={0.1} unit="s" value={draft.fadeOutMs/1000} change={v=>update("fadeOutMs",Math.round(v*1000))}/><NumberField label="Czas przywracania" min={0} max={30} step={0.1} unit="s" value={draft.fadeInMs/1000} change={v=>update("fadeInMs",Math.round(v*1000))}/></div><p className="footnote">Płynne przejścia, bez pauzowania i zmiany globalnej głośności Windows.</p>
      </SettingsSection>

      <SettingsSection id="broadcast-settings" icon="clock" title="Rytm audycji" description="Krótka chwila z wiadomościami, dopasowana do Ciebie." disabled={locked}>
        <div className="form-grid"><NumberField label="Minimalna długość" min={0.5} max={15} step={0.5} unit="min" value={draft.broadcastMinDurationMs/60000} change={v=>update("broadcastMinDurationMs",v*60000)}/><NumberField label="Maksymalna długość" min={0.5} max={15} step={0.5} unit="min" value={draft.broadcastMaxDurationMs/60000} change={v=>update("broadcastMaxDurationMs",v*60000)}/></div><p className="footnote">Orientacyjny czas. Jeśli dostępne są tylko nagłówki, audycja może być krótsza.</p>
        <div className="schedule-settings"><NumberField label="Interwał audycji" min={1} max={1440} unit="min" value={draft.intervalMs/60000} change={v=>update("intervalMs",v*60000)}/><div><span className="badge">Start w Twoim studiu</span><p className="small muted">Po kliknięciu Start pierwsza audycja zabrzmi po pełnym interwale. Każdy następny interwał zaczyna się po zakończonym odsłuchu.</p></div></div>
      </SettingsSection>

      <SettingsSection id="voice-settings" icon="radio" title="Głos Twojego studia" description="Lektor AI czyta przygotowany scenariusz po polsku." disabled={locked}>
        <div className="form-grid"><Field label="Głos OpenAI"><select value={draft.openAiVoice} onChange={e=>update("openAiVoice",e.target.value)}>{!voices.includes(draft.openAiVoice) && <option value={draft.openAiVoice}>{draft.openAiVoice} (zapisany)</option>}{voices.map(voice=><option key={voice}>{voice}</option>)}</select></Field><Field label="Model tekstowy" hint="Responses API i Structured Outputs."><input required maxLength={100} value={draft.textModel} onChange={e=>update("textModel",e.target.value)} spellCheck={false}/></Field></div><p className="footnote">Przygotowanie nowych scenariuszy i nagrań korzysta z płatnego API OpenAI.</p>
      </SettingsSection>
      {formError && <p role="alert" className="message message-error">{formError}</p>}
      <div className={`settings-save ${dirty?"has-changes":""}`}><p role="status">{(app.saving || dirty) && <><Icon name="settings" size={16}/>{app.saving?"Zapisywanie…":"Masz niezapisane zmiany"}</>}</p><div className="button-row"><button type="button" className="button button-secondary" disabled={locked || !dirty} onClick={()=>{setDraft(app.settings);setFormError("");notifications.info("Cofnięto niezapisane zmiany.");}}>Cofnij</button><button className="button button-primary" disabled={locked || !dirty}>{app.saving?"Zapisywanie…":"Zapisz ustawienia"}<Icon name="check" size={17}/></button></div></div>
    </form>
    <Credentials credentials={credentials}/>
  </>;
}

function Credentials({credentials}:{credentials:CredentialStore}) {
  const [busy,setBusy] = useState(false);
  const [validation,setValidation] = useState<KeyValidation|null>(null);
  async function run(action:()=>Promise<void>) { setBusy(true);try {await action();}catch(cause){setValidation(null);notifications.error(errorText(cause), { title: "Błąd obsługi klucza API" });}finally{setBusy(false);} }
  return <SettingsSection id="api-settings" icon="key" title="Połączenie z OpenAI" description="Twój klucz. Bezpiecznie zapisany na tym komputerze." disabled={busy}>
    <div className="credential-heading"><Icon name="key" size={23}/><div><h3>Windows Credential Manager</h3><p className="small muted">Klucz wpiszesz w osobnym, natywnym oknie Windows.</p></div></div>
    <p className="muted small">Klucz nie jest wyświetlany w interfejsie ani zapisywany w bazie ustawień.</p>
    <div className="button-row"><button className="button button-secondary" onClick={()=>void run(async()=>{const saved=await credentials.save();if(saved)setValidation(null);if(saved)notifications.success("Klucz zapisano w bezpiecznym magazynie.");else notifications.info("Anulowano zapis klucza.");})}><Icon name="plus" size={16}/>Zapisz klucz</button><button className="button button-secondary" onClick={()=>void run(async()=>{const result=await credentials.check();setValidation(result);if(result.status==="accepted")notifications.success(result.message, { title: "Klucz przyjęty przez API" });else if(result.status==="rejected")notifications.error(result.message, { title: "Klucz odrzucony przez API" });else notifications.warning(result.message, { title: "Sprawdź konfigurację klucza" });})}>{busy?"Oczekiwanie…":"Sprawdź klucz"}</button><button className="text-button danger-text" onClick={()=>void run(async()=>{await credentials.delete();setValidation(null);notifications.success("Usunięto klucz z magazynu.");})}>Usuń klucz</button></div>
    {validation && <span className={`badge credential-badge ${validation.status==="accepted"?"badge-success":""}`}>{validation.status==="accepted"?"Klucz przyjęty przez API":validation.status==="not_configured"?"Brak zapisanego klucza":"Sprawdź wynik diagnostyki"}</span>}
    <p className="footnote">Sprawdzenie klucza odczytuje metadane modeli. Nie generuje treści i nie potwierdza salda ani uprawnień do generowania.</p>
  </SettingsSection>;
}
