import { useEffect, useRef } from "react";
import { ActionRow, CalculatorRow, ResultRow } from "./ResultRow";
import { sectionLines } from "../../utils/navigation";

function Heading({ children }) {
  return <div className="px-[14px] pt-3 pb-1.5 text-[12px] leading-[16px] text-secondary">{children}</div>;
}

/**
 * Grouped results with section headers; scrolls the selection into view.
 * Renders `items` (see `flatten`), which include the expanded app's actions.
 */
export function ResultList({ sections, items, selected, onSelect, onOpen }) {
  const listRef = useRef(null);
  const rowRefs = useRef([]);

  // Scrolls the list only: scrollIntoView would also scroll the page, which
  // pushes the panel out of the window while the window is still growing.
  useEffect(() => {
    const list = listRef.current;
    const row = rowRefs.current[selected];
    if (!list || !row) return;
    const listBox = list.getBoundingClientRect();
    const rowBox = row.getBoundingClientRect();
    if (rowBox.top < listBox.top) list.scrollTop -= listBox.top - rowBox.top;
    else if (rowBox.bottom > listBox.bottom) list.scrollTop += rowBox.bottom - listBox.bottom;
  }, [selected]);

  // The cap uses the window's largest height, 632/752 of its fixed width (see
  // window_manager.rs), not 100vh: on Windows the window is sized to this
  // content, so a 100vh cap would shrink along with it. 198 px = 2 × 36 margin
  // + 2 × 8 panel padding + search bar, footer and borders (91) + 19 spare.
  const groups = [];
  items.forEach((item, i) => {
    if (groups.at(-1)?.sectionIndex !== item.sectionIndex) groups.push({ sectionIndex: item.sectionIndex, rows: [] });
    groups.at(-1).rows.push([item, i]);
  });
  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="Results"
      className="scroll-hidden max-h-[min(436px,calc(100vw*632/752-198px))] pb-2"
    >
      {groups.map(({ sectionIndex, rows }) => {
        const section = sections[sectionIndex];
        return (
          <div key={section.id} role="group" aria-label={section.title}>
            <Heading>{section.title}</Heading>
            {sectionLines(rows, section.title).map(({ heading, key, item, index: i }) => {
              if (heading) return <Heading key={key}>{heading}</Heading>;
              const row = {
                ref: (el) => (rowRefs.current[i] = el),
                selected: i === selected,
                onHover: () => i !== selected && onSelect(i),
                onOpen: () => onOpen(item.action ?? item.result, "open"),
              };
              if (item.action) {
                return <ActionRow key={item.key} action={item.action} app={item.result} {...row} />;
              }
              const Row = item.result.kind === "calculator" ? CalculatorRow : ResultRow;
              return <Row key={item.key} result={item.result} {...row} />;
            })}
          </div>
        );
      })}
    </div>
  );
}
