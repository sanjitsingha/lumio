import { forwardRef } from "react";
import { CornerDownLeft } from "lucide-react";
import { ResultIcon } from "./ResultIcon";
import { cx } from "../../utils/cx";
import { KIND_LABELS } from "../../utils/navigation";

/** Row colours; the selected row gets a light rim inside its edge. */
const rowTone = (selected) =>
  selected ? "bg-accent text-on-accent shadow-[inset_0_0_0_1.5px_var(--color-selection-rim)]" : "text-label";

function EnterKey() {
  return (
    <span className="flex shrink-0 items-center opacity-90">
      <CornerDownLeft size={18} strokeWidth={1.75} />
    </span>
  );
}

export const ResultRow = forwardRef(function ResultRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className={cx("mx-2 flex h-[52px] items-center gap-2.5 rounded-[10px] pr-3 pl-1.5", rowTone(selected))}
    >
      <span className="flex h-[40px] w-[40px] shrink-0 items-center justify-center">
        <ResultIcon icon={result.icon} size={40} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[16px]">{result.title}</span>
        <span
          className={cx("block truncate text-[11px]", selected ? "text-on-accent/85" : "text-secondary")}
          title={result.subtitle}
        >
          {selected && result.note ? result.note : result.subtitle || KIND_LABELS[result.kind]}
        </span>
      </span>
      {selected && <EnterKey />}
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
      className={cx("mx-2 flex h-[52px] items-center gap-3 rounded-[10px] px-3", rowTone(selected))}
    >
      <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-[16px] leading-[22px]">
        <span className="truncate">{result.subtitle}</span>
        <span className="shrink-0">
          = <span className="font-semibold">{result.title}</span>
        </span>
      </span>
      {selected && <EnterKey />}
    </div>
  );
});
