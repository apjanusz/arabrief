import { useCopy } from "./app/i18n";
import { useRef, useState } from "react";
import "./App.css";
import { audioSessionReader } from "./platform/tauri/audio";
import { audioPlayer } from "./platform/tauri/player";
import { settingsRepository, apiCredentials } from "./platform/tauri/settings";
import { newsProvider } from "./platform/tauri/news";
import { contentGenerator } from "./platform/tauri/content";
import { speechProvider } from "./platform/tauri/speech";
import { preparationRecovery } from "./platform/tauri/recovery";
import { backgroundLifecycle } from "./platform/tauri/background";
import { useAraBrief } from "./app/useAraBrief";
import { Dashboard } from "./app/views/Dashboard";
import { Interests } from "./app/views/Interests";
import { Settings } from "./app/views/Settings";
import { Icon } from "./app/components/Icon";
import logo from "./assets/brand/AraBrief-logo-ciemne.svg";
import { ToastViewport } from "./app/notifications/ToastViewport";
import { LanguageProvider } from "./app/i18n";

const services = { background: backgroundLifecycle, recovery: preparationRecovery, settings: settingsRepository, audio: audioSessionReader, player: audioPlayer, news: newsProvider, content: contentGenerator, speech: speechProvider };
const pageDefinitions = (copy: ReturnType<typeof useCopy>) => ({
  dashboard: { title: copy("ui_4f53532fad"), subtitle: copy("ui_ba99f878a8"), icon: "radio" },
  topics: { title: copy("ui_4822b9848d"), subtitle: copy("ui_436c0d359e"), icon: "topics" },
  settings: { title: copy("ui_cbe8870c04"), subtitle: copy("ui_e14561a437"), icon: "settings" },
} as const);
type Page = keyof ReturnType<typeof pageDefinitions>;

function App() {

  const app = useAraBrief(services);
  return <LanguageProvider preference={app.settings?.uiLanguage ?? "auto"}><AppView app={app}/></LanguageProvider>;
}
function AppView({ app }: { app: ReturnType<typeof useAraBrief> }) {
  const copy = useCopy();
  const pages = pageDefinitions(copy);
  const [page, setPage] = useState<Page>("dashboard");
  const heading = useRef<HTMLHeadingElement>(null);
  function navigate(next: Page) { setPage(next); requestAnimationFrame(() => heading.current?.focus()); }
  return <div className="app-shell">
    <a href="#main-content" className="skip-link">{copy("ui_0df8acbe18")}</a>
    <aside className="sidebar">
      <div className="brand"><img src={logo} alt="AraBrief" width="208" height="56"/><small>{copy("ui_551a20611b")}</small></div>
      <p className="nav-caption">{copy("ui_2aa4bc2e68")}</p>
      <nav aria-label={copy("ui_fcc3d9141f")}>{(Object.keys(pages) as Page[]).map(key => <button key={key} aria-current={page===key?"page":undefined} className={`nav-item ${page===key?"selected":""}`} onClick={()=>navigate(key)}><Icon name={pages[key].icon}/><span>{pages[key].title}</span>{page===key && <span className="nav-indicator"/>}</button>)}</nav>
      <div className="sidebar-bottom"><div className="brand-bars" aria-hidden="true"><i/><i/><i/></div><p>{copy("ui_4339d4622d")}<br/><strong>{copy("ui_fc61746eca")}</strong></p><div className="sidebar-status"><span className={`status-dot ${app.settings?"":"status-off"}`}/>{app.loading?copy("ui_c1994a4f99"):app.settings?copy("ui_8bc3cd34c6"):copy("ui_8ac350b6f0")}</div><small>{copy("ui_21f5ad0f5b")}</small></div>
    </aside>
    <div className="workspace">
      <header className="topbar"><span className="topbar-label">{copy("ui_2f67dddf36")}</span>{app.busy ? <button className="text-button danger-text" disabled={app.stopping} onClick={()=>void app.stop()}><Icon name="stop" size={14}/>{app.stopping?copy("ui_c01c73065d"):app.scheduledActive?copy("ui_c21620fb29"):copy("ui_965a7eb80f")}</button> : <span className="topbar-status"><span className="status-dot status-off"/>{copy("ui_8fb3cbc2d7")}</span>}</header>
      <main id="main-content" className="main-content">
        <div className="page-heading"><div><h1 ref={heading} tabIndex={-1}>{pages[page].title}</h1><p>{pages[page].subtitle}</p></div><span className="page-index" aria-hidden="true">{page==="dashboard"?"01":page==="topics"?"02":"03"} / ARA</span></div>
        <div hidden={page!=="dashboard"}><Dashboard app={app} navigate={navigate}/></div>
        <div hidden={page!=="topics"}><Interests app={app}/></div>
        <div hidden={page!=="settings"}><Settings app={app} credentials={apiCredentials}/></div>
        <footer className="page-footer"><span>AraBrief <span className="footer-dot">/</span> {copy("ui_b13c58d4c3")}</span><span>{copy("ui_1ad90c7808")}</span></footer>
      </main>
    </div>
    <ToastViewport/>
  </div>;
}
export default App;
