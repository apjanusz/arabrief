import type { AppSettings, BroadcastState } from "../../domain/models";
import type { PreparedBroadcast } from "./ManualBroadcastRunner";

/** Single broadcast at a time; stop cancels work and awaits volume restoration. */
export interface BroadcastScheduler {
  start(settings: AppSettings, ready?: PreparedBroadcast): Promise<void>;
  stop(): Promise<void>;
  getState(): BroadcastState;
  subscribe(listener: (state: BroadcastState) => void): () => void;
  /** Reconcile monotonic time and wall clock after Windows resumes. */
  wake(): void;
}
