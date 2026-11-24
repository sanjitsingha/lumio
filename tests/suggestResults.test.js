import { describe, expect, it } from "vitest";
import { suggestFromResults } from "../src/utils/suggestResults";

const sections = [
  { id: "top", results: [{ kind: "app", title: "Calculator" }] },
  { id: "files", results: [{ kind: "file", title: "calcium.pdf" }] },
  { id: "settings", results: [{ kind: "setting", title: "Toggle Bluetooth" }, { kind: "action", title: "Open Terminal" }] },
];

describe("suggestFromResults", () => {
  it("finishes the name of an application, setting or command", () => {
    expect(suggestFromResults("calc", sections)).toBe("calculator");
    expect(suggestFromResults("tog", sections)).toBe("toggle bluetooth");
    expect(suggestFromResults("open t", sections)).toBe("open terminal");
  });
  it("follows the case being typed", () => {
    expect(suggestFromResults("Calc", sections)).toBe("Calculator");
    expect(suggestFromResults("CALC", sections)).toBe("CALCULATOR");
  });
  it("keeps what was typed, spaces included", () => {
    expect(suggestFromResults("  calc", sections)).toBe("  calculator");
  });
  it("suggests nothing for files, short searches, whole names or no match", () => {
    for (const q of ["calci", "c", "calculator", "zzz", ""]) expect(suggestFromResults(q, sections), q).toBeNull();
    expect(suggestFromResults("calc", undefined)).toBeNull();
  });
});
