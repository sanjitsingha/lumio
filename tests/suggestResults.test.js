import { describe, expect, it } from "vitest";
import { suggestFromResults, suggestFullName } from "../src/utils/suggestResults";

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
  it("leaves names typed from the middle to suggestFullName", () => {
    expect(suggestFromResults("photoshop", [{ id: "top", results: [{ kind: "app", title: "Adobe Photoshop 2027" }] }])).toBeNull();
  });
});

describe("suggestFullName", () => {
  const found = [
    { id: "top", results: [{ kind: "app", title: "Adobe Photoshop 2027" }] },
    { id: "files", results: [{ kind: "file", title: "studio notes.txt" }] },
    { id: "apps", results: [{ kind: "app", title: "Visual Studio Code" }] },
  ];

  it("gives the whole name when the search starts a later word", () => {
    expect(suggestFullName("photoshop", found)).toBe("Adobe Photoshop 2027");
    expect(suggestFullName("photo", found)).toBe("Adobe Photoshop 2027");
    expect(suggestFullName("Photoshop", found)).toBe("Adobe Photoshop 2027");
    expect(suggestFullName("  photoshop ", found)).toBe("Adobe Photoshop 2027");
  });
  it("takes the words in any order", () => {
    expect(suggestFullName("photoshop adobe", found)).toBe("Adobe Photoshop 2027");
    expect(suggestFullName("2027 photo", found)).toBe("Adobe Photoshop 2027");
  });
  it("names the best application, setting or command, not a file", () => {
    expect(suggestFullName("studio", found)).toBe("Visual Studio Code");
    expect(suggestFullName("code vis", found)).toBe("Visual Studio Code");
  });
  it("leaves names typed in order to suggestFromResults", () => {
    for (const q of ["adobe p", "adobe photoshop 2027", "Adobe"]) expect(suggestFullName(q, found), q).toBeNull();
  });
  it("needs every word to start a word of the name", () => {
    for (const q of ["shop", "photoshop zzz", "p", "", "  "]) expect(suggestFullName(q, found), q).toBeNull();
    expect(suggestFullName("photoshop", undefined)).toBeNull();
  });
});
