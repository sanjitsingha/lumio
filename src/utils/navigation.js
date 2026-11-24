// Keyboard navigation over grouped search results.

/**
 * Flattens sections into `[{ key, result, action, section, sectionIndex, index }]`.
 * The app whose id is `expandedId` is followed by one item per action of its
 * own (`action` set, `result` is the app).
 */
export function flatten(sections, expandedId = null) {
  const items = [];
  sections.forEach((section, sectionIndex) => {
    section.results.forEach((result) => {
      const push = (key, action) =>
        items.push({ key, result, action, section: section.id, sectionIndex, index: items.length });
      push(result.id, null);
      if (result.id === expandedId) result.actions?.forEach((action) => push(action.id, action));
    });
  });
  return items;
}

/** What each kind of result is called ("Application" for Google Chrome). */
export const KIND_LABELS = {
  app: "Application",
  action: "Action",
  setting: "Setting",
  folder: "Folder",
  file: "File",
  web: "Web Search",
  calculator: "Calculator",
  conversion: "Currency",
  measurement: "Measurement",
  time: "Time",
};

/** What an item of `flatten` is; an app's own actions are "Action". */
export function kindLabel(item) {
  if (!item) return null;
  return item.action ? KIND_LABELS.action : (KIND_LABELS[item.result.kind] ?? null);
}

/**
 * The lines of one section as the list shows them: its rows (`[item, index]`
 * pairs), with an "Actions" heading before an app's actions and the
 * section's own heading again after them when more of its results follow.
 */
export function sectionLines(rows, title) {
  const lines = [];
  rows.forEach(([item, index], n) => {
    const previous = n > 0 ? rows[n - 1][0] : null;
    if (item.action && !previous?.action) lines.push({ heading: "Actions", key: `actions:${item.result.id}` });
    else if (!item.action && previous?.action) lines.push({ heading: title, key: `after:${previous.result.id}` });
    lines.push({ item, index });
  });
  return lines;
}

/** The app whose actions are shown while `item` is selected: the app itself, or an action's app. */
export function expansionFor(item) {
  if (!item || !(item.action || item.result.actions?.length)) return null;
  return item.result.id;
}

/**
 * Keyboard selection of `items[index]`: shows that app's actions (and keeps
 * them while one of them is selected), hides any others, and keeps the
 * chosen item selected. Returns the new `{ expanded, selected }`.
 */
export function selectItem(sections, expanded, items, index) {
  const item = items[index];
  const next = item ? expansionFor(item) : expanded;
  if (next === expanded) return { expanded, selected: index };
  return { expanded: next, selected: flatten(sections, next).findIndex((i) => i.key === item.key) };
}

/** Arrow keys: moves within bounds (no wrap-around, like macOS). */
export function moveSelection(count, current, delta) {
  if (count === 0) return -1;
  if (current < 0) return delta > 0 ? 0 : count - 1;
  return Math.min(count - 1, Math.max(0, current + delta));
}

/** Tab / Shift+Tab: first item of the next/previous section (wraps around). */
export function jumpSection(items, current, direction) {
  if (items.length === 0) return -1;
  const starts = [];
  items.forEach((item, i) => {
    if (i === 0 || items[i - 1].sectionIndex !== item.sectionIndex) starts.push(i);
  });
  if (starts.length === 1) return 0;
  const currentSection = current >= 0 ? items[current].sectionIndex : -1;
  const pos = starts.findIndex((s) => items[s].sectionIndex === currentSection);
  if (direction > 0) return starts[(pos + 1) % starts.length];
  // Shift+Tab inside a section first returns to its start
  if (pos >= 0 && current !== starts[pos]) return starts[pos];
  return starts[(pos - 1 + starts.length) % starts.length];
}
