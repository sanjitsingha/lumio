/**
 * Copies `text` during a user action (e.g. Enter). Falls back to a hidden
 * textarea where the async Clipboard API is missing or refused.
 */
export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // older WebKitGTK, or no permission: use the legacy path below
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  const copied = document.execCommand("copy");
  area.remove();
  if (!copied) throw new Error("The result could not be copied.");
}
