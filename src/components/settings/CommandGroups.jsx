import { Children, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { motion } from "framer-motion";
import { ChevronRight, Search, X } from "lucide-react";
import { Checkbox, Select, placeMenu } from "../common/controls";
import { useKeyCapture } from "../shortcuts/ShortcutRecorder";
import { ResultIcon } from "../spotlight/ResultIcon";
import { useSettings, useUpdateSettings } from "../../hooks/useSettings";
import { api, errorMessage } from "../../services/tauri";
import { toast } from "../../stores/toastStore";
import { ALL_CATEGORIES, categoryOptions, commandCategory } from "../../utils/commandCategories";
import { cx } from "../../utils/cx";
import { liveModifierKeys, shortcutKeys, shortcutText } from "../../utils/keys";

const COLUMNS = "grid grid-cols-[minmax(0,1fr)_84px_150px_16px] items-center gap-x-3 px-3.5";
const MAX_ALIAS_CHARS = 24; // MAX_ALIAS_CHARS in `storage/settings.rs`
const ICON_SIZE = 18;
/** The long lists open with this many rows and add the rest a few per frame, so they never freeze. */
const FIRST_ROWS = 40;
const ROWS_PER_FRAME = 30;

/** Every word typed must be in the command's name, place, keywords, alias or hotkey. */
function matches(command, words, alias, hotkey) {
  const text = [command.title, command.subtitle, alias, hotkey, ...command.keywords].join(" ").toLowerCase();
  return words.every((word) => text.includes(word));
}

/** Stays at the top of the page while the long lists scroll under it. */
function CommandSearch({ value, onChange, onFocus, categories, category, onCategory }) {
  const input = useRef(null);
  return (
    <div className="sticky top-0 z-10 -mt-3 mb-1 flex items-center justify-between gap-3 bg-bg py-3">
      <div className="flex h-[28px] w-[260px] items-center gap-1.5 rounded-[5px] bg-fill px-2 text-secondary focus-within:ring-1 focus-within:ring-white/35">
        <Search size={13} aria-hidden className="shrink-0" />
        <input
          ref={input}
          type="text"
          aria-label="Search commands and settings"
          placeholder="Search commands and settings"
          value={value}
          spellCheck={false}
          onChange={(e) => onChange(e.target.value)}
          onFocus={onFocus}
          onKeyDown={(e) => e.key === "Escape" && onChange("")}
          className="min-w-0 flex-1 bg-transparent text-[12.5px] text-label outline-none placeholder:text-tertiary"
        />
        {value && (
          <button
            type="button"
            aria-label="Clear search"
            onClick={() => {
              onChange("");
              input.current.focus();
            }}
            className="shrink-0 hover:text-label"
          >
            <X size={13} />
          </button>
        )}
      </div>
      {categories && (
        <Select
          label="Category"
          value={category}
          options={categories}
          onChange={(next) => {
            onCategory(next);
            // the shorter list starts at its first row
            input.current?.closest(".scroll-area")?.scrollTo({ top: 0 });
          }}
          buttonClassName="h-[28px] w-[176px] justify-between rounded-[5px] bg-fill px-2.5"
        />
      )}
    </div>
  );
}

/** Saved on leaving the field; Escape puts the saved alias back. */
function AliasField({ title, value, disabled, ownerOf, onFocus, onCommit }) {
  const [text, setText] = useState(value);
  const cancelled = useRef(false);
  useEffect(() => setText(value), [value]);

  const commit = () => {
    if (cancelled.current) {
      cancelled.current = false;
      setText(value);
      return;
    }
    const next = text.trim().toLowerCase();
    const owner = next && next !== value ? ownerOf(next) : null;
    if (owner) {
      toast.error(`“${next}” is already the alias of ${owner}.`);
      setText(value);
      return;
    }
    setText(next);
    if (next !== value) onCommit(next);
  };

  return (
    <input
      aria-label={`Alias for ${title}`}
      value={text}
      placeholder="Alias"
      maxLength={MAX_ALIAS_CHARS}
      disabled={disabled}
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onFocus={onFocus}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Escape") cancelled.current = true;
        if (e.key === "Enter" || e.key === "Escape") e.currentTarget.blur();
      }}
      className="w-full min-w-0 bg-transparent text-[12.5px] text-label outline-none placeholder:text-tertiary focus:placeholder:text-transparent disabled:opacity-40"
    />
  );
}

/**
 * Asks for the keys and saves them as soon as they are pressed. A refused
 * combination stays on show with the reason, so another can be tried.
 */
function HotkeyPopover({ id, title, anchor, hotkey, os, shortcuts }) {
  const { stopRecording, onSaved } = shortcuts;
  const popover = useRef(null);
  const [place, setPlace] = useState(null);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(null);

  const capture = useCallback(
    async (text) => {
      setBusy(true);
      setAttempt({ text, error: null });
      try {
        onSaved(await api.setShortcut(id, text));
      } catch (e) {
        setAttempt({ text, error: errorMessage(e) });
      } finally {
        setBusy(false);
      }
    },
    [id, onSaved],
  );
  const mods = useKeyCapture({ onCapture: capture, onCancel: stopRecording, busy });

  // again when a reason makes it taller
  const failed = attempt?.error;
  useLayoutEffect(() => {
    const { left, top } = placeMenu(anchor.current, popover.current);
    setPlace({ left, top });
  }, [anchor, failed]);

  useEffect(() => {
    const onPress = (e) => {
      if (!popover.current?.contains(e.target) && !anchor.current?.contains(e.target)) stopRecording();
    };
    document.addEventListener("pointerdown", onPress);
    window.addEventListener("scroll", stopRecording, true);
    window.addEventListener("resize", stopRecording);
    return () => {
      document.removeEventListener("pointerdown", onPress);
      window.removeEventListener("scroll", stopRecording, true);
      window.removeEventListener("resize", stopRecording);
    };
  }, [anchor, stopRecording]);

  const held = liveModifierKeys(mods, os);
  // a refused combination stays on show while its keys are being let go
  const letGoing = attempt && held.every((key) => shortcutKeys(attempt.text, os).includes(key));
  const preview = held.length && !letGoing ? `${held.join(" + ")} + …` : shortcutText(attempt?.text ?? hotkey, os);
  return createPortal(
    <motion.div
      ref={popover}
      role="dialog"
      aria-label={`Set hotkey for ${title}`}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.1 }}
      onMouseDown={(e) => e.preventDefault()}
      style={place ?? { left: 0, top: 0, visibility: "hidden" }}
      className="fixed z-50 w-[180px] rounded-[6px] border-[0.5px] border-separator-strong bg-raised p-3 shadow-panel"
    >
      <div className="text-[12.5px] font-medium text-label">Press your hotkey</div>
      <div
        aria-live="polite"
        className={cx(
          "mt-1 flex min-h-[34px] items-center justify-center text-center text-[13px] font-medium",
          preview ? "text-label" : "text-tertiary",
        )}
      >
        {preview || "Waiting for keys…"}
      </div>
      {failed && (
        <p role="alert" className="mt-2 text-[11.5px] leading-[15px] text-danger">
          {failed}
        </p>
      )}
      <div className="mt-2.5 text-[11.5px] text-secondary">Esc to cancel</div>
    </motion.div>,
    document.body,
  );
}

function HotkeyField({ id, title, hotkey, os, disabled, shortcuts }) {
  const anchor = useRef(null);
  const recording = shortcuts.recordingSlot === id;
  const text = hotkey ? shortcutText(hotkey, os) : "Add Hotkey";
  const remove = async () => {
    try {
      shortcuts.onSaved(await api.setShortcut(id, null));
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };
  return (
    <span className="flex min-w-0 items-center gap-1.5 justify-self-start">
      <button
        ref={anchor}
        type="button"
        title={hotkey ? text : undefined}
        aria-label={`${hotkey ? "Change" : "Add"} hotkey for ${title}`}
        aria-haspopup="dialog"
        aria-expanded={recording}
        disabled={disabled}
        onClick={() => (recording ? shortcuts.stopRecording() : shortcuts.startRecording(id))}
        className={cx(
          "min-w-0 truncate text-left text-[12.5px] disabled:opacity-40",
          hotkey || recording ? "text-label" : "text-tertiary enabled:hover:text-label",
        )}
      >
        {text}
      </button>
      {hotkey && (
        <button
          type="button"
          aria-label={`Remove hotkey for ${title}`}
          disabled={disabled}
          onClick={remove}
          className="shrink-0 text-secondary enabled:hover:text-label disabled:opacity-40"
        >
          <X size={12} />
        </button>
      )}
      {recording && <HotkeyPopover id={id} title={title} anchor={anchor} hotkey={hotkey} os={os} shortcuts={shortcuts} />}
    </span>
  );
}

function CommandRow({ command, groupOn, os, shortcuts, saved, bindings, available, ownerOf, onToggle, onAlias }) {
  const { id, title, icon } = command;
  const checked = !saved.disabled.includes(id);
  const on = groupOn && checked;
  return (
    <div
      className={cx(
        COLUMNS,
        "h-[40px] border-t-[0.5px] border-separator",
        // hundreds of rows: only the ones on screen are laid out and painted
        "[contain-intrinsic-size:auto_40px] [content-visibility:auto]",
      )}
    >
      <div className={cx("flex min-w-0 items-center gap-2.5", !on && "opacity-50")}>
        <span className="flex shrink-0 items-center">
          <ResultIcon icon={icon} size={ICON_SIZE} />
        </span>
        <span className="truncate text-[12.5px] font-medium text-label" title={title}>
          {title}
        </span>
      </div>
      <AliasField
        title={title}
        value={saved.aliases[id] ?? ""}
        disabled={!on}
        ownerOf={(alias) => ownerOf(alias, id)}
        onFocus={() => shortcuts.recordingSlot && shortcuts.stopRecording()}
        onCommit={(alias) => onAlias(id, alias)}
      />
      <HotkeyField id={id} title={title} hotkey={bindings[id]} os={os} disabled={!on || !available} shortcuts={shortcuts} />
      <Checkbox label={`Enable ${title}`} checked={checked} disabled={!groupOn} onChange={(v) => onToggle(id, v)} />
    </div>
  );
}

function CommandGroup({ title, icon, on, disabled, onToggle, shortcuts, searching, reveal, children }) {
  const [open, setOpen] = useState(false);
  // once a category was picked the list stays open, also back on All categories
  useEffect(() => {
    if (reveal) setOpen(true);
  }, [reveal]);
  // a search shows its matches whichever way the group was left
  const shown = open || searching;
  const rows = Children.toArray(children);
  const [drawn, setDrawn] = useState(FIRST_ROWS);
  useEffect(() => {
    if (!shown) {
      setDrawn(FIRST_ROWS);
      return undefined;
    }
    if (drawn >= rows.length) return undefined;
    const frame = requestAnimationFrame(() => setDrawn((n) => n + ROWS_PER_FRAME));
    return () => cancelAnimationFrame(frame);
  }, [shown, drawn, rows.length]);
  const toggleOpen = () => {
    // the row being recorded is about to go away
    if (open && shortcuts.recordingSlot) shortcuts.stopRecording();
    setOpen(!open);
  };
  return (
    <section className="mb-2 overflow-hidden rounded-[6px] bg-surface">
      <div className={cx("flex h-[44px] items-center gap-2.5 px-3.5", shown && "bg-fill")}>
        <button
          type="button"
          aria-expanded={shown}
          disabled={searching}
          onClick={toggleOpen}
          className="flex min-w-0 items-center gap-2.5"
        >
          <span className="flex shrink-0 items-center">{icon}</span>
          <span className="truncate text-[13px] font-medium text-label">{title}</span>
          <ChevronRight
            size={14}
            strokeWidth={2.5}
            aria-hidden
            className={cx("shrink-0 text-label transition-transform duration-150", shown && "rotate-90")}
          />
        </button>
        <span className="flex-1" />
        <Checkbox label={`Enable ${title}`} checked={on} disabled={disabled} onChange={onToggle} />
      </div>
      {shown && (
        <>
          <div className={cx(COLUMNS, "h-[26px] border-t-[0.5px] border-separator text-[11.5px] font-medium text-secondary")}>
            <span>Name</span>
            <span>Alias</span>
            <span>Hotkey</span>
            <span />
          </div>
          {rows.slice(0, drawn)}
        </>
      )}
    </section>
  );
}

/** Spotlight's commands by group, each with an alias, a hotkey and a way to turn it off. */
export function CommandGroups({ os, shortcuts }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const [commands, setCommands] = useState([]);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState(ALL_CATEGORIES);
  const [reveal, setReveal] = useState(0);

  useEffect(() => {
    api.getCommands().then(setCommands).catch((e) => toast.error(errorMessage(e)));
  }, []);

  if (commands.length === 0) return null;
  const { spotlight: s, commands: saved } = settings;
  const bindings = settings.shortcuts.bindings;
  const ownerOf = (alias, except) => {
    const id = Object.keys(saved.aliases).find((key) => key !== except && saved.aliases[key] === alias);
    return id ? (commands.find((c) => c.id === id)?.title ?? id) : null;
  };
  const onToggle = (id, value) =>
    update({ commands: { disabled: value ? saved.disabled.filter((d) => d !== id) : [...saved.disabled, id] } });
  const onAlias = (id, alias) => update({ commands: { aliases: { [id]: alias } } });
  // "Set Volume to 25%" comes before "Set Volume to 100%"
  const ofKind = (kind) =>
    commands.filter((c) => c.kind === kind).sort((a, b) => a.title.localeCompare(b.title, undefined, { numeric: true }));
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const filtering = category !== ALL_CATEGORIES;
  const searching = words.length > 0 || filtering;

  const actions = ofKind("action");
  const pages = ofKind("setting");
  const apps = ofKind("app");
  const settingsApp = os === "windows" ? "Windows Settings" : "System Settings";
  const groups = [
    {
      id: "action",
      items: actions,
      title: "System",
      icon: <img src="/settings-icons/general.svg" alt="" width={ICON_SIZE} height={ICON_SIZE} draggable={false} />,
      on: s.searchActions,
      setOn: (v) => update({ spotlight: { searchActions: v } }),
    },
    {
      id: "setting",
      items: pages,
      title: settingsApp,
      // each page has its own glyph; the group shows the settings app as a whole
      icon: <ResultIcon icon={{ type: "symbol", name: "settings" }} size={ICON_SIZE} />,
      on: s.searchSettings,
      setOn: (v) => update({ spotlight: { searchSettings: v } }),
    },
    {
      id: "app",
      items: apps,
      title: "Applications",
      icon: <ResultIcon icon={{ type: "symbol", name: "app-window" }} size={ICON_SIZE} />,
      on: s.searchApplications,
      setOn: (v) => update({ spotlight: { searchApplications: v } }),
    },
  ]
    .map((group) => ({
      ...group,
      items: group.items.filter(
        (c) => (!filtering || commandCategory(c, os) === category) && matches(c, words, saved.aliases[c.id], bindings[c.id]),
      ),
    }))
    .filter((group) => group.items.length > 0);

  return (
    <div className="mb-5">
      <CommandSearch
        value={query}
        onChange={setQuery}
        onFocus={() => shortcuts.recordingSlot && shortcuts.stopRecording()}
        categories={categoryOptions(commands, os)}
        category={category}
        onCategory={(next) => {
          setCategory(next);
          setReveal((n) => n + 1);
        }}
      />
      {groups.length === 0 && (
        <div className="rounded-[6px] bg-surface px-3.5 py-6 text-center text-[12.5px] text-secondary">
          Nothing matches “{query.trim()}”{filtering ? ` in ${category}` : ""}.
        </div>
      )}
      {groups.map((group) => (
        <CommandGroup
          key={group.id}
          title={group.title}
          icon={group.icon}
          on={group.on}
          disabled={!s.enabled}
          onToggle={group.setOn}
          shortcuts={shortcuts}
          searching={searching}
          reveal={reveal}
        >
          {group.items.map((command) => (
            <CommandRow
              key={command.id}
              command={command}
              groupOn={s.enabled && group.on}
              os={os}
              shortcuts={shortcuts}
              saved={saved}
              bindings={bindings}
              available={!!shortcuts.report?.available}
              ownerOf={ownerOf}
              onToggle={onToggle}
              onAlias={onAlias}
            />
          ))}
        </CommandGroup>
      ))}
    </div>
  );
}
