import { useEffect, useState } from "react";
import { api } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";
import { contrastText, resolveAccent, resolveTheme } from "../utils/theme";

const darkQuery = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;
const motionQuery = typeof window !== "undefined" ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;

/** Tracks the OS appearance (portal on Linux, webview media queries as fallback). */
export function useSystemAppearance() {
  const [system, setSystem] = useState({ dark: null, accent: null, reduceMotion: null });
  const [mediaDark, setMediaDark] = useState(darkQuery?.matches ?? false);
  const [mediaMotion, setMediaMotion] = useState(motionQuery?.matches ?? false);

  useEffect(() => {
    let alive = true;
    api.getSystemAppearance().then((a) => alive && setSystem(a)).catch(() => {});
    const unsubscribe = subscribe(EVENTS.appearanceChanged, (a) => setSystem(a));
    const onDark = (e) => setMediaDark(e.matches);
    const onMotion = (e) => setMediaMotion(e.matches);
    darkQuery?.addEventListener("change", onDark);
    motionQuery?.addEventListener("change", onMotion);
    return () => {
      alive = false;
      unsubscribe();
      darkQuery?.removeEventListener("change", onDark);
      motionQuery?.removeEventListener("change", onMotion);
    };
  }, []);

  return {
    dark: system.dark ?? mediaDark,
    accent: system.accent,
    reduceMotion: system.reduceMotion ?? mediaMotion,
  };
}

/** Applies theme, accent and motion preference to the document. */
export function useAppearance(settings) {
  const system = useSystemAppearance();
  const appearance = settings?.appearance ?? { theme: "system", accent: "blue", motion: "system" };
  const theme = resolveTheme(appearance.theme, system.dark);
  const accent = resolveAccent(appearance.accent, theme === "dark", system.accent);
  const reduceMotion = appearance.motion === "reduced" || (appearance.motion === "system" && system.reduceMotion);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.style.setProperty("--lm-accent", accent);
    root.style.setProperty("--lm-accent-contrast", contrastText(accent));
  }, [theme, accent]);

  return { theme, accent, reduceMotion, systemDark: system.dark, systemAccent: system.accent };
}
