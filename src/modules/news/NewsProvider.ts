import type { NewsItem } from "../../domain/models";

export interface NewsResult {
  items: NewsItem[];
  topics: { topic: string; nextFetchAt: number; error: string | null }[];
  fetchedTopics: string[];
}
export interface NewsProvider {
  fetch(request: { topics: string[] }): Promise<NewsResult>;
  /** Call only after an actual broadcast used these articles, never after a preview. */
  markUsed(articleIds: string[], broadcastId: string): Promise<void>;
}
