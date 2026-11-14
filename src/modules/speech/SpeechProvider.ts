import type { BroadcastScript, SpeechAsset } from "../../domain/models";

export interface SpeechProvider {
  synthesize(script: BroadcastScript, signal?: AbortSignal): Promise<SpeechAsset>;
  release(asset: SpeechAsset): Promise<void>;
}
