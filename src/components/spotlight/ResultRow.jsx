import { forwardRef } from "react";
import { ArrowLeftRight, Equal } from "lucide-react";
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

/** A long equation also wraps, so more of what was typed stays in view. */
function equationSize(equation) {
  if (equation.length <= 23) return cx("truncate", amountSize(equation));
  return equation.length <= 90
    ? "line-clamp-3 text-[14px] leading-[20px] break-words"
    : "line-clamp-4 text-[12px] leading-[16px] break-words";
}

/** One half of an answer: a small label, the value, and anything below it. */
function AnswerSide({ label, value, size, children }) {
  return (
    <span className="flex min-w-0 flex-col items-center justify-center px-8 text-center">
      <span className="text-[11px] leading-[16px] font-medium text-secondary">{label}</span>
      <span className={cx("mt-0.5 max-w-full font-semibold text-label", size)} title={value}>
        {value}
      </span>
      {children}
    </span>
  );
}

/** One currency of a conversion: its code, the amount, its name and a small note. */
function ConversionSide({ code, amount, name, note }) {
  return (
    <AnswerSide label={code} value={amount} size={cx("truncate", amountSize(amount))}>
      <span className="max-w-full truncate text-[12px] leading-[16px] text-secondary">{name}</span>
      <span className="mt-2 h-[14px] max-w-full truncate text-[10px] leading-[14px] text-tertiary">{note}</span>
    </AnswerSide>
  );
}

/**
 * An answer as tall as Quick files (181 px with the list's border and
 * padding): one side each of a line with an icon on it. A click or Enter
 * copies the answer.
 */
const AnswerPanel = forwardRef(function AnswerPanel({ label, Icon, selected, onHover, onOpen, children }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      aria-label={label}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className="relative mx-2 mt-2 grid h-[164px] grid-cols-2 rounded-[5px] bg-fill"
    >
      {children}
      <span aria-hidden className="absolute inset-y-2 left-1/2 w-px -translate-x-1/2 bg-separator" />
      <span
        aria-hidden
        className="absolute top-1/2 left-1/2 flex h-9 w-9 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-separator bg-panel text-secondary"
      >
        <Icon size={16} strokeWidth={1.75} />
      </span>
    </div>
  );
});

/** The time somewhere, as tall as the conversions: its place, zone and gap on the left, only the time, big, on the right. Enter copies it. */
export const TimeRow = forwardRef(function TimeRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      aria-label={`The time in ${result.to} is ${result.title}`}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className="mx-2 mt-2 flex h-[164px] items-center justify-between gap-6 rounded-[5px] bg-fill px-8"
    >
      <span className="flex min-w-0 flex-col items-start text-left">
        <span className="max-w-full truncate text-[22px] leading-[28px] font-semibold text-label">{result.to}</span>
        <span className="mt-1 max-w-full truncate text-[12px] leading-[16px] text-secondary">{result.toName}</span>
        <span className="mt-0.5 max-w-full truncate text-[12px] leading-[16px] text-tertiary">{result.rate}</span>
      </span>
      <span className="shrink-0 text-[56px] leading-[64px] font-semibold tracking-tight whitespace-nowrap text-label" title={result.title}>
        {result.title}
      </span>
    </div>
  );
});

/** A currency or measurement answer, one unit on each side. */
export const ConversionRow = forwardRef(function ConversionRow({ result, ...row }, ref) {
  const label = result.title ? `${result.source} equals ${result.title}` : `${result.source} to ${result.to}`;
  return (
    <AnswerPanel ref={ref} label={label} Icon={ArrowLeftRight} {...row}>
      <ConversionSide code={result.from} amount={result.source} name={result.fromName} note={result.updated} />
      <ConversionSide
        code={result.to}
        amount={result.title ?? "—"}
        name={result.toName}
        note={result.rate ?? "Rates aren't available yet"}
      />
    </AnswerPanel>
  );
});

/** The calculator's answer: the equation as typed on the left, the result on the right. */
export const CalculatorRow = forwardRef(function CalculatorRow({ result, ...row }, ref) {
  return (
    <AnswerPanel ref={ref} label={`${result.subtitle} equals ${result.title}`} Icon={Equal} {...row}>
      <AnswerSide label="Equation" value={result.subtitle} size={equationSize(result.subtitle)} />
      <AnswerSide label="Result" value={result.title} size={cx("truncate", amountSize(result.title))} />
    </AnswerPanel>
  );
});
