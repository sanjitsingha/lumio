import { useEffect, useRef, useState } from "react";
import { api } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";

/**
 * Keeps the native window as tall as `el` plus `extra` (the margins and
 * borders around it) and returns `el`'s height, which the panel animates to.
 * The window grows at once but shrinks only after `shrinkDelay` ms, so the
 * panel's shrinking animation is not cut off. Takes the element itself (from
 * a callback ref) so that a remounted panel is observed again.
 */
export function useAutoHeight(el, extra, shrinkDelay = 0) {
  const applied = useRef("");
  const requested = useRef(0);
  const [height, setHeight] = useState(null);
  useEffect(() => {
    if (!el) return undefined;
    let shrinkTimer;
    const send = (target, width) => {
      requested.current = target;
      const key = `${target}@${width}`;
      if (key === applied.current) return;
      // the backend converts CSS pixels to window pixels using the viewport width
      api
        .setSpotlightHeight(target, width)
        .then(() => (applied.current = key))
        .catch(() => {});
    };
    const apply = () => {
      // a replaced panel reports 0 once it leaves the page
      if (!el.isConnected) return;
      // layout height: unlike getBoundingClientRect it ignores the entry animation's scale
      const natural = el.offsetHeight;
      setHeight(natural);
      const width = window.innerWidth;
      if (width < 100) return; // hidden window: no layout yet, retried when shown
      const target = natural + extra;
      clearTimeout(shrinkTimer);
      if (target < requested.current && shrinkDelay > 0) shrinkTimer = setTimeout(() => send(target, width), shrinkDelay);
      else send(target, width);
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
      clearTimeout(shrinkTimer);
      observer.disconnect();
      window.removeEventListener("resize", apply);
      offShown();
    };
  }, [el, extra, shrinkDelay]);
  return height;
}
