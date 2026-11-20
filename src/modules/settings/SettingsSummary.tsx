import { useCopy } from "../../app/i18n";
import { useEffect, useState } from "react";
import { notifications } from "../../app/notifications/notifications";
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
  const copy = useCopy();
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let mounted = true;
    void repository.load().then(value => { if (mounted) setSettings(value); }).catch(() => {});
    return () => { mounted = false; };
  }, [repository]);
  async function run(action: () => Promise<string | void>) {
    setBusy(true);
    try { const message = await action(); if(message) notifications.success(message); } catch (cause) { notifications.error(String(cause)); } finally { setBusy(false); }
  }
  return (
    <details className="mt-6 border-t border-slate-700 pt-5">
      <summary className="cursor-pointer font-medium"> {copy("ui_aa3f3f2672")} </summary>
      {settings && <p className="mt-3 text-sm text-slate-400"> {copy("ui_fe2374a15d")} {settings.preferredAudioSource?.name ?? copy('ui_f06747dcec')} {copy("ui_c7278b3025")} {settings.intervalMs / 60000}  {copy("ui_80c573f25a")} {Math.round(settings.duckVolume * 100)} {copy("ui_24cfebfe32")} {settings.fadeOutMs / 1000}/{settings.fadeInMs / 1000}  {copy("ui_96ed917c24")} {settings.broadcastMinDurationMs / 60000}–{settings.broadcastMaxDurationMs / 60000}  {copy("ui_a6e8d5b0a3")} {settings.openAiVoice} {copy("ui_ff5a6fffa7")} {settings.autoStart ? copy("ui_617c9a0674") : copy("ui_9a0ea78dd2")}.</p>}
      <button type="button" disabled={busy || disabled || !selected || !settings} className="mt-3 rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => {
        const current = await repository.load();
        const next = { ...current, preferredAudioSource: selected, narratorVolume: (await player.getStatus()).volume };
        await repository.save(next); setSettings(next); return copy("ui_75faab64a1");
      })}> {copy("ui_12030cd2e3")} </button>
      <p className="mt-4 text-sm text-slate-400"> {copy("ui_3a4e22a087")} </p>
      <div className="mt-3 flex flex-wrap gap-3">
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => { if(await credentials.save()) notifications.success(copy('ui_a67cbbd819')); else notifications.info(copy('ui_37a6d8a58c')); })}> {copy("ui_d2cb009435")} </button>
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => {
          const result = await credentials.check();
          const listed = (value: boolean | null) => value === null ? copy('ui_140efd626b') : value ? copy("ui_189fdbd771") : copy("ui_190fb46214");
          const message = `${copy(result.message)} ${copy("textModelLabel")} ${result.textModel}: ${listed(result.textModelListed)}. ${copy("realtimeModelLabel")}: ${listed(result.realtimeModelListed)}.`;
          if(result.status === "accepted") notifications.success(message);
          else if(result.status === "rejected") notifications.error(message);
          else notifications.warning(message);
        })}> {copy("ui_b5047ed4cb")} </button>
        <button disabled={busy} className="rounded-lg border border-slate-600 px-4 py-2 disabled:opacity-50" onClick={() => void run(async () => { await credentials.delete(); return copy("ui_ee6c9ef93b"); })}> {copy("ui_d447927cb2")} </button>
      </div>
    </details>
  );
}
