import { useCallback, useSyncExternalStore } from "react";
import { settingsStore } from "../stores/settingsStore";
import { toast } from "../stores/toastStore";
import { errorMessage } from "../services/tauri";

export function useSettingsState() {
  return useSyncExternalStore(settingsStore.subscribe, settingsStore.getState);
}

export function useSettings() {
  return useSettingsState().settings;
}

/** Returns `update(patch)` which reports failures as a toast. */
export function useUpdateSettings() {
  return useCallback(async (patch) => {
    try {
      return await settingsStore.update(patch);
    } catch (error) {
      toast.error(errorMessage(error));
      return null;
    }
  }, []);
}
