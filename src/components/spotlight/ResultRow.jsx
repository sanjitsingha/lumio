import { forwardRef } from "react";
import { ArrowLeftRight } from "lucide-react";
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

/** Long amounts get smaller instead of being cut off. */
function amountSize(amount) {
  if (amount.length <= 12) return "text-[30px] leading-[38px]";
  if (amount.length <= 17) return "text-[24px] leading-[38px]";
  if (amount.length <= 23) return "text-[18px] leading-[38px]";
  return amount.length <= 30 ? "text-[14px] leading-[38px]" : "text-[12px] leading-[38px]";
}

/** One currency of a conversion: its code, the amount, its name and a small note. */
function ConversionSide({ code, amount, name, note }) {
  return (
    <span className="flex min-w-0 flex-col items-center justify-center px-8 text-center">
      <span className="text-[11px] leading-[16px] font-medium text-secondary">{code}</span>
      <span className={cx("mt-0.5 max-w-full truncate font-semibold text-label", amountSize(amount))} title={amount}>
        {amount}
      </span>
      <span className="max-w-full truncate text-[12px] leading-[16px] text-secondary">{name}</span>
      <span className="mt-2 h-[14px] max-w-full truncate text-[10px] leading-[14px] text-tertiary">{note}</span>
    </span>
  );
}

/**
 * A currency answer, as tall as Quick files (181 px with the list's border and
 * padding): one currency on each side of a line with the conversion icon on
 * it. A click or Enter copies the answer.
 */
export const ConversionRow = forwardRef(function ConversionRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      aria-label={result.title ? `${result.source} equals ${result.title}` : `${result.source} to ${result.to}`}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className="relative mx-2 mt-2 grid h-[164px] grid-cols-2 rounded-[5px] transition-colors hover:bg-fill"
    >
      <ConversionSide code={result.from} amount={result.source} name={result.fromName} note={result.updated} />
      <ConversionSide
        code={result.to}
        amount={result.title ?? "—"}
        name={result.toName}
        note={result.rate ?? "Rates aren't available yet"}
      />
      <span aria-hidden className="absolute inset-y-2 left-1/2 w-px -translate-x-1/2 bg-separator" />
      <span
        aria-hidden
        className="absolute top-1/2 left-1/2 flex h-9 w-9 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-separator bg-panel text-secondary"
      >
        <ArrowLeftRight size={16} strokeWidth={1.75} />
      </span>
    </div>
  );
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
