import { useEffect } from "react";
import { MotionConfig } from "framer-motion";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import SpotlightPage from "./pages/Spotlight";
import SettingsPage from "./pages/Settings";
import { useSettingsState } from "./hooks/useSettings";
import { useAppearance } from "./hooks/useAppearance";
import { settingsStore } from "./stores/settingsStore";

const windowLabel = getCurrentWebviewWindow().label;
document.documentElement.dataset.window = windowLabel;

export default function App() {
  const { settings, error } = useSettingsState();
  const appearance = useAppearance(settings);

  useEffect(() => {
    settingsStore.init({ takeNotices: windowLabel === "settings" });
  }, []);

  if (error && windowLabel === "settings") {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-[13px] text-secondary">
        Lumio could not load its settings: {error.message || String(error)}
      </div>
    );
  }
  if (!settings) return null;

  return (
    <MotionConfig reducedMotion={appearance.reduceMotion ? "always" : "never"}>
      {windowLabel === "spotlight" ? <SpotlightPage /> : <SettingsPage appearance={appearance} />}
    </MotionConfig>
  );
}
