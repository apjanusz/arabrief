export interface PlaybackStatus {
  phase: "idle" | "preparing" | "ducking" | "playing" | "paused" | "restoring" | "ended" | "stopped" | "error";
  playbackId: number;
  volume: number;
  outputDeviceId: string | null;
  error: string | null;
}
export interface PlayRequest {
  resourceId: string;
  musicSessionId?: string;
  outputDeviceId?: string;
  playbackKey?: string;
}
export interface AudioPlayer {
  /** Resolves after playback (actual queue exhaustion) and any restoration. */
  play(request: PlayRequest): Promise<PlaybackStatus>;
  stop(): Promise<void>;
  /** Acknowledged after the native player applies pause/resume and music fades. */
  setPaused(paused: boolean, playbackKey: string): Promise<PlaybackStatus>;
  getStatus(): Promise<PlaybackStatus>;
  setVolume(volume: number): Promise<void>;
}
