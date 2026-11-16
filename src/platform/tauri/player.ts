import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AudioPlayer, PlaybackStatus } from "../../modules/audio/AudioPlayer";

const validPlaybackKey = (key: unknown): key is string => typeof key === "string" && key.length > 0 && new TextEncoder().encode(key).length <= 128;

export const audioPlayer: AudioPlayer = {
  play: request => request.playbackKey !== undefined && !validPlaybackKey(request.playbackKey)
    ? Promise.reject(new Error("Nieprawidłowy identyfikator odtwarzania."))
    : invoke<PlaybackStatus>("audio_play", { request }),
  stop: () => invoke("audio_stop"),
  setPaused: (paused, playbackKey) => typeof paused !== "boolean" || !validPlaybackKey(playbackKey)
    ? Promise.reject(new Error("Nieprawidłowe żądanie pauzy."))
    : invoke<PlaybackStatus>("audio_set_paused", { paused, playbackKey }),
  getStatus: () => isTauri() ? invoke<PlaybackStatus>("audio_get_status") : Promise.reject(new Error("Odtwarzacz wymaga aplikacji desktopowej.")),
  setVolume: volume => invoke("audio_set_volume", { volume }),
};
