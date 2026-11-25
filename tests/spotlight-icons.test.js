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
    const fallbacks = ["web.rs", "apps/mod.rs"].flatMap((file) => names(rust(file), /Symbol \{ name: "([a-z0-9-]+)"/g));
    expect(actions.length).toBeGreaterThan(3);
    expect(fallbacks).toEqual(expect.arrayContaining(["globe", "app-window"]));
    // page("dark-mode", "moon", "Open dark mode settings", …): each settings page's glyph
    const pages = names(rust("system_settings.rs"), /^ {4}page\("[a-z0-9-]+", "([a-z0-9-]+)"/gm);
    expect(pages.length).toBeGreaterThan(90);
    // c("lock-screen", "Lock Screen", …, "lock", Native(lock))
    const commands = names(rust("../platform/windows/system_commands.rs"), /"([a-z0-9-]+)", (?:Open|Program|Keys|Native|Ask)\(/g);
    expect(commands.length).toBeGreaterThan(100);
    const missing = [...new Set([...actions, ...fallbacks, ...commands, ...pages])].filter((name) => !SYMBOLS[name]);
    expect(missing).toEqual([]);
  });

  it("gives every symbol a component", () => {
    for (const [name, Icon] of Object.entries(SYMBOLS)) {
      expect(Icon, name).toBeTruthy();
    }
  });
});
