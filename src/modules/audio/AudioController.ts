export interface AudioSession {
  id: string;
  displayName: string;
  volume: number;
}

/** Native owns snapshots and recovery. No pause/resume operations are exposed. */
export interface AudioController {
  listSessions(): Promise<AudioSession[]>;
  duck(sessionIds: string[], volume: number): Promise<{ restorationToken: string }>;
  play(resourceId: string, signal?: AbortSignal): Promise<void>;
  stop(): Promise<void>;
  restore(restorationToken: string): Promise<void>;
}
