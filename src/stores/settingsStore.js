// Single source of truth for settings inside a window. The backend owns the
// data; this store mirrors it and applies optimistic updates.
import { api } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";

let state = { settings: null, loaded: false, notices: [], error: null };
const listeners = new Set();
let initPromise = null;

function set(patch) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

export function deepMerge(target, patch) {
  if (!patch || typeof patch !== "object" || Array.isArray(patch)) return patch;
  const out = { ...(target || {}) };
  for (const [k, v] of Object.entries(patch)) {
    out[k] = v && typeof v === "object" && !Array.isArray(v) ? deepMerge(out[k], v) : v;
  }
  return out;
}

export const settingsStore = {
  subscribe(listener) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },

  getState() {
    return state;
  },

  /** Loads settings once per window and follows changes made elsewhere. */
  init({ takeNotices = false } = {}) {
    if (!initPromise) {
      initPromise = api
        .getSettings(takeNotices)
        .then((payload) => {
          set({ settings: payload.settings, notices: payload.notices, loaded: true, error: null });
          subscribe(EVENTS.settingsChanged, (settings) => set({ settings }));
        })
        .catch((error) => {
          initPromise = null;
          set({ error });
        });
    }
    return initPromise;
  },

  /** Optimistically applies `patch`; reverts and rethrows if the backend refuses. */
  async update(patch) {
    const previous = state.settings;
    set({ settings: deepMerge(previous, patch) });
    try {
      const next = await api.updateSettings(patch);
      set({ settings: next });
      return next;
    } catch (error) {
      set({ settings: previous });
      throw error;
    }
  },

  replace(settings) {
    set({ settings });
  },

  dismissNotices() {
    set({ notices: [] });
  },
};
