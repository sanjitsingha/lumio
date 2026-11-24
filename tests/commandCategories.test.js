import { describe, expect, it } from "vitest";
import { ALL_CATEGORIES, categoryOptions, commandCategory } from "../src/utils/commandCategories";

const linuxPage = (id, place) => ({ id: `setting:${id}`, kind: "setting", subtitle: `System Settings › ${place}` });
const windowsPage = (id, place) => ({ id: `setting:${id}`, kind: "setting", subtitle: `Settings › ${place}` });
const command = (category) => ({ id: "action:x", kind: "action", subtitle: "What it does", category });

describe("commandCategory", () => {
  it("uses a system command's own category, on either platform", () => {
    expect(commandCategory(command("Media"), "linux")).toBe("Media");
    expect(commandCategory(command("Taskbar & Start"), "windows")).toBe("Taskbar & Start");
  });

  it("puts the built-in actions, which have no category, under General", () => {
    expect(commandCategory(command(undefined), "linux")).toBe("General");
    expect(commandCategory(command(undefined), "windows")).toBe("General");
  });

  it("puts installed apps under Applications", () => {
    expect(commandCategory({ id: "app:x", kind: "app", subtitle: "" }, "windows")).toBe("Applications");
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
    for (const [place, category] of Object.entries(places)) {
      expect(commandCategory(linuxPage("x", place), "linux"), place).toBe(category);
    }
  });

  it("files every Windows Settings page under the category of its System commands", () => {
    const places = {
      "Personalization › Colors": "Appearance",
      "Personalization › Background": "Appearance",
      "Personalization › Start": "Taskbar & Start",
      "Personalization › Taskbar": "Taskbar & Start",
      "System › Display": "Display",
      "System › Display › Night light": "Display",
      "System › Sound › Volume mixer": "Sound",
      "System › Notifications": "Notifications",
      "System › Power & battery": "Power",
      "System › Storage › Storage Sense": "Cleanup",
      "System › Multitasking": "Windows",
      "System › Clipboard": "Clipboard",
      "System › Remote Desktop": "Network",
      "System › About": "System",
      "System › Recovery": "System",
      "Windows Update › Update history": "Updates",
      "Bluetooth & devices": "Bluetooth",
      "Bluetooth & devices › Printers & scanners": "Devices",
      "Bluetooth & devices › Cameras": "Devices",
      "Bluetooth & devices › Mouse": "Mouse",
      "Bluetooth & devices › Touchpad": "Touchpad",
      "Bluetooth & devices › Typing": "Keyboard",
      "Network & internet › Wi-Fi": "Network",
      "Apps › Default apps": "Applications",
      "Accounts › Your info": "Accounts",
      "Time & language › Date & time": "Date & Time",
      "Time & language › Language & region": "Language & Region",
      "Time & language › Speech": "Language & Region",
      "Gaming › Game Mode": "Gaming",
      "Accessibility › Mouse pointer and touch": "Mouse",
      "Accessibility › Magnifier": "Accessibility",
      "Privacy & security › Location": "Privacy & Security",
    };
    for (const [place, category] of Object.entries(places)) {
      expect(commandCategory(windowsPage("x", place), "windows"), place).toBe(category);
    }
  });

  it("files a few pages by what they are, not where they are", () => {
    expect(commandCategory(linuxPage("brightness", "Power"), "linux")).toBe("Display");
    expect(commandCategory(linuxPage("updates", "System › Operating System"), "linux")).toBe("Updates");
    expect(commandCategory(windowsPage("keyboard-layout", "Time & language › Language & region"), "windows")).toBe("Keyboard");
  });
});

describe("categoryOptions", () => {
  it("starts with All categories, then lists only what exists, A to Z", () => {
    const options = categoryOptions([command("Sound"), command("Media"), command("Sound"), linuxPage("x", "Bluetooth")], "linux");
    expect(options[0]).toEqual({ value: ALL_CATEGORIES, label: "All categories" });
    expect(options.slice(1).map((o) => o.value)).toEqual(["Bluetooth", "Media", "Sound"]);
  });

  it("merges a Windows page and System commands of the same category", () => {
    const options = categoryOptions([command("Sound"), windowsPage("sound", "System › Sound")], "windows");
    expect(options.slice(1).map((o) => o.value)).toEqual(["Sound"]);
  });
});
