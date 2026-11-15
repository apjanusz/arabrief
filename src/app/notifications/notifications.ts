export type NotificationKind = "success" | "error" | "warning" | "info";
export interface NotificationOptions {
  title?: string;
  /** Stable identity for background faults; do not use for independent user actions. */
  key?: string;
}
export interface Notification extends NotificationOptions {
  id: number;
  kind: NotificationKind;
  message: string;
  duration: number;
  closing: boolean;
}

/** UI event store, independent of React, Tauri and native services. */
export function createNotificationStore() {
  let sequence = 0;
  let items: readonly Notification[] = [];
  const listeners = new Set<() => void>();
  const publish = (next: readonly Notification[]) => {
    items = next;
    listeners.forEach(listener => listener());
  };
  function push(kind: NotificationKind, message: string, options: NotificationOptions = {}) {
    const previous = options.key ? items.find(item => item.key === options.key && !item.closing) : undefined;
    if (previous?.kind === kind && previous.message === message && previous.title === options.title) return previous.id;
    const item: Notification = { ...options, id: previous?.id ?? ++sequence, kind, message,
      duration: kind === "success" || kind === "info" ? 5000 : 0, closing: false };
    publish(previous ? items.map(existing => existing.id === previous.id ? item : existing) : [...items, item]);
    return item.id;
  }
  return {
    subscribe(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; },
    getSnapshot: () => items,
    success: (message: string, options?: NotificationOptions) => push("success", message, options),
    error: (message: string, options?: NotificationOptions) => push("error", message, options),
    warning: (message: string, options?: NotificationOptions) => push("warning", message, options),
    info: (message: string, options?: NotificationOptions) => push("info", message, options),
    dismiss(id: number) {
      if (items.some(item => item.id === id && !item.closing)) publish(items.map(item => item.id === id ? { ...item, closing: true } : item));
    },
    remove(id: number) { if (items.some(item => item.id === id)) publish(items.filter(item => item.id !== id)); },
  };
}

export const notifications = createNotificationStore();
