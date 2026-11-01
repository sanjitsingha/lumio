import { describe, expect, it } from "vitest";
import {
  deletePreviousSegment,
  formatCanonical,
  keyFromCode,
  liveModifierKeys,
  shortcutFromEvent,
  shortcutKeys,
} from "../src/utils/keys";

const event = (code, mods = {}) => ({
  code,
  ctrlKey: !!mods.ctrl,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
  metaKey: !!mods.super,
});

describe("keyFromCode", () => {
  it("maps letters, digits and function keys", () => {
    expect(keyFromCode("KeyK")).toBe("K");
    expect(keyFromCode("Digit7")).toBe("7");
    expect(keyFromCode("Numpad3")).toBe("3");
    expect(keyFromCode("F13")).toBe("F13");
    expect(keyFromCode("F25")).toBeNull();
  });

  it("maps named keys to the backend's canonical names", () => {
    expect(keyFromCode("Space")).toBe("Space");
    expect(keyFromCode("ArrowUp")).toBe("Up");
    expect(keyFromCode("NumpadEnter")).toBe("Enter");
    expect(keyFromCode("Backquote")).toBe("Backquote");
    expect(keyFromCode("IntlRo")).toBeNull();
  });
});

describe("shortcutFromEvent", () => {
  it("produces canonical text with modifiers in a fixed order", () => {
    expect(shortcutFromEvent(event("Space", { alt: true })).text).toBe("Alt+Space");
    expect(shortcutFromEvent(event("KeyK", { shift: true, ctrl: true, super: true })).text).toBe("Ctrl+Shift+Super+K");
  });

  it("reports modifiers while no key is pressed yet", () => {
    const combo = shortcutFromEvent(event("ControlLeft", { ctrl: true }));
    expect(combo.key).toBeNull();
    expect(combo.text).toBeNull();
    expect(combo.mods.ctrl).toBe(true);
  });

  it("matches formatCanonical", () => {
    expect(formatCanonical({ ctrl: true, alt: true }, "Delete")).toBe("Ctrl+Alt+Delete");
  });
});

describe("shortcutKeys", () => {
  it("uses platform names for the Super key", () => {
    expect(shortcutKeys("Super+L", "windows")).toEqual(["Win", "L"]);
    expect(shortcutKeys("Super+L", "linux")).toEqual(["Super", "L"]);
  });

  it("renders symbols for punctuation and arrows", () => {
    expect(shortcutKeys("Ctrl+Comma")).toEqual(["Ctrl", ","]);
    expect(shortcutKeys("Alt+Up")).toEqual(["Alt", "↑"]);
    expect(shortcutKeys(null)).toEqual([]);
  });

  it("lists held modifiers", () => {
    expect(liveModifierKeys({ ctrl: true, alt: false, shift: true, super: false })).toEqual(["Ctrl", "Shift"]);
  });
});

describe("deletePreviousSegment (Ctrl+Backspace)", () => {
  it("removes the previous word", () => {
    expect(deletePreviousSegment("open visual studio")).toEqual({ value: "open visual ", caret: 12 });
  });

  it("removes trailing separators together with the word", () => {
    expect(deletePreviousSegment("open visual   ")).toEqual({ value: "open ", caret: 5 });
  });

  it("treats path separators as segment boundaries", () => {
    expect(deletePreviousSegment("~/Documents/report")).toEqual({ value: "~/Documents/", caret: 12 });
    expect(deletePreviousSegment("C:\\Users\\me")).toEqual({ value: "C:\\Users\\", caret: 9 });
  });

  it("works in the middle of the text", () => {
    expect(deletePreviousSegment("alpha beta gamma", 10)).toEqual({ value: "alpha  gamma", caret: 6 });
  });

  it("does nothing at the start", () => {
    expect(deletePreviousSegment("abc", 0)).toEqual({ value: "abc", caret: 0 });
  });
});
