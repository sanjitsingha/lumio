import { describe, expect, it } from "vitest";
import { expansionFor, flatten, jumpSection, moveSelection, selectItem } from "../src/utils/navigation";

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

describe("app actions", () => {
  const firefox = {
    id: "app:ff",
    actions: [
      { id: "app:ff#new", title: "New Window" },
      { id: "app:ff#private", title: "New Private Window" },
    ],
  };
  const withActions = [
    { id: "top", results: [firefox] },
    { id: "files", results: [{ id: "file:a" }, { id: "file:b" }] },
  ];

  it("lists the expanded app's actions right after it, in its section", () => {
    const items = flatten(withActions, "app:ff");
    expect(items.map((i) => i.key)).toEqual(["app:ff", "app:ff#new", "app:ff#private", "file:a", "file:b"]);
    expect(items[2]).toMatchObject({ action: { title: "New Private Window" }, sectionIndex: 0, index: 2 });
    expect(items[2].result.id).toBe("app:ff");
    expect(flatten(withActions).map((i) => i.key)).toEqual(["app:ff", "file:a", "file:b"]);
  });

  it("keeps the actions open while the app or one of its actions is selected", () => {
    const items = flatten(withActions, "app:ff");
    expect(expansionFor(items[0])).toBe("app:ff");
    expect(expansionFor(items[2])).toBe("app:ff");
    expect(expansionFor(items[3])).toBeNull();
    expect(expansionFor(undefined)).toBeNull();
  });

  it("moving past the actions hides them and keeps the new item selected", () => {
    const open = flatten(withActions, "app:ff");
    expect(selectItem(withActions, "app:ff", open, 2)).toEqual({ expanded: "app:ff", selected: 2 });
    expect(selectItem(withActions, "app:ff", open, 3)).toEqual({ expanded: null, selected: 1 });
    // back up onto the app: its actions appear below it and it stays selected
    expect(selectItem(withActions, null, flatten(withActions), 0)).toEqual({ expanded: "app:ff", selected: 0 });
    expect(selectItem(withActions, null, flatten(withActions), -1)).toEqual({ expanded: null, selected: -1 });
  });

  it("Tab skips over action rows to the next section", () => {
    const items = flatten(withActions, "app:ff");
    expect(jumpSection(items, 1, 1)).toBe(3);
    expect(jumpSection(items, 3, -1)).toBe(0);
  });
});
