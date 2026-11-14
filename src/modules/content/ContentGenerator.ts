import type { BroadcastScript, NewsItem } from "../../domain/models";

export interface ContentGenerator {
  generate(items: readonly NewsItem[], language: string, signal?: AbortSignal): Promise<BroadcastScript>;
}
