export interface PlaybackStatus {
  phase: "idle" | "preparing" | "ducking" | "playing" | "restoring" | "ended" | "stopped" | "error";
  playbackId: number;
  volume: number;
  outputDeviceId: string | null;
  error: string | null;
}
export interface PlayRequest {
  resourceId: string;
  musicSessionId?: string;
  outputDeviceId?: string;
}
export interface AudioPlayer {
  /** Resolves after playback (actual queue exhaustion) and any restoration. */
  play(request: PlayRequest): Promise<PlaybackStatus>;
  stop(): Promise<void>;
  getStatus(): Promise<PlaybackStatus>;
  setVolume(volume: number): Promise<void>;
}
