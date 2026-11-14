import "./App.css";

import { AudioSessionPicker } from "./modules/audio/AudioSessionPicker";
import { audioSessionReader } from "./platform/tauri/audio";
import { audioPlayer } from "./platform/tauri/player";
import { settingsRepository, apiCredentials } from "./platform/tauri/settings";

function App() {
  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col justify-center px-6 py-12">
      <header>
        <p className="text-sm font-medium uppercase tracking-widest text-teal-400">Twoja przerwa na informacje</p>
        <h1 className="mt-3 text-5xl font-semibold tracking-tight">AraBrief</h1>
        <p className="mt-5 max-w-xl text-lg leading-relaxed text-slate-300">
          Krótkie audycje informacyjne przygotowane przez AI, w rytmie Twojego dnia.
          Muzyka gra dalej — podczas audycji zostanie tylko ściszona.
        </p>
      </header>
      <section aria-labelledby="status-heading" className="mt-10 rounded-2xl border border-slate-700 bg-slate-900 p-6">
        <h2 id="status-heading" className="text-lg font-medium">Stan aplikacji</h2>
        <p role="status" className="mt-3 flex items-center gap-3 text-teal-300">
          <span aria-hidden="true" className="h-2 w-2 rounded-full bg-teal-400" />
          Wybór źródła muzyki dostępny
        </p>
        <p className="mt-3 leading-relaxed text-slate-400">
          Ręczne ściszanie muzyki jest dostępne. Harmonogram, źródła wiadomości,
          generowanie treści i synteza mowy nie są jeszcze podłączone.
        </p>
      </section>
      <AudioSessionPicker reader={audioSessionReader} player={audioPlayer} repository={settingsRepository} credentials={apiCredentials} />
    </main>
  );
}

export default App;
