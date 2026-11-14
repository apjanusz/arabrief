import type { NewsItem } from "../../domain/models";

export interface NewsProvider {
  fetch(request: { sourceUrls: string[]; since: string }, signal?: AbortSignal): Promise<NewsItem[]>;
}
