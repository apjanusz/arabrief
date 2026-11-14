import { invoke, isTauri } from "@tauri-apps/api/core";
import type { NewsProvider, NewsResult } from "../../modules/news/NewsProvider";

export const newsProvider: NewsProvider = {
  fetch: ({ topics }) => isTauri() ? invoke<NewsResult>("news_fetch", { topics }) : Promise.reject(new Error("Pobieranie RSS wymaga aplikacji desktopowej.")),
  markUsed: (articleIds, broadcastId) => invoke("news_mark_used", { articleIds, broadcastId }),
};
