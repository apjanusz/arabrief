import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { Icon } from "../components/Icon";
import { notifications } from "./notifications";
import type { Notification } from "./notifications";
import "./toasts.css";
import { useCopy } from "../i18n";

const labels = { success: "ui_3d9d03d320", error: "ui_df557b52a8", warning: "ui_0256098037", info: "ui_8fcbd2b189" };
const icons = { success: "check", error: "error", warning: "warning", info: "info" } as const;

function Toast({ item, paused }: { item: Notification; paused: boolean }) {
  const copy = useCopy();
  const remaining = useRef(item.duration);
  useEffect(() => { remaining.current = item.duration; }, [item]);
  useEffect(() => {
    if (item.closing) {
      const timer = setTimeout(() => notifications.remove(item.id), 180);
      return () => clearTimeout(timer);
    }
    if (paused || !item.duration) return;
    const started = performance.now();
    const timer = setTimeout(() => notifications.dismiss(item.id), remaining.current);
    return () => { clearTimeout(timer); remaining.current = Math.max(0, remaining.current - (performance.now() - started)); };
  }, [item, paused]);

  function close(button: HTMLButtonElement) {
    // No focus stealing on arrival; return keyboard users to the interface on dismissal.
    if (document.activeElement === button) {
      const next = [...document.querySelectorAll<HTMLButtonElement>('.toast:not(.toast-closing) .toast-close')].find(other => other !== button);
      (next ?? document.querySelector<HTMLElement>(".page-heading h1"))?.focus({ preventScroll: true });
    }
    notifications.dismiss(item.id);
  }
  return <li className={`toast toast-${item.kind} ${item.closing ? "toast-closing" : ""}`}>
    <span className="toast-icon"><Icon name={icons[item.kind]} size={19}/></span>
    <div className="toast-content" role={item.kind === "error" ? "alert" : "status"} aria-atomic="true">
      <strong>{item.title ? copy(item.title) : copy(labels[item.kind] as Parameters<typeof copy>[0])}</strong>
      <p>{copy(item.message)}</p>
    </div>
    <button type="button" className="icon-button toast-close" disabled={item.closing} aria-label={copy("ui_9be23b3d7f").replace('{0}', copy(item.title ?? labels[item.kind]))} onClick={event => close(event.currentTarget)} onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); close(event.currentTarget); } }}><Icon name="close" size={16}/></button>
  </li>;
}

/** Mounted once at the application root, outside the layout and hidden views. */
export function ToastViewport() {
  const copy = useCopy();
  const items = useSyncExternalStore(notifications.subscribe, notifications.getSnapshot);
  const region = useRef<HTMLElement>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [inactive, setInactive] = useState(() => document.hidden || !document.hasFocus());
  useEffect(() => {
    const visibility = () => setInactive(document.hidden || !document.hasFocus());
    const shortcut = (event: KeyboardEvent) => {
      if (event.key === "F8" && region.current?.querySelector("button:not(:disabled)")) {
        event.preventDefault(); region.current.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
      }
    };
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("blur", visibility);
    window.addEventListener("focus", visibility);
    window.addEventListener("keydown", shortcut);
    return () => { document.removeEventListener("visibilitychange", visibility); window.removeEventListener("blur", visibility); window.removeEventListener("focus", visibility); window.removeEventListener("keydown", shortcut); };
  }, []);
  useEffect(() => { if (!items.length) { setHovered(false); setFocused(false); } }, [items.length]);
  return createPortal(<section ref={region} className="toast-region" aria-label={copy("ui_44a8673551")} onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)} onFocusCapture={() => setFocused(true)} onBlurCapture={event => { if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false); }}>
    <ol className="toast-stack">{items.map(item => <Toast key={item.id} item={item} paused={hovered || focused || inactive}/>)}</ol>
  </section>, document.body);
}
