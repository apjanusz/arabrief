import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PreparationRecovery } from "../../modules/scheduler/PreparationRecovery";
export const preparationRecovery: PreparationRecovery = {
  restore: () => isTauri() ? invoke("broadcast_restore") : Promise.resolve(null),
  claim: broadcastId => invoke("broadcast_claim", { broadcastId }),
  complete: broadcastId => invoke("broadcast_complete", { broadcastId }),
};
