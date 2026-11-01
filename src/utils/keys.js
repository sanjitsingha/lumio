// Keyboard helpers shared by the shortcut recorder and Spotlight.
// Shortcut text uses the backend's canonical form: `Ctrl+Alt+Shift+Super+Key`.

const NAMED_CODES = {
  Space: "Space",
  Enter: "Enter",
  NumpadEnter: "Enter",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Escape: "Escape",
  Comma: "Comma",
  Period: "Period",
  Slash: "Slash",
  Semicolon: "Semicolon",
  Quote: "Quote",
  BracketLeft: "BracketLeft",
  BracketRight: "BracketRight",
  Backslash: "Backslash",
  Minus: "Minus",
  Equal: "Equal",
  Backquote: "Backquote",
};

const MODIFIER_CODES = new Set([
  "ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight",
  "MetaLeft", "MetaRight", "OSLeft", "OSRight", "AltGraph",
]);

/** Maps a `KeyboardEvent.code` to a canonical key name, or null if unsupported. */
export function keyFromCode(code) {
  if (!code) return null;
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1];
  m = /^(?:Digit|Numpad)([0-9])$/.exec(code);
  if (m) return m[1];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  return NAMED_CODES[code] ?? null;
}

export function isModifierCode(code) {
  return MODIFIER_CODES.has(code);
}

/**
 * Reads the combination currently held in a keydown event.
 * Returns `{ mods, key, text }`; `key` is null while only modifiers are held.
 */
export function shortcutFromEvent(event) {
  const mods = {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    super: event.metaKey,
  };
  const key = isModifierCode(event.code) ? null : keyFromCode(event.code);
  return { mods, key, text: key ? formatCanonical(mods, key) : null };
}

export function formatCanonical(mods, key) {
  const parts = [];
  if (mods.ctrl) parts.push("Ctrl");
  if (mods.alt) parts.push("Alt");
  if (mods.shift) parts.push("Shift");
  if (mods.super) parts.push("Super");
  parts.push(key);
  return parts.join("+");
}

const DISPLAY = {
  Up: "↑",
  Down: "↓",
  Left: "←",
  Right: "→",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Minus: "−",
  Equal: "=",
  Backquote: "`",
  PageUp: "Page Up",
  PageDown: "Page Down",
  Escape: "Esc",
  Backspace: "⌫",
};

/** Splits canonical text into keycap labels, using platform names for Super. */
export function shortcutKeys(text, os = "linux") {
  if (!text) return [];
  return text.split("+").map((part) => {
    if (part === "Super") return os === "windows" ? "Win" : "Super";
    return DISPLAY[part] ?? part;
  });
}

export function liveModifierKeys(mods, os = "linux") {
  return shortcutKeys(
    [mods.ctrl && "Ctrl", mods.alt && "Alt", mods.shift && "Shift", mods.super && "Super"].filter(Boolean).join("+"),
    os,
  );
}

const SEGMENT_BREAK = /[\s/\\.\-_:,;]/;

/**
 * Ctrl+Backspace: removes the segment before the caret (a word, or a path
 * component), including the separators directly before the caret.
 */
export function deletePreviousSegment(value, caret = value.length) {
  if (caret <= 0) return { value, caret: 0 };
  let start = caret;
  while (start > 0 && SEGMENT_BREAK.test(value[start - 1])) start--;
  while (start > 0 && !SEGMENT_BREAK.test(value[start - 1])) start--;
  return { value: value.slice(0, start) + value.slice(caret), caret: start };
}
