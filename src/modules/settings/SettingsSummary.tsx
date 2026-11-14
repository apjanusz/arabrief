import { useEffect, useState } from "react";
import type { AppSettings } from "../../domain/models";
import type { SettingsRepository } from "./SettingsRepository";
import type { AudioPlayer } from "../audio/AudioPlayer";
import type { CredentialStore } from "./CredentialStore";

export function SettingsSummary({ repository, credentials, player, selected, disabled }: {
  repository: SettingsRepository;
  credentials: CredentialStore;
  player: AudioPlayer;
  selected: { id: string; name: string } | null;
  disabled: boolean;
}) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let mounted = true;
    void repository.load().then(value => { if (mounted) setSettings(value); }).catch(() => {});
    return () => { mounted = false; };
  }, [repository]);
  async function run(action: () => Promise<string>) {
    setBusy(true);
    try { setMessage(await action()); } catch (cause) { setMessage(String(cause)); } finally { setBusy(false); }
  }
  return (
    <details className="mt-6 border-t border-slate-700 pt-5">
      <summary className="cursor-pointer font-medium">Zapisane ustawienia i klucz API</summary>
      {settings && <p className="mt-3 text-sm text-slate-400">Źródło: {settings.preferredAudioSource?.name ?? "Nie wybrano"}. Interwał: {settings.intervalMs / 60000} min. Muzyka: {Math.round(settings.duckVolume * 100)}% oryginału. Fade: {settings.fadeOutMs / 1000}/{settings.fadeInMs / 1000} s. Audycja: {settings.broadcastMinDurationMs / 60000}–{settings.broadcastMaxDurationMs / 60000} min. Głos: {settings.openAiVoice}. Autostart: {settings.autoStart ? "zapisany jako włączony" : "wyłączony"} (harmonogram nie jest jeszcze podłączony).</p>}
      <button type="button" disabled={busy || disabled || !selected || !settings} className="mt-3 rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => {
        const current = await repository.load();
        const next = { ...current, preferredAudioSource: selected, narratorVolume: (await player.getStatus()).volume };
        await repository.save(next); setSettings(next); return "Zapisano wybrane źródło i głośność lektora.";
      })}>Zapisz źródło i głośność lektora</button>
      <p className="mt-4 text-sm text-slate-400">Klucz wpiszesz w natywnym oknie Windows. „Sprawdź klucz” wysyła do OpenAI żądanie metadanych modeli, bez generowania treści. Test nie potwierdza dostępnych środków ani uprawnień do generowania.</p>
      <div className="mt-3 flex flex-wrap gap-3">
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => await credentials.save() ? "Klucz zapisany w Windows Credential Manager." : "Anulowano zapis klucza.")}>Zapisz klucz</button>
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => {
          const result = await credentials.check();
          const listed = (value: boolean | null) => value === null ? "nie ustalono" : value ? "widoczny na liście" : "nie znaleziono na liście";
          return `${result.message} Model tekstowy ${result.textModel}: ${listed(result.textModelListed)}. Model Realtime: ${listed(result.realtimeModelListed)}.`;
        })}>Sprawdź klucz</button>
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => { await credentials.delete(); return "Usunięto klucz z magazynu."; })}>Usuń klucz</button>
      </div>
      <p role="status" className="mt-3 text-sm text-teal-300">{message}</p>
    </details>
  );
}
