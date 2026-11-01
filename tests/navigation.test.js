import { describe, expect, it } from "vitest";
import { flatten, jumpSection, moveSelection } from "../src/utils/navigation";

const sections = [
  { id: "top", results: [{ id: "a" }] },
  { id: "apps", results: [{ id: "b" }, { id: "c" }] },
  { id: "files", results: [{ id: "d" }, { id: "e" }, { id: "f" }] },
];

describe("flatten", () => {
  it("assigns global indices and section indices", () => {
    const items = flatten(sections);
    expect(items.map((i) => i.result.id)).toEqual(["a", "b", "c", "d", "e", "f"]);
    expect(items[3]).toMatchObject({ section: "files", sectionIndex: 2, index: 3 });
  });
});

describe("moveSelection", () => {
  it("moves within bounds without wrapping", () => {
    expect(moveSelection(6, 0, 1)).toBe(1);
    expect(moveSelection(6, 5, 1)).toBe(5);
    expect(moveSelection(6, 0, -1)).toBe(0);
    expect(moveSelection(0, 0, 1)).toBe(-1);
  });
});

describe("jumpSection (Tab / Shift+Tab)", () => {
  const items = flatten(sections);

  it("Tab moves to the first item of the next section and wraps", () => {
    expect(jumpSection(items, 0, 1)).toBe(1);
    expect(jumpSection(items, 2, 1)).toBe(3);
    expect(jumpSection(items, 4, 1)).toBe(0);
  });

  it("Shift+Tab returns to the section start first, then the previous section", () => {
    expect(jumpSection(items, 4, -1)).toBe(3);
    expect(jumpSection(items, 3, -1)).toBe(1);
    expect(jumpSection(items, 0, -1)).toBe(3);
  });

  it("handles a single section and no results", () => {
    expect(jumpSection(flatten([sections[2]]), 2, 1)).toBe(0);
    expect(jumpSection([], 0, 1)).toBe(-1);
  });
});
