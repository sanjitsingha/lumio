import { useEffect, useId, useRef, useState } from "react";
import { motion } from "framer-motion";
import { ChevronDown, LoaderCircle } from "lucide-react";
import { cx } from "../../utils/cx";

const KNOB_TRAVEL = 12; // .lm-switch width - knob width - 2 * inset

export function Switch({ checked, onChange, disabled = false, label }) {
  // While pressed, and for a moment after so the slide plays as glass, the
  // knob swells into a clear lens over the track.
  const [glass, setGlass] = useState(false);
  const settle = useRef();
  useEffect(() => () => clearTimeout(settle.current), []);
  const press = () => {
    clearTimeout(settle.current);
    setGlass(true);
  };
  const release = () => {
    clearTimeout(settle.current);
    settle.current = setTimeout(() => setGlass(false), 260);
  };

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      onPointerDown={press}
      onPointerUp={release}
      onPointerLeave={release}
      onPointerCancel={release}
      className={cx("lm-switch shrink-0", disabled && "opacity-40")}
    >
      <motion.span
        className="lm-switch-knob"
        data-glass={glass || undefined}
        initial={false}
        animate={{ x: checked ? KNOB_TRAVEL : 0, scaleX: glass ? 1.4 : 1, scaleY: glass ? 1.3 : 1 }}
        transition={{ type: "spring", stiffness: 520, damping: 26 }}
      />
    </button>
  );
}

export function Segmented({ value, options, onChange, disabled = false, size = "md" }) {
  const id = useId();
  return (
    <div
      role="radiogroup"
      className={cx("lm-segmented inline-flex rounded-[7px] p-[2px]", disabled && "pointer-events-none opacity-40")}
    >
      {options.map((opt) => {
        const active = opt.value === value;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(opt.value)}
            className={cx(
              "relative rounded-[5px] font-medium",
              size === "sm" ? "px-2.5 py-[2px] text-[11.5px]" : "px-3 py-[3px] text-[12px]",
              active ? "text-label" : "text-secondary hover:text-label",
            )}
          >
            {active && (
              <motion.span
                layoutId={`seg-${id}`}
                transition={{ type: "spring", stiffness: 500, damping: 30 }}
                className="lm-segment absolute inset-0 rounded-[5px]"
              />
            )}
            <span className="relative">{opt.label}</span>
          </button>
        );
      })}
    </div>
  );
}

export function Slider({ value, min, max, step = 1, onChange, onCommit, format = (v) => v, width = 180, disabled = false }) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className={cx("flex items-center gap-3", disabled && "opacity-40")}>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        onPointerUp={(e) => onCommit?.(Number(e.currentTarget.value))}
        onKeyUp={(e) => onCommit?.(Number(e.currentTarget.value))}
        className="lm-range"
        style={{ width, "--pct": `${pct}%` }}
      />
      <span className="w-[54px] text-right text-[12px] tabular-nums text-secondary">{format(value)}</span>
    </div>
  );
}

export function Select({ value, options, onChange, disabled = false }) {
  return (
    <div className={cx("relative inline-flex", disabled && "opacity-40")}>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="appearance-none rounded-[6px] bg-surface py-[3px] pr-7 pl-2.5 text-[12.5px] text-label shadow-control outline-none dark:bg-raised"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown size={13} className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-secondary" />
    </div>
  );
}

/** A macOS push button: a rounded rectangle that darkens while pressed. */
export function Button({ children, onClick, variant = "default", busy = false, disabled = false, icon: Icon, type = "button" }) {
  const styles = {
    default: "lm-button px-3 text-label",
    primary: "lm-button-primary px-3",
    destructive: "lm-button px-3 text-danger",
    plain: "text-accent hover:underline",
  };
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled || busy}
      className={cx(
        "inline-flex h-[22px] shrink-0 items-center gap-1.5 rounded-[6px] text-[13px] whitespace-nowrap disabled:opacity-45",
        styles[variant],
      )}
    >
      {busy ? <LoaderCircle size={13} className="animate-spin" /> : Icon ? <Icon size={13} /> : null}
      {children}
    </button>
  );
}

/** Inset grouped list, like macOS System Settings. */
export function Group({ title, footer, children }) {
  return (
    <section className="mb-5">
      {title && <h3 className="mb-1.5 px-1 text-[12px] text-secondary">{title}</h3>}
      <div className="overflow-hidden rounded-[10px] border-[0.5px] border-separator bg-surface [&>*+*]:border-t-[0.5px] [&>*+*]:border-separator">
        {children}
      </div>
      {footer && <p className="mt-1.5 px-1 text-[11.5px] leading-[15px] font-light text-secondary">{footer}</p>}
    </section>
  );
}

export function Row({ label, description, children, disabled = false, align = "center" }) {
  return (
    <div className={cx("flex min-h-[44px] gap-4 px-3.5 py-2.5", align === "start" ? "items-start" : "items-center", disabled && "opacity-50")}>
      <div className="min-w-0 flex-1">
        <div className="text-[13px] text-label">{label}</div>
        {description && <div className="mt-0.5 text-[11.5px] leading-[15px] font-light text-secondary">{description}</div>}
      </div>
      {children !== undefined && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

export function ToggleRow({ label, description, checked, onChange, disabled }) {
  return (
    <Row label={label} description={description} disabled={disabled}>
      <Switch checked={!!checked} onChange={onChange} disabled={disabled} label={label} />
    </Row>
  );
}

export function KeyCaps({ keys, size = "md", muted = false }) {
  if (!keys?.length) return <span className="text-[12px] text-tertiary">None</span>;
  return (
    <span className="inline-flex items-center gap-1">
      {keys.map((k, i) => (
        <kbd
          key={`${k}-${i}`}
          className={cx(
            "inline-flex items-center justify-center rounded-[5px] border-[0.5px] border-separator-strong bg-surface font-sans font-medium shadow-[0_1px_0_var(--lm-separator-strong)] dark:bg-raised",
            size === "sm" ? "h-[18px] min-w-[18px] px-1 text-[10.5px]" : "h-[22px] min-w-[22px] px-1.5 text-[11.5px]",
            muted ? "text-secondary" : "text-label",
          )}
        >
          {k}
        </kbd>
      ))}
    </span>
  );
}

export function StatusDot({ tone }) {
  const color = { ok: "bg-success", warn: "bg-warning", error: "bg-danger", off: "bg-tertiary" }[tone] || "bg-tertiary";
  return <span className={cx("inline-block h-[7px] w-[7px] shrink-0 rounded-full", color)} />;
}
