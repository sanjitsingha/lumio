/** The dropdown's value for "no filter". */
export const ALL_CATEGORIES = "all";

/** The categories of Linux's System commands: their subtitles in `platform/linux/system_commands.rs`. */
const SYSTEM_CATEGORIES = new Set([
  "Session",
  "Power",
  "Sound",
  "Media",
  "Display",
  "Night Light",
  "Color Filters",
  "Appearance",
  "Accent Color",
  "Wallpaper",
  "Text",
  "Pointer",
  "Dock",
  "Panel",
  "Panel Menus",
  "Clock",
  "Windows",
  "Workspaces",
  "Hot Corners",
  "Window Behavior",
  "Network",
  "Bluetooth",
  "Notifications",
  "Keyboard",
  "Mouse",
  "Touchpad",
  "Accessibility",
  "Privacy",
  "Cleanup",
  "Updates",
  "Date & Time",
  "Files",
  "Folders",
  "Screenshots",
]);

/** System Settings pages, by the start of their place in the app; the first match wins. */
const PAGE_CATEGORIES = [
  ["Applications", "Applications"],
  ["Bluetooth", "Bluetooth"],
  ["Date & Time", "Date & Time"],
  ["Desktop › Appearance", "Appearance"],
  ["Desktop › Dock", "Dock"],
  ["Desktop › Multitasking", "Windows"],
  ["Desktop › Text", "Text"],
  ["Desktop › Wallpaper", "Wallpaper"],
  ["Displays › Filters", "Color Filters"],
  ["Displays › Night Light", "Night Light"],
  ["Displays", "Display"],
  ["Keyboard", "Keyboard"],
  ["Language", "Language & Region"],
  ["Mouse & Touchpad › Mouse", "Mouse"],
  ["Mouse & Touchpad › Pointing", "Pointer"],
  ["Mouse & Touchpad", "Touchpad"],
  ["Network", "Network"],
  ["Sharing", "Network"],
  ["Notifications", "Notifications"],
  ["Online Accounts", "Accounts"],
  ["User Accounts", "Accounts"],
  ["Pen", "Devices"],
  ["Printers", "Devices"],
  ["Power", "Power"],
  ["Security & Privacy › Housekeeping", "Cleanup"],
  ["Security & Privacy", "Privacy"],
  ["Sound", "Sound"],
  ["System", "About This Computer"],
  ["Universal Access", "Accessibility"],
];

/** Pages whose place in the app is not what people look for them under. */
const PAGE_EXCEPTIONS = { "setting:brightness": "Display", "setting:updates": "Updates" };

/** The category of a Linux command; the built-in actions describe themselves, so they are General. */
export function commandCategory(command) {
  if (command.kind === "app") return "Applications";
  if (command.kind === "setting") {
    if (PAGE_EXCEPTIONS[command.id]) return PAGE_EXCEPTIONS[command.id];
    const place = command.subtitle.replace(/^.*?› /, "");
    return PAGE_CATEGORIES.find(([start]) => place.startsWith(start))?.[1] ?? "General";
  }
  return SYSTEM_CATEGORIES.has(command.subtitle) ? command.subtitle : "General";
}

/** The dropdown's options: All categories, then the ones this computer has commands in, A to Z. */
export function categoryOptions(commands) {
  const names = [...new Set(commands.map(commandCategory))].sort((a, b) => a.localeCompare(b));
  return [{ value: ALL_CATEGORIES, label: "All categories" }, ...names.map((name) => ({ value: name, label: name }))];
}
