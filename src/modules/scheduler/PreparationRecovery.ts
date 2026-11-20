import type { BroadcastScript, SpeechAsset } from "../../domain/models";
import type { NewsResult } from "../news/NewsProvider";

export function newsCooldown(news: NewsResult, now: number): string | null {
  const pending = news.topics?.filter(topic => Number.isFinite(topic.nextFetchAt) && topic.nextFetchAt > now) ?? [];
  const limited = pending.some(topic => /HTTP (429|503)\b/.test(topic.error ?? ""));
  if (!pending.length || (!limited && (pending.length !== news.topics.length || pending.some(topic=>topic.error)))) return null;
  return `news_wait:${limited ? "limited" : "normal"}:${Math.min(...pending.map(topic=>topic.nextFetchAt))}`;
}

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
