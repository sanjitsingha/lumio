import { describe, expect, it } from "vitest";
import { ALL_CATEGORIES, categoryOptions, commandCategory } from "../src/utils/commandCategories";

const page = (id, place) => ({ id: `setting:${id}`, kind: "setting", subtitle: `System Settings › ${place}` });
const action = (subtitle) => ({ id: "action:x", kind: "action", subtitle });

describe("commandCategory", () => {
  it("uses a system command's own category", () => {
    expect(commandCategory(action("Media"))).toBe("Media");
    expect(commandCategory(action("Date & Time"))).toBe("Date & Time");
  });

  it("puts the built-in actions under General", () => {
    expect(commandCategory(action("Quick calculations"))).toBe("General");
  });

  it("files every System Settings page where people look for it", () => {
    const places = {
      "Applications › Defaults": "Applications",
      Bluetooth: "Bluetooth",
      "Date & Time": "Date & Time",
      "Desktop › Appearance": "Appearance",
      "Desktop › Dock & Panel": "Dock",
      "Desktop › Multitasking": "Windows",
      "Desktop › Text": "Text",
      "Desktop › Wallpaper": "Wallpaper",
      Displays: "Display",
      "Displays › Filters": "Color Filters",
      "Displays › Night Light": "Night Light",
      "Keyboard › Layout": "Keyboard",
      "Language & Region": "Language & Region",
      "Mouse & Touchpad › Gestures": "Touchpad",
      "Mouse & Touchpad › Mouse": "Mouse",
      "Mouse & Touchpad › Pointing": "Pointer",
      "Mouse & Touchpad › Touchpad": "Touchpad",
      Network: "Network",
      Notifications: "Notifications",
      "Online Accounts": "Accounts",
      "Pen & Drawing": "Devices",
      Power: "Power",
      Printers: "Devices",
      "Security & Privacy › Firewall": "Privacy",
      "Security & Privacy › Housekeeping": "Cleanup",
      Sharing: "Network",
      "Sound › Output": "Sound",
      "System › Hardware": "About This Computer",
      "Universal Access": "Accessibility",
      "User Accounts": "Accounts",
    };
    for (const [place, category] of Object.entries(places)) expect(commandCategory(page("x", place)), place).toBe(category);
  });

  it("files brightness and updates by what they are, not where their page is", () => {
    expect(commandCategory(page("brightness", "Power"))).toBe("Display");
    expect(commandCategory(page("updates", "System › Operating System"))).toBe("Updates");
  });
});

describe("categoryOptions", () => {
  it("starts with All categories, then lists only what exists, A to Z", () => {
    const options = categoryOptions([action("Sound"), action("Media"), action("Sound"), page("x", "Bluetooth")]);
    expect(options[0]).toEqual({ value: ALL_CATEGORIES, label: "All categories" });
    expect(options.slice(1).map((o) => o.value)).toEqual(["Bluetooth", "Media", "Sound"]);
  });
});
