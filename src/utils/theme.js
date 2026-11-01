// Accent palette (light, dark) and theme resolution.

export const ACCENTS = {
  blue: ["#007AFF", "#0A84FF"],
  purple: ["#AF52DE", "#BF5AF2"],
  pink: ["#FF2D55", "#FF375F"],
  red: ["#FF3B30", "#FF453A"],
  orange: ["#FF9500", "#FF9F0A"],
  yellow: ["#FFCC00", "#FFD60A"],
  green: ["#34C759", "#30D158"],
  graphite: ["#8E8E93", "#98989D"],
};

export const ACCENT_ORDER = ["system", "blue", "purple", "pink", "red", "orange", "yellow", "green", "graphite"];

/** Relative luminance check so text on light accents (yellow) stays readable. */
export function contrastText(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex || "");
  if (!m) return "#ffffff";
  const [r, g, b] = m.slice(1).map((h) => {
    const c = parseInt(h, 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return luminance > 0.55 ? "#1d1d1f" : "#ffffff";
}

export function resolveTheme(preference, systemDark) {
  if (preference === "dark") return "dark";
  if (preference === "light") return "light";
  return systemDark ? "dark" : "light";
}

export function resolveAccent(accent, dark, systemAccent) {
  if (accent === "system") return systemAccent || ACCENTS.blue[dark ? 1 : 0];
  const pair = ACCENTS[accent] || ACCENTS.blue;
  return pair[dark ? 1 : 0];
}
