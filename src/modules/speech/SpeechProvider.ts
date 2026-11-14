import type { SpeechAsset } from "../../domain/models";

export interface SpeechProvider {
  synthesize(broadcastId: string, voice: string, jobId: string): Promise<SpeechAsset>;
  cancel(jobId: string): Promise<boolean>;
}
