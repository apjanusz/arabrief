export interface AudioSession {
  id: string;
  sessionIdentifier: string;
  deviceId: string;
  pid: number;
  processName: string | null;
  state: "active" | "inactive";
  displayName: string;
  volume: number;
}

/** Native owns snapshots and recovery. No pause/resume operations are exposed. */
export interface AudioController {
  listSessions(): Promise<AudioSession[]>;
  duck(sessionId: string): Promise<{ restorationToken: string; outcome: "completed" | "manual_override" }>;
  play(resourceId: string, signal?: AbortSignal): Promise<void>;
  stop(): Promise<void>;
  restore(restorationToken: string): Promise<"completed" | "manual_override">;
}
