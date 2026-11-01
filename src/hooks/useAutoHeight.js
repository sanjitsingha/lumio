import { useEffect, useRef } from "react";
import { api } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";

/**
 * Keeps the native window exactly as tall as `ref`'s content plus `margin`
 * (the transparent area that holds the panel's shadow).
 */
export function useAutoHeight(ref, margin) {
  const applied = useRef("");
  useEffect(() => {
    const el = ref.current;
    if (!el) return undefined;
    const apply = () => {
      const width = window.innerWidth;
      if (width < 100) return; // hidden window: no layout yet, retried when shown
      const height = Math.ceil(el.getBoundingClientRect().height + margin * 2);
      const key = `${height}@${width}`;
      if (key === applied.current) return;
      // the backend converts CSS pixels to window pixels using the viewport width
      api
        .setSpotlightHeight(height, width)
        .then(() => (applied.current = key))
        .catch(() => {});
    };
    const observer = new ResizeObserver(apply);
    observer.observe(el);
    window.addEventListener("resize", apply);
    const offShown = subscribe(EVENTS.spotlightShown, () => {
      applied.current = "";
      requestAnimationFrame(apply);
    });
    apply();
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", apply);
      offShown();
    };
  }, [ref, margin]);
}
