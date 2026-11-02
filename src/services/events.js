// Event names shared with `src-tauri/src/events.rs`.
import { listen } from "@tauri-apps/api/event";

export const EVENTS = {
  settingsChanged: "lummo://settings-changed",
  spotlightShown: "lummo://spotlight-shown",
  spotlightHiding: "lummo://spotlight-hiding",
  spotlightHidden: "lummo://spotlight-hidden",
  navigate: "lummo://navigate",
  appearanceChanged: "lummo://appearance-changed",
  updateStatus: "lummo://update-status",
  notice: "lummo://notice",
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
