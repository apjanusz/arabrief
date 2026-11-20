import { useCopy } from "../i18n";
import { useEffect, useId, useRef, useState } from "react";
import type { ReactNode, ComponentProps } from "react";
import type { AppSettings } from "../../domain/models";
import type { CredentialStore, KeyValidation } from "../../modules/settings/CredentialStore";
import type { Workspace } from "../useAraBrief";
import { errorText } from "../useAraBrief";
import { Icon, BrandMark } from "../components/Icon";
import { notifications } from "../notifications/notifications";
import { useTranslation } from "../i18n";
import { isLanguagePreference } from "../../domain/language";
import { CountryPicker } from "../components/CountryPicker";

const voices = ["marin", "cedar"];
// Responses web_search and Standard USD/1M input/output checked 2026-11-16.
// https://developers.openai.com/api/docs/pricing — 0.25/2, 0.75/4.5, 2.5/15.
const models = [
  { id: "gpt-5-mini", name: "GPT-5 mini", cost: "$" },
  { id: "gpt-5.4-mini", name: "GPT-5.4 mini", cost: "$$" },
  { id: "gpt-5.4", name: "GPT-5.4", cost: "$$$" },
];
function ModelSelect({ value, change }: { value:string; change:(value:string)=>void }) {
  const copy = useCopy();
  const id=useId(); const root=useRef<HTMLDivElement>(null); const trigger=useRef<HTMLButtonElement>(null);
  const [open,setOpen]=useState(false);
  const selected=models.find(model=>model.id===value);
  const [custom,setCustom]=useState(!selected);
  const [customValue,setCustomValue]=useState(selected?"":value);
  useEffect(()=>{setCustom(!models.some(model=>model.id===value));},[value]);
  function close() {setOpen(false);trigger.current?.focus();}
  function focusOption(index:number) { const options=root.current?.querySelectorAll<HTMLButtonElement>('[role="option"]');options?.[(index+options.length)%options.length]?.focus(); }
  const options=[...models,{id:"other",name:copy("ui_3cf5a4f34f"),cost:""}];
  return <div ref={root} className="field model-field" onBlur={event=>{if(!event.currentTarget.contains(event.relatedTarget))setOpen(false);}} onKeyDown={event=>{if(event.key==="Escape"&&open){event.preventDefault();close();}}}>
    <label id={`${id}-label`} htmlFor={`${id}-trigger`}> {copy("ui_a18715e38d")} </label>
    <div className="model-select">
      <button id={`${id}-trigger`} ref={trigger} type="button" className="model-trigger" aria-haspopup="listbox" aria-expanded={open} aria-controls={`${id}-options`} onClick={()=>setOpen(!open)} onKeyDown={event=>{if(event.key==="ArrowDown"||event.key==="ArrowUp"){event.preventDefault();setOpen(true);requestAnimationFrame(()=>focusOption(event.key==="ArrowUp"?options.length-1:0));}}}>
        <span>{custom?copy("ui_3cf5a4f34f"):selected?.name??value}</span><span className="model-cost">{custom?"":selected?.cost}</span><Icon name="arrow" size={13}/>
      </button>
      {open&&<div id={`${id}-options`} className="model-options" role="listbox" aria-labelledby={`${id}-label`}>{options.map((model,index)=><button key={model.id} type="button" role="option" aria-selected={model.id==="other"?custom:!custom&&model.id===value} className={`model-option ${(model.id==="other"?custom:!custom&&model.id===value)?"is-selected":""}`} onKeyDown={event=>{if(["ArrowDown","ArrowUp","Home","End"].includes(event.key)){event.preventDefault();focusOption(event.key==="Home"?0:event.key==="End"?options.length-1:index+(event.key==="ArrowDown"?1:-1));}}} onClick={()=>{
        if(model.id==="other"){setCustom(true);change(customValue);}else{if(custom)setCustomValue(value);setCustom(false);change(model.id);}close();
      }}><span>{model.name}</span><span className="model-cost">{model.cost}</span></button>)}</div>}
    </div>
    {custom&&<><label htmlFor={`${id}-custom`}> {copy("ui_3c70d5f050")} </label><input id={`${id}-custom`} required maxLength={100} pattern="[A-Za-z0-9_.:\-]+" value={value} onChange={event=>{setCustomValue(event.target.value);change(event.target.value);}} placeholder={copy('ui_0d87a7958f')} autoComplete="off" spellCheck={false}/><small> {copy("ui_445a7b6d40")} </small></>}
    <small> {copy("ui_bc788b4510")} </small>
  </div>;
}
function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {

  return <label className="field"><span>{label}</span>{children}{hint && <small>{hint}</small>}</label>;
}
function NumberField({ label, value, min, max, step=1, unit, change }: { label:string; value:number; min:number; max:number; step?:number; unit:string; change:(value:number)=>void }) {
  const copy = useCopy();
  const id=useId();
  const display=value.toLocaleString(copy("locale"),{maximumFractionDigits:unit==="s"?1:2});
  return <div className="field volume-field"><div className="range-heading"><label htmlFor={id}>{label}</label><output htmlFor={id}>{display}<small>{unit}</small></output></div><input id={id} aria-valuetext={`${display} ${unit}`} type="range" min={min} max={max} step={step} value={value} onChange={event=>change(Number(event.target.value))}/></div>;
}
function VolumeField({ label, value, hint, change }: { label:string; value:number; hint:string; change:(value:number)=>void }) {
  const copy = useCopy();

  const id = useId();
  return <div className="field volume-field"><div className="range-heading"><label htmlFor={id}>{label}</label><output htmlFor={id}>{Math.round(value*100)}<small>%</small></output></div><input id={id} aria-describedby={`${id}-hint`} aria-valuetext={`${Math.round(value*100)} ${copy("percent")}`} type="range" min="0" max="1" step="0.01" value={value} onChange={e=>change(Number(e.target.value))}/><small id={`${id}-hint`}>{hint}</small></div>;
}
function SettingsSection({ id, icon, title, description, disabled, children }: { id:string; icon:ComponentProps<typeof Icon>["name"]; title:string; description:string; disabled?:boolean; children:ReactNode }) {

  return <fieldset id={id} disabled={disabled} className="settings-section"><legend className="sr-only">{title}</legend><div className="settings-section-intro"><span className="icon-chip"><Icon name={icon}/></span><h2>{title}</h2><p>{description}</p></div><div className="settings-section-body">{children}</div></fieldset>;
}

export function Settings({ app, credentials }: { app: Workspace; credentials: CredentialStore }) {
  const copy = useCopy();
  const { language, t } = useTranslation();
  const [draft,setDraft] = useState<AppSettings | null>(app.settings);
  const [formError,setFormError] = useState("");
  const [refreshing,setRefreshing] = useState(false);
  useEffect(()=>setDraft(app.settings),[app.settings]);
  const locked = app.busy || app.saving || !draft;
  const update = <K extends keyof AppSettings>(key:K,value:AppSettings[K]) => setDraft(current=>current?{...current,[key]:value}:null);
  async function submit(event:React.FormEvent) {
    event.preventDefault(); if (!draft) return; setFormError("");
    if (draft.broadcastMinDurationMs>draft.broadcastMaxDurationMs) { setFormError(copy("ui_5ecbd097b4")); return; }
    try { await app.save(draft); } catch { /* Workspace displays native validation errors. */ }
  }
  const selected = app.sessions.find(s=>s.id===draft?.preferredAudioSource?.id);
  const dirty = !!draft && JSON.stringify(draft) !== JSON.stringify(app.settings);
  if (!draft) return <section className="panel empty-state connection-state" aria-busy={app.loading}><BrandMark/><span className="eyebrow"> {copy("ui_2798a4ec09")} </span><h2>{app.loading ? copy("ui_93b05bafe6") : copy("ui_e847c899b7")}</h2><p> {copy("ui_8bdb85d4db")} </p><button className="button button-secondary" disabled={app.loading} onClick={()=>void app.reconnect()}><Icon name="refresh" size={16}/>{app.loading?copy("ui_c1994a4f99"):copy("ui_2e283b6a79")}</button></section>;
  return <>
    <nav className="settings-nav" aria-label={copy("ui_d3356992a8")}>{[["music-settings",copy("ui_1a38c8918f")],["sound-settings",copy("ui_4f36ed8eed")],["broadcast-settings",copy('ui_b9f5ba9e84')],["voice-settings",copy("ui_dbb8249fe8")],["background-settings",copy("ui_e9af572d08")],["api-settings",copy('ui_7f8d132328')]].map(([id,label])=><a href={`#${id}`} key={id}>{copy(label)}<Icon name="arrow" size={13}/></a>)}</nav>
    <form onSubmit={e=>void submit(e)} className="settings-form">
      <SettingsSection id="language-settings" icon="settings" title={t("languageTitle")} description={t("languageDescription")} disabled={locked}>
        <CountryPicker value={draft.newsRegion} onChange={value=>update("newsRegion",value)}/>
        <div lang={language}><Field label={t("languageLabel")} hint={t("languageHint")}><select value={draft.uiLanguage} onChange={event=>{if(isLanguagePreference(event.target.value))update("uiLanguage",event.target.value);}}><option value="auto">{t("automatic")}</option><option value="pl" lang="pl">Polski</option><option value="en" lang="en">English</option></select></Field></div>
      </SettingsSection>
      <SettingsSection id="music-settings" icon="music" title={copy('ui_f760c21c4c')} description={copy("ui_4bfd02264f")} disabled={locked}>
        <div className="field-toolbar"><span className="eyebrow"> {copy("ui_c8142e6ffb")} </span><button type="button" className="text-button" disabled={refreshing || app.busy} onClick={()=>{setRefreshing(true);void app.refreshSessions().finally(()=>setRefreshing(false));}}><Icon name="refresh" size={15}/>{refreshing?copy("ui_3fb27eb205"):copy("ui_9b205c3f88")}</button></div>
        <Field label={copy("ui_1a38c8918f")}><select value={draft.preferredAudioSource?.id??""} onChange={e=>{const source=app.sessions.find(s=>s.id===e.target.value);update("preferredAudioSource",source?{id:source.id,name:source.displayName}:null);}}><option value=""> {copy("ui_f88f7cbfb0")} </option>{draft.preferredAudioSource && !selected && <option value={draft.preferredAudioSource.id}>{draft.preferredAudioSource.name}  {copy("ui_cd942336e6")} </option>}{app.sessions.map(source=><option key={source.id} value={source.id}>{source.displayName} · {source.processName??`PID ${source.pid}`} · {source.deviceId.slice(-14)}</option>)}</select></Field>
        {selected ? <div className="session-meta"><span className={`status-dot ${selected.state==="active"?"":"status-off"}`}/><div>{selected.state==="active"?copy('ui_10f8647bd8'):copy('ui_b823c3fa57')} <span>·</span>  {copy("ui_f5352f7883")} {Math.round(selected.volume*100)}%<details><summary> {copy("ui_746dea9989")} </summary><p>PID {selected.pid}<br/>{selected.deviceId}</p></details></div></div> : <div className="inline-note"><Icon name="music" size={17}/><p> {copy("ui_fc8c7c9dd1")} </p></div>}
      </SettingsSection>

      <SettingsSection id="sound-settings" icon="volume" title={copy("ui_005769e358")} description={copy("ui_6e500564e2")} disabled={locked}>
        <div className="form-grid"><VolumeField label={copy('ui_4c05caf016')} value={draft.duckVolume} change={v=>update("duckVolume",v)} hint={copy("ui_f795078068")}/><VolumeField label={copy("ui_9a93085890")} value={draft.narratorVolume} change={v=>update("narratorVolume",v)} hint={copy("ui_d2ccb134de")}/><NumberField label={copy("ui_e9cdda994b")} min={0} max={30} step={0.1} unit="s" value={draft.fadeOutMs/1000} change={v=>update("fadeOutMs",Math.round(v*1000))}/><NumberField label={copy('ui_612027d8e9')} min={0} max={30} step={0.1} unit="s" value={draft.fadeInMs/1000} change={v=>update("fadeInMs",Math.round(v*1000))}/></div><p className="footnote"> {copy("ui_97a9418a45")} </p>
      </SettingsSection>

      <SettingsSection id="broadcast-settings" icon="clock" title={copy('ui_e188d51794')} description={copy("ui_84c9d91b92")} disabled={locked}>
        <div className="form-grid"><NumberField label={copy("ui_4f37703e8a")} min={0.5} max={15} step={0.5} unit="min" value={draft.broadcastMinDurationMs/60000} change={v=>update("broadcastMinDurationMs",v*60000)}/><NumberField label={copy("ui_e97eac330e")} min={0.5} max={15} step={0.5} unit="min" value={draft.broadcastMaxDurationMs/60000} change={v=>update("broadcastMaxDurationMs",v*60000)}/></div><p className="footnote"> {copy("ui_8057db5dd9")} </p>
        <div className="schedule-settings"><NumberField label={copy("ui_2b1906e478")} min={5} max={1440} unit="min" value={draft.intervalMs/60000} change={v=>update("intervalMs",v*60000)}/><div><span className="badge"> {copy("ui_b5a986a17c")} </span><p className="small muted"> {copy("ui_ad3db79fc8")} </p></div></div>
        <p className="footnote">{copy("minimumBroadcastGap")}</p>
      </SettingsSection>

      <SettingsSection id="voice-settings" icon="radio" title={copy("ui_08341c2a9d")} description={copy('ui_cb91377303')} disabled={locked}>
        <div className="form-grid"><Field label={copy("ui_6eee7b29e3")}><select value={draft.openAiVoice} onChange={e=>update("openAiVoice",e.target.value)}>{voices.map(voice=><option key={voice}>{voice}</option>)}</select></Field><ModelSelect value={draft.textModel} change={value=>update("textModel",value)}/></div><p className="footnote"> {copy("ui_bdfbd033c2")} </p>
      </SettingsSection>
      <SettingsSection id="background-settings" icon="settings" title={copy("ui_e9af572d08")} description={copy("ui_87f7cdef2d")} disabled={locked}>
        <Field label={copy('ui_468354ce47')} hint={copy("ui_d674e37cc0")}><span className="checkbox-option"><input type="checkbox" checked={draft.autoStart} onChange={e=>update("autoStart",e.target.checked)}/>  {copy("ui_7b2556cc44")} </span></Field>
        <Field label={copy('ui_91351305d8')} hint={copy("ui_b072ebf92b")}><span className="checkbox-option"><input type="checkbox" checked={draft.startSchedulerOnLaunch} onChange={e=>update("startSchedulerOnLaunch",e.target.checked)}/>  {copy("ui_62809e6a24")} </span></Field>
        <p className="footnote"> {copy("ui_bf82a5cf26")} </p>
      </SettingsSection>
      {formError && <p role="alert" className="message message-error">{copy(formError)}</p>}
      <div className={`settings-save ${dirty?"has-changes":""}`}><p role="status">{(app.saving || dirty) && <><Icon name="settings" size={16}/>{app.saving?copy("ui_177003e983"):copy('ui_c7f52c17ec')}</>}</p><div className="button-row"><button type="button" className="button button-secondary" disabled={locked || !dirty} onClick={()=>{setDraft(app.settings);setFormError("");notifications.info(copy("ui_770676d418"));}}> {copy("ui_9234a06836")} </button><button className="button button-primary" disabled={locked || !dirty}>{app.saving?copy("ui_177003e983"):copy('ui_e2994c3283')}<Icon name="check" size={17}/></button></div></div>
    </form>
    <Credentials credentials={credentials}/>
  </>;
}

function Credentials({credentials}:{credentials:CredentialStore}) {
  const copy = useCopy();
  const [busy,setBusy] = useState(false);
  const [validation,setValidation] = useState<KeyValidation|null>(null);
  async function run(action:()=>Promise<void>) { setBusy(true);try {await action();}catch(cause){setValidation(null);notifications.error(errorText(cause), { title: copy("ui_100a828f42") });}finally{setBusy(false);} }
  return <SettingsSection id="api-settings" icon="key" title={copy("ui_0bb6d13b6f")} description={copy("ui_5085ce1fd1")} disabled={busy}>
    <div className="credential-heading"><Icon name="key" size={23}/><div><h3>Windows Credential Manager</h3><p className="small muted"> {copy("ui_54c69f58ad")} </p></div></div>
    <p className="muted small"> {copy("ui_68f42d3338")} </p>
    <div className="button-row"><button className="button button-secondary" onClick={()=>void run(async()=>{const saved=await credentials.save();if(saved)setValidation(null);if(saved)notifications.success(copy('ui_4160ff9001'));else notifications.info(copy('ui_37a6d8a58c'));})}><Icon name="plus" size={16}/> {copy("ui_d2cb009435")} </button><button className="button button-secondary" onClick={()=>void run(async()=>{const result=await credentials.check();setValidation(result);if(result.status==="accepted")notifications.success(result.message, { title: copy("ui_9161af0082") });else if(result.status==="rejected")notifications.error(result.message, { title: copy('ui_6c488629e9') });else notifications.warning(result.message, { title: copy("ui_12d3378811") });})}>{busy?copy("ui_d22cc791a3"):copy("ui_b5047ed4cb")}</button><button className="text-button danger-text" onClick={()=>void run(async()=>{await credentials.delete();setValidation(null);notifications.success(copy("ui_ee6c9ef93b"));})}> {copy("ui_d447927cb2")} </button></div>
    {validation && <span className={`badge credential-badge ${validation.status==="accepted"?"badge-success":""}`}>{validation.status==="accepted"?copy("ui_9161af0082"):validation.status==="not_configured"?copy('ui_ef13bdc0c1'):copy("ui_e36b13cd81")}</span>}
    <p className="footnote"> {copy("ui_7372b6bd7a")} </p>
  </SettingsSection>;
}
