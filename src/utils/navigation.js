// Keyboard navigation over grouped search results.

/** Flattens sections into `[{ result, section, sectionIndex, index }]`. */
export function flatten(sections) {
  const items = [];
  sections.forEach((section, sectionIndex) => {
    section.results.forEach((result) => {
      items.push({ result, section: section.id, sectionIndex, index: items.length });
    });
  });
  return items;
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
