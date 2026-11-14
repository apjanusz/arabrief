import type { BroadcastScript } from "../../domain/models";

export interface ContentGenerator {
  generate(articleIds: readonly string[]): Promise<BroadcastScript>;
}
