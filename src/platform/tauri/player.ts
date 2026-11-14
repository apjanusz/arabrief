import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AudioPlayer, PlaybackStatus } from "../../modules/audio/AudioPlayer";

export const audioPlayer: AudioPlayer = {
  play: request => invoke<PlaybackStatus>("audio_play", { request }),
  stop: () => invoke("audio_stop"),
  getStatus: () => isTauri() ? invoke<PlaybackStatus>("audio_get_status") : Promise.reject(new Error("Odtwarzacz wymaga aplikacji desktopowej.")),
  setVolume: volume => invoke("audio_set_volume", { volume }),
};
