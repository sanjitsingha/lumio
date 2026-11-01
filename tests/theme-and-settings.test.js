import { describe, expect, it } from "vitest";
import { ACCENTS, contrastText, resolveAccent, resolveTheme } from "../src/utils/theme";
import { deepMerge } from "../src/stores/settingsStore";

describe("theme resolution", () => {
  it("follows the system only in system mode", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("uses the light/dark variant of the palette accent", () => {
    expect(resolveAccent("blue", false)).toBe("#007AFF");
    expect(resolveAccent("blue", true)).toBe("#0A84FF");
    expect(resolveAccent("green", true)).toBe(ACCENTS.green[1]);
  });

  it("system accent falls back to blue when the OS has none", () => {
    expect(resolveAccent("system", false, "#3689e6")).toBe("#3689e6");
    expect(resolveAccent("system", true, null)).toBe("#0A84FF");
  });

  it("picks readable text on accents", () => {
    expect(contrastText("#007AFF")).toBe("#ffffff");
    expect(contrastText("#FFCC00")).toBe("#1d1d1f");
  });
});

describe("settings patches", () => {
  it("deep-merges nested objects and replaces arrays", () => {
    const base = { capsLock: { size: 40, opacity: 0.9 }, spotlight: { exclusions: ["a", "b"] } };
    const merged = deepMerge(base, { capsLock: { size: 64 }, spotlight: { exclusions: ["c"] } });
    expect(merged).toEqual({ capsLock: { size: 64, opacity: 0.9 }, spotlight: { exclusions: ["c"] } });
    expect(base.capsLock.size).toBe(40);
  });
});
