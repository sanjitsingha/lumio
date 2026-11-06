import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { SYMBOLS } from "../src/components/spotlight/ResultIcon";

// vitest runs from the project root
const rust = (file) => readFileSync(resolve(process.cwd(), "src-tauri/src/search", file), "utf8");
const names = (source, pattern) => [...source.matchAll(pattern)].map((m) => m[1]);

describe("Spotlight icons", () => {
  it("draws every icon the backend names", () => {
    // QuickAction { icon: "calculator", … } and fallbacks such as ResultIcon::Symbol { name: "globe".into() }
    const actions = names(rust("actions.rs"), /icon: "([a-z0-9-]+)"/g);
    const fallbacks = ["system_settings.rs", "web.rs", "apps/mod.rs"].flatMap((file) =>
      names(rust(file), /Symbol \{ name: "([a-z0-9-]+)"/g),
    );
    expect(actions.length).toBeGreaterThan(3);
    expect(fallbacks).toEqual(expect.arrayContaining(["settings", "globe", "app-window"]));
    const missing = [...new Set([...actions, ...fallbacks])].filter((name) => !SYMBOLS[name]);
    expect(missing).toEqual([]);
  });

  it("gives every symbol a component and a colour", () => {
    for (const [name, { Icon, tint }] of Object.entries(SYMBOLS)) {
      expect(Icon, name).toBeTruthy();
      expect(tint, name).toMatch(/^#[0-9A-F]{6}$/i);
    }
  });
});
