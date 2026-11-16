/** Native lifecycle port; the scheduler stays independent of React and Tauri. */
export interface BackgroundStatus { active: boolean; canStart: boolean }
export interface BackgroundLaunch { closing: boolean; startScheduler: boolean; recoveryMessage: string | null }
export interface BackgroundLifecycle {
  connect(actions: { toggle(): Promise<void>; quit(): Promise<void>; wake(): void }): Promise<{ launch: BackgroundLaunch; dispose(): void } | null>;
  update(status: BackgroundStatus): Promise<void>;
}
