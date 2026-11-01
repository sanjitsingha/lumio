import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { KeyCaps } from "../common/controls";
import { liveModifierKeys, shortcutFromEvent, shortcutKeys } from "../../utils/keys";

/**
 * Captures one key combination from the keyboard. Calls `onCapture(text)`
 * with canonical text (e.g. `Ctrl+Alt+K`) or `onCancel()` on a bare Escape.
 */
export function ShortcutRecorder({ os, onCapture, onCancel, busy }) {
  const [mods, setMods] = useState({ ctrl: false, alt: false, shift: false, super: false });

  useEffect(() => {
    const onKeyDown = (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (busy) return;
      const combo = shortcutFromEvent(event);
      setMods(combo.mods);
      if (!combo.key) return;
      const bare = !combo.mods.ctrl && !combo.mods.alt && !combo.mods.shift && !combo.mods.super;
      if (combo.key === "Escape" && bare) {
        onCancel();
        return;
      }
      onCapture(combo.text);
    };
    const onKeyUp = (event) => {
      event.preventDefault();
      setMods(shortcutFromEvent(event).mods);
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
  }, [onCapture, onCancel, busy]);

  const live = liveModifierKeys(mods, os);
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.97 }}
      animate={{ opacity: 1, scale: 1 }}
      className="flex h-[26px] min-w-[210px] items-center justify-center gap-2 rounded-[7px] bg-accent/10 px-3 ring-2 ring-accent/60"
      aria-live="polite"
    >
      {live.length ? (
        <KeyCaps keys={[...live, "…"]} size="sm" />
      ) : (
        <motion.span
          animate={{ opacity: [0.55, 1, 0.55] }}
          transition={{ duration: 1.6, repeat: Infinity }}
          className="text-[12px] font-medium text-accent"
        >
          Press your desired shortcut…
        </motion.span>
      )}
    </motion.div>
  );
}

export function CapturedShortcut({ text, os }) {
  return <KeyCaps keys={shortcutKeys(text, os)} size="sm" />;
}
