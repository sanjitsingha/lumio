import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { motion } from "framer-motion";
import { Check, ChevronDown, LoaderCircle } from "lucide-react";
import { cx } from "../../utils/cx";

const KNOB_TRAVEL = 12; // .lm-switch width - knob width - 2 * inset

export function Switch({ checked, onChange, disabled = false, label }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx("lm-switch shrink-0", disabled && "opacity-40")}
    >
      <motion.span
        className="lm-switch-knob"
        initial={false}
        animate={{ x: checked ? KNOB_TRAVEL : 0 }}
        transition={{ duration: 0.15, ease: "easeOut" }}
      />
    </button>
  );
}

export function Checkbox({ checked, onChange, label, disabled = false }) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "flex h-[16px] w-[16px] shrink-0 items-center justify-center rounded-[3px] border text-label",
        checked ? "border-white/45 bg-white/[0.08]" : "border-white/25",
        disabled && "opacity-40",
      )}
    >
      {checked && <Check size={11} strokeWidth={3} aria-hidden />}
    </button>
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

/**
 * A small number box. What is typed stays local until Enter or leaving the
 * box, then goes to `onCommit` rounded and kept within `min`–`max`.
 */
export function NumberField({ value, min, max, onCommit, disabled = false, width = 64, label }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const typed = Math.round(Number(text));
    const next = text.trim() !== "" && Number.isFinite(typed) ? Math.min(max, Math.max(min, typed)) : value;
    setText(String(next));
    if (next !== value) onCommit(next);
  };
  return (
    <input
      type="number"
      inputMode="numeric"
      aria-label={label}
      min={min}
      max={max}
      value={text}
      disabled={disabled}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
      style={{ width }}
      className={cx(
        "rounded-[4px] bg-surface px-2 py-[3px] text-right text-[12.5px] tabular-nums text-label shadow-control outline-none dark:bg-raised",
        disabled && "opacity-40",
      )}
    />
  );
}

/** The space between a dropdown and its button, and kept from the window's edges. */
const MENU_GAP = 4;
const MENU_EDGE = 8;

/** Where a floating menu goes: below `anchor`, or above it when there is more room there. */
export function placeMenu(anchor, menu) {
  const box = anchor.getBoundingClientRect();
  const width = Math.max(menu.offsetWidth, box.width);
  const height = menu.offsetHeight;
  const below = window.innerHeight - box.bottom - MENU_GAP - MENU_EDGE;
  const above = box.top - MENU_GAP - MENU_EDGE;
  const down = height <= below || below >= above;
  const maxHeight = Math.min(height, down ? below : above);
  return {
    left: Math.max(MENU_EDGE, Math.min(box.right - width, window.innerWidth - width - MENU_EDGE)),
    top: down ? box.bottom + MENU_GAP : box.top - MENU_GAP - maxHeight,
    minWidth: width,
    maxHeight,
  };
}

/**
 * A dropdown. Its menu opens in a layer over the window, so a group's rounded
 * clipping or the page's scrolling cannot cut it off. Focus stays on the
 * button, which handles the keyboard. `locked` shows the value at full
 * strength but does not open.
 */
export function Select({ value, options, onChange, label, disabled = false, locked = false }) {
  const id = useId();
  const button = useRef(null);
  const menu = useRef(null);
  const [isOpen, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const [place, setPlace] = useState(null);
  const inert = disabled || locked;
  const open = isOpen && !inert;
  const current = options.findIndex((o) => o.value === value);
  const optionId = (i) => `${id}-option-${i}`;

  const show = () => {
    setActive(Math.max(current, 0));
    setPlace(null);
    setOpen(true);
  };
  const choose = (option) => {
    setOpen(false);
    if (option.value !== value) onChange(option.value);
  };

  // below the button, or above it when there is more room there
  useLayoutEffect(() => {
    if (open) setPlace(placeMenu(button.current, menu.current));
  }, [open]);

  useEffect(() => {
    if (!open) return undefined;
    const close = () => setOpen(false);
    const onPress = (e) => {
      if (!menu.current?.contains(e.target) && !button.current?.contains(e.target)) close();
    };
    const onScroll = (e) => {
      if (!menu.current?.contains(e.target)) close();
    };
    document.addEventListener("pointerdown", onPress);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      document.removeEventListener("pointerdown", onPress);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [open]);

  useEffect(() => {
    if (open && place) menu.current?.children[active]?.scrollIntoView?.({ block: "nearest" });
  }, [open, place, active]);

  const onKeyDown = (e) => {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    const last = options.length - 1;
    const moves = {
      ArrowDown: Math.min(active + 1, last),
      ArrowUp: Math.max(active - 1, 0),
      Home: 0,
      End: last,
    };
    if (e.key in moves) {
      e.preventDefault();
      setActive(moves[e.key]);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(options[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setOpen(false);
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        role="combobox"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? `${id}-menu` : undefined}
        aria-activedescendant={open ? optionId(active) : undefined}
        disabled={inert}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
        className={cx(
          "group inline-flex h-[24px] max-w-[220px] items-center gap-1.5 text-[12.5px] text-label",
          disabled && "opacity-40",
        )}
      >
        <span className="truncate">{options[current]?.label ?? value}</span>
        <ChevronDown
          size={13}
          aria-hidden
          className={cx(
            "shrink-0 text-secondary transition-transform duration-150",
            !inert && "group-hover:text-label",
            open && "rotate-180",
          )}
        />
      </button>
      {open &&
        createPortal(
          <motion.div
            ref={menu}
            id={`${id}-menu`}
            role="listbox"
            aria-label={label}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.1 }}
            // keeps the focus, and with it the keyboard, on the button
            onMouseDown={(e) => e.preventDefault()}
            style={place ?? { left: 0, top: 0, visibility: "hidden" }}
            className="fixed z-50 max-w-[280px] overflow-y-auto rounded-[6px] border-[0.5px] border-separator-strong bg-raised p-1 shadow-panel [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          >
            {options.map((option, i) => (
              <div
                key={option.value}
                id={optionId(i)}
                role="option"
                aria-selected={i === current}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(option)}
                className={cx(
                  "flex h-[26px] items-center justify-between gap-4 rounded-[4px] px-2 text-[12.5px] whitespace-nowrap text-label",
                  i === active && "bg-fill-strong",
                )}
              >
                <span className="min-w-0 truncate">{option.label}</span>
                <Check size={12} strokeWidth={2.5} aria-hidden className={cx("shrink-0", i !== current && "invisible")} />
              </div>
            ))}
          </motion.div>,
          document.body,
        )}
    </>
  );
}

/** A macOS push button: a rounded rectangle that darkens while pressed. */
export function Button({ children, onClick, variant = "default", busy = false, disabled = false, icon: Icon, type = "button" }) {
  const styles = {
    default: "lm-button px-2 text-label",
    primary: "lm-button-primary px-2",
    destructive: "lm-button px-2 text-danger",
    plain: "text-accent hover:underline",
  };
  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled || busy}
      className={cx(
        "inline-flex h-[20px] shrink-0 items-center gap-1 rounded-[4px] text-[11.5px] whitespace-nowrap disabled:opacity-45",
        styles[variant],
      )}
    >
      {busy ? <LoaderCircle size={11} className="animate-spin" /> : Icon ? <Icon size={11} /> : null}
      {children}
    </button>
  );
}

/** Inset grouped list, like macOS System Settings. */
export function Group({ title, children }) {
  return (
    <section className="mb-5">
      {title && <h3 className="mb-1.5 px-1 text-[12px] text-secondary">{title}</h3>}
      <div className="overflow-hidden rounded-[6px] bg-surface [&>*+*]:border-t-[0.5px] [&>*+*]:border-separator">
        {children}
      </div>
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
            "inline-flex items-center justify-center rounded-[3px] border-[0.5px] border-separator-strong bg-surface font-sans font-medium shadow-[0_1px_0_var(--lm-separator-strong)] dark:bg-raised",
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
