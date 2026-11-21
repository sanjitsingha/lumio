import { forwardRef } from "react";
import { ResultIcon } from "./ResultIcon";
import { cx } from "../../utils/cx";
import { KIND_LABELS } from "../../utils/navigation";

/** Row colours; the selected row is a shade lighter than the panel. */
const rowTone = (selected) => (selected ? "bg-fill-strong text-label" : "text-label");

export const ResultRow = forwardRef(function ResultRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className={cx("mx-2 flex h-[52px] items-center gap-2.5 rounded-[5px] pr-3 pl-1.5", rowTone(selected))}
    >
      <span className="flex h-[24px] w-[24px] shrink-0 items-center justify-center">
        <ResultIcon icon={result.icon} size={24} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[14px]">{result.title}</span>
        <span
          className="block truncate text-[10px] text-secondary"
          title={result.subtitle}
        >
          {selected && result.note ? result.note : result.subtitle || KIND_LABELS[result.kind]}
        </span>
      </span>
    </div>
  );
});

/**
 * One of an app's own actions, listed under "Actions" below the app while it
 * is selected: a row like the app's, with its icon and its name below.
 */
export const ActionRow = forwardRef(function ActionRow({ action, app, ...row }, ref) {
  const result = { icon: app.icon, title: action.title, subtitle: action.subtitle || app.title, kind: "action" };
  return <ResultRow ref={ref} result={result} {...row} />;
});

/** The calculator's answer on one line, "2 + 2 = 4"; a long sum shortens, the answer never does. */
export const CalculatorRow = forwardRef(function CalculatorRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className={cx("mx-2 flex h-[52px] items-center gap-3 rounded-[5px] px-3", rowTone(selected))}
    >
      <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-[14px] leading-[20px]">
        <span className="truncate">{result.subtitle}</span>
        <span className="shrink-0">
          = <span className="font-semibold">{result.title}</span>
        </span>
      </span>
    </div>
  );
});
