import type { BroadcastScript, SpeechAsset } from "../../domain/models";

/** Recoverable absence of safe news, distinct from a failed service. */
export class NoUsableMaterial extends Error {
  constructor(message = "Nie ma nowych, bezpiecznych materiałów. Spróbujemy w kolejnym cyklu.") {
    super(message); this.name = "NoUsableMaterial";
  }
}
export interface PreparationRecovery {
  restore(): Promise<{ script: BroadcastScript; audio: SpeechAsset | null } | null>;
  claim(broadcastId: string): Promise<void>;
  complete(broadcastId: string): Promise<void>;
}
