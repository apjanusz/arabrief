import type { AppSettings, BroadcastState } from "../../domain/models";

/** Single broadcast at a time; stop cancels work and awaits volume restoration. */
export interface BroadcastScheduler {
  start(settings: AppSettings): Promise<void>;
  stop(): Promise<void>;
  getState(): BroadcastState;
  subscribe(listener: (state: BroadcastState) => void): () => void;
}
