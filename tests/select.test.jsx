import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Select } from "../src/components/common/controls";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const OPTIONS = [
  { value: "small", label: "Small" },
  { value: "medium", label: "Medium" },
  { value: "large", label: "Large" },
];

let host;
let root;

function mount(props = {}) {
  const onChange = vi.fn();
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  act(() => root.render(<Select label="Text size" value="medium" options={OPTIONS} onChange={onChange} {...props} />));
  return onChange;
}

const button = () => host.querySelector('[role="combobox"]');
const menu = () => document.querySelector('[role="listbox"]');
const option = (name) => [...document.querySelectorAll('[role="option"]')].find((o) => o.textContent === name);
const click = (el) => act(() => el.dispatchEvent(new MouseEvent("click", { bubbles: true })));
const press = (key) =>
  act(() => button().dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true })));

afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("Select", () => {
  it("shows the chosen option and opens a menu on click", () => {
    mount();
    expect(button().textContent).toBe("Medium");
    expect(menu()).toBeNull();
    click(button());
    expect(button().getAttribute("aria-expanded")).toBe("true");
    expect(option("Medium").getAttribute("aria-selected")).toBe("true");
  });

  it("chooses a clicked option and closes", () => {
    const onChange = mount();
    click(button());
    click(option("Large"));
    expect(onChange).toHaveBeenCalledExactlyOnceWith("large");
    expect(menu()).toBeNull();
  });

  it("does not report the option that is already chosen", () => {
    const onChange = mount();
    click(button());
    click(option("Medium"));
    expect(onChange).not.toHaveBeenCalled();
    expect(menu()).toBeNull();
  });

  it("works from the keyboard, starting at the chosen option", () => {
    const onChange = mount();
    press("ArrowDown");
    expect(button().getAttribute("aria-activedescendant")).toBe(option("Medium").id);
    press("ArrowDown");
    press("ArrowDown");
    expect(button().getAttribute("aria-activedescendant")).toBe(option("Large").id);
    press("Home");
    press("Enter");
    expect(onChange).toHaveBeenCalledExactlyOnceWith("small");
    expect(menu()).toBeNull();
  });

  it("closes without choosing on Escape or a press outside", () => {
    const onChange = mount();
    press("Enter");
    press("Escape");
    expect(menu()).toBeNull();
    click(button());
    act(() => document.body.dispatchEvent(new Event("pointerdown", { bubbles: true })));
    expect(menu()).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("stays shut when locked or disabled", () => {
    mount({ locked: true });
    expect(button().disabled).toBe(true);
    expect(button().textContent).toBe("Medium");
    expect(button().parentElement.className).not.toContain("opacity");
    expect(button().className).not.toContain("opacity-40");
    press("Enter");
    expect(menu()).toBeNull();
  });
});
