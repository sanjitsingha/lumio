import { useEffect, useRef } from "react";
import { ResultRow } from "./ResultRow";

/** Grouped results with section headers; scrolls the selection into view. */
export function ResultList({ sections, items, selected, onSelect, onOpen }) {
  const rowRefs = useRef([]);

  useEffect(() => {
    rowRefs.current[selected]?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  let flatIndex = 0;
  return (
    <div role="listbox" aria-label="Results" className="scroll-area max-h-[min(452px,calc(100vh-150px))] pb-2">
      {sections.map((section) => (
        <div key={section.id} role="group" aria-label={section.title}>
          <div className="px-[22px] pt-2.5 pb-1 text-[11px] font-semibold tracking-[0.01em] text-secondary">
            {section.title}
          </div>
          {section.results.map((result) => {
            const i = flatIndex++;
            return (
              <ResultRow
                key={result.id}
                ref={(el) => (rowRefs.current[i] = el)}
                result={result}
                selected={i === selected}
                onHover={() => i !== selected && onSelect(i)}
                onOpen={() => onOpen(items[i].result, "open")}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}
