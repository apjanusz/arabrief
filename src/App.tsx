import { useRef, useState } from "react";
import "./App.css";
import { audioSessionReader } from "./platform/tauri/audio";
import { audioPlayer } from "./platform/tauri/player";
import { settingsRepository, apiCredentials } from "./platform/tauri/settings";
import { newsProvider } from "./platform/tauri/news";
import { contentGenerator } from "./platform/tauri/content";
import { speechProvider } from "./platform/tauri/speech";
import { useAraBrief } from "./app/useAraBrief";
import { Dashboard } from "./app/views/Dashboard";
import { Interests } from "./app/views/Interests";
import { Settings } from "./app/views/Settings";
import { Icon } from "./app/components/Icon";
import logo from "./assets/brand/AraBrief-logo-ciemne.svg";
import { ToastViewport } from "./app/notifications/ToastViewport";

const services = { settings: settingsRepository, audio: audioSessionReader, player: audioPlayer, news: newsProvider, content: contentGenerator, speech: speechProvider };
const pages = {
  dashboard: { title: "Twoje studio", subtitle: "Muzyka zostaje. Świat dołącza.", icon: "radio" },
  topics: { title: "Zainteresowania", subtitle: "Wiadomości, które mają dla Ciebie znaczenie.", icon: "topics" },
  settings: { title: "Ustawienia", subtitle: "Dopasuj AraBrief do swojego rytmu.", icon: "settings" },
} as const;
type Page = keyof typeof pages;

function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const heading = useRef<HTMLHeadingElement>(null);
  const app = useAraBrief(services);
  function navigate(next: Page) { setPage(next); requestAnimationFrame(() => heading.current?.focus()); }
  return <div className="app-shell">
    <a href="#main-content" className="skip-link">Przejdź do treści</a>
    <aside className="sidebar">
      <div className="brand"><img src={logo} alt="AraBrief" width="208" height="56"/><small>Twój świat. Na głos.</small></div>
      <p className="nav-caption">NA TWOICH FALACH</p>
      <nav aria-label="Główna nawigacja">{(Object.keys(pages) as Page[]).map(key => <button key={key} aria-current={page===key?"page":undefined} className={`nav-item ${page===key?"selected":""}`} onClick={()=>navigate(key)}><Icon name={pages[key].icon}/><span>{pages[key].title}</span>{page===key && <span className="nav-indicator"/>}</button>)}</nav>
      <div className="sidebar-bottom"><div className="brand-bars" aria-hidden="true"><i/><i/><i/></div><p>Mała przerwa na świat.<br/><strong>Bez przerwy w muzyce.</strong></p><div className="sidebar-status"><span className={`status-dot ${app.settings?"":"status-off"}`}/>{app.loading?"Łączenie…":app.settings?"Aplikacja połączona":"Brak połączenia"}</div><small>AraBrief dla Windows</small></div>
    </aside>
    <div className="workspace">
      <header className="topbar"><span className="topbar-label">OSOBISTE STUDIO INFORMACJI</span>{app.busy ? <button className="text-button danger-text" disabled={app.stopping} onClick={()=>void app.stop()}><Icon name="stop" size={14}/>{app.stopping?"Zatrzymywanie…":app.scheduledActive?"Zatrzymaj harmonogram":"Zatrzymaj audycję"}</button> : <span className="topbar-status"><span className="status-dot status-off"/>Harmonogram zatrzymany</span>}</header>
      <main id="main-content" className="main-content">
        <div className="page-heading"><div><h1 ref={heading} tabIndex={-1}>{pages[page].title}</h1><p>{pages[page].subtitle}</p></div><span className="page-index" aria-hidden="true">{page==="dashboard"?"01":page==="topics"?"02":"03"} / ARA</span></div>
        <div hidden={page!=="dashboard"}><Dashboard app={app} navigate={navigate}/></div>
        <div hidden={page!=="topics"}><Interests app={app}/></div>
        <div hidden={page!=="settings"}><Settings app={app} credentials={apiCredentials}/></div>
        <footer className="page-footer"><span>AraBrief <span className="footer-dot">/</span> Wiadomości w Twoim rytmie.</span><span>Tworzone przez AI. Wybrane przez Ciebie.</span></footer>
      </main>
    </div>
    <ToastViewport/>
  </div>;
}
export default App;
