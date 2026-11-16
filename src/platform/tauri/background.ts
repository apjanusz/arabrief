import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { BackgroundLifecycle, BackgroundLaunch } from "../../modules/scheduler/BackgroundLifecycle";

function launchState(value: unknown): BackgroundLaunch {
  if (!value || typeof value !== "object" || !("closing" in value) || typeof value.closing !== "boolean"
    || !("startScheduler" in value) || typeof value.startScheduler !== "boolean"
    || !("recoveryMessage" in value) || (value.recoveryMessage !== null && typeof value.recoveryMessage !== "string")) {
    throw new Error("Nieprawidłowa odpowiedź cyklu życia aplikacji.");
  }
  return { closing: value.closing, startScheduler: value.startScheduler, recoveryMessage: value.recoveryMessage };
}

export const backgroundLifecycle: BackgroundLifecycle = {
  async connect(actions) {
    if (!isTauri()) return null;
    const unlisten: (() => void)[] = [];
    let quitting: Promise<void> | null = null;
    const quit = () => quitting ??= actions.quit().finally(() => invoke("background_stopped"));
    try {
      unlisten.push(await listen<unknown>("background-action", event => {
        if (event.payload === "quit") void quit().catch(console.error);
        else if (event.payload === "toggle" && !quitting) void actions.toggle().catch(console.error);
      }));
      unlisten.push(await listen("background-tick", () => actions.wake()));
      const launch = launchState(await invoke<unknown>("background_connect"));
      if (launch.closing) void quit().catch(console.error);
      return { launch, dispose: () => unlisten.forEach(stop => stop()) };
    } catch (cause) { unlisten.forEach(stop => stop()); throw cause; }
  },
  async update(status) {
    if (typeof status.active !== "boolean" || typeof status.canStart !== "boolean" || (status.active && status.canStart)) {
      throw new Error("Nieprawidłowy stan harmonogramu.");
    }
    if (isTauri()) await invoke("background_status", { status });
  },
};
