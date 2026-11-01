// Event names shared with `src-tauri/src/events.rs`.
import { listen } from "@tauri-apps/api/event";

export const EVENTS = {
  settingsChanged: "lumio://settings-changed",
  spotlightShown: "lumio://spotlight-shown",
  spotlightHidden: "lumio://spotlight-hidden",
  navigate: "lumio://navigate",
  appearanceChanged: "lumio://appearance-changed",
  updateStatus: "lumio://update-status",
  notice: "lumio://notice",
};

/** Subscribes and returns a synchronous unsubscribe function (safe in effects). */
export function subscribe(name, handler) {
  let unlisten = null;
  let cancelled = false;
  listen(name, (event) => handler(event.payload)).then((fn) => {
    if (cancelled) fn();
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    if (unlisten) unlisten();
  };
}
