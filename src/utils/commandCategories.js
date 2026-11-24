/** The dropdown's value for "no filter". */
export const ALL_CATEGORIES = "all";

/**
 * Settings pages by the start of their place in the settings app, named like
 * the System commands' categories of that platform (`category` from the
 * backend) so one category holds both. The first match wins.
 */
const PAGE_CATEGORIES = {
  linux: [
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
  ],
  windows: [
    ["Personalization › Start", "Taskbar & Start"],
    ["Personalization › Taskbar", "Taskbar & Start"],
    ["Personalization", "Appearance"],
    ["System › Display", "Display"],
    ["System › Sound", "Sound"],
    ["System › Notifications", "Notifications"],
    ["System › Power", "Power"],
    ["System › Storage", "Cleanup"],
    ["System › Multitasking", "Windows"],
    ["System › Clipboard", "Clipboard"],
    ["System › Remote Desktop", "Network"],
    ["System", "System"],
    ["Windows Update", "Updates"],
    ["Bluetooth & devices › Mouse", "Mouse"],
    ["Bluetooth & devices › Touchpad", "Touchpad"],
    ["Bluetooth & devices › Typing", "Keyboard"],
    ["Bluetooth & devices ›", "Devices"],
    ["Bluetooth & devices", "Bluetooth"],
    ["Network & internet", "Network"],
    ["Apps", "Applications"],
    ["Accounts", "Accounts"],
    ["Time & language › Date", "Date & Time"],
    ["Time & language", "Language & Region"],
    ["Gaming", "Gaming"],
    ["Accessibility › Mouse pointer", "Mouse"],
    ["Accessibility", "Accessibility"],
    ["Privacy & security", "Privacy & Security"],
  ],
};

/** Pages whose place in the app is not what people look for them under. */
const PAGE_EXCEPTIONS = {
  linux: { "setting:brightness": "Display", "setting:updates": "Updates" },
  windows: { "setting:keyboard-layout": "Keyboard" },
};

/** The category of a command on `os`; the built-in actions describe themselves, so they are General. */
export function commandCategory(command, os) {
  if (command.kind === "app") return "Applications";
  if (command.kind === "setting") {
    const exception = PAGE_EXCEPTIONS[os]?.[command.id];
    if (exception) return exception;
    const place = command.subtitle.replace(/^.*?› /, "");
    return (PAGE_CATEGORIES[os] ?? []).find(([start]) => place.startsWith(start))?.[1] ?? "General";
  }
  return command.category ?? "General";
}

/** The dropdown's options: All categories, then the ones this computer has commands in, A to Z. */
export function categoryOptions(commands, os) {
  const names = [...new Set(commands.map((c) => commandCategory(c, os)))].sort((a, b) => a.localeCompare(b));
  return [{ value: ALL_CATEGORIES, label: "All categories" }, ...names.map((name) => ({ value: name, label: name }))];
}
