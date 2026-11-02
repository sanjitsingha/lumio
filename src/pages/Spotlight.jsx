import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CircleAlert, SearchX } from "lucide-react";
import { SearchBar } from "../components/spotlight/SearchBar";
import { ResultList } from "../components/spotlight/ResultList";
import { SpotlightFooter } from "../components/spotlight/SpotlightFooter";
import { useSearch } from "../hooks/useSearch";
import { useAutoHeight } from "../hooks/useAutoHeight";
import { useSettings } from "../hooks/useSettings";
import { api, errorMessage } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";
import { calculate } from "../utils/calc";
import { copyText } from "../utils/clipboard";
import { deletePreviousSegment } from "../utils/keys";
import { expansionFor, flatten, jumpSection, moveSelection, selectItem } from "../utils/navigation";

/** Transparent margin around the panel that holds its drop shadow (`SPOTLIGHT_MARGIN` in window_manager.rs). */
const MARGIN = 36;
/** The panel's top and bottom border (`border`, 1 px each). */
const PANEL_BORDERS = 2;
const EASE = [0.2, 0.8, 0.2, 1];
/** How the panel's height follows its contents as results come and go. */
const RESIZE = { duration: 0.2, ease: EASE };

export default function SpotlightPage() {
  const settings = useSettings();
  const delay = settings?.spotlight?.searchDelayMs ?? 60;
  const { query, setQuery, response, staleResponse, loading, error, reset } = useSearch(delay);
  const [selected, setSelected] = useState(0);
  // id of the app whose own actions are listed below it
  const [expanded, setExpanded] = useState(null);
  const [recent, setRecent] = useState([]);
  const [showKey, setShowKey] = useState(0);
  // fading out before the window hides
  const [closing, setClosing] = useState(false);
  const [launchError, setLaunchError] = useState(null);
  const inputRef = useRef(null);
  // the panel's contents at their natural size; state, not a ref, because the
  // panel is remounted (key) every time Spotlight opens
  const [content, setContent] = useState(null);

  const contentHeight = useAutoHeight(content, 2 * MARGIN + PANEL_BORDERS, RESIZE.duration * 1000);

  const loadRecent = useCallback(() => {
    api.getRecentItems(6).then(setRecent).catch(() => setRecent([]));
  }, []);

  const focusInput = useCallback(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    input.select();
  }, []);

  useEffect(() => {
    loadRecent();
    focusInput();
    const offShown = subscribe(EVENTS.spotlightShown, () => {
      setClosing(false);
      setShowKey((k) => k + 1);
      setSelected(0);
      setLaunchError(null);
      loadRecent();
      // the webview receives focus a moment after the window is mapped
      requestAnimationFrame(focusInput);
      setTimeout(focusInput, 60);
    });
    const offHiding = subscribe(EVENTS.spotlightHiding, () => setClosing(true));
    // every opening starts with an empty search box
    const offHidden = subscribe(EVENTS.spotlightHidden, () => {
      setRecent([]);
      reset();
    });
    return () => {
      offShown();
      offHiding();
      offHidden();
    };
  }, [focusInput, loadRecent, reset]);

  // Show previous results while the next ones load, to avoid flicker.
  const shown = response || (query.trim() ? staleResponse : null);
  // computed here rather than in the backend, so it follows every keystroke
  const calc = useMemo(() => calculate(query), [query]);
  const sections = useMemo(() => {
    if (query.trim()) {
      const found = shown?.sections ?? [];
      if (!calc) return found;
      // drawn as "2 + 2 = 4" (CalculatorRow); Enter copies the title
      const result = {
        id: "calculator:answer",
        kind: "calculator",
        title: calc.answer,
        subtitle: calc.expression,
        reveal: false,
        actions: [],
      };
      return [{ id: "calculator", title: "Calculator", results: [result] }, ...found];
    }
    return recent.length ? [{ id: "recent", title: "Recent", results: recent }] : [];
  }, [query, shown, recent, calc]);
  const items = useMemo(() => flatten(sections, expanded), [sections, expanded]);

  // new results start at the first item, showing its actions if it is an app
  useEffect(() => {
    setSelected(0);
    setExpanded(expansionFor(flatten(sections)[0]));
  }, [shown?.requestId, recent]);

  // keyboard moves expand and collapse app actions; hovering only highlights,
  // so rows never shift under the pointer
  const select = (index) => {
    const next = selectItem(sections, expanded, items, index);
    setExpanded(next.expanded);
    setSelected(next.selected);
  };

  // an action row runs that action
  const current = items[selected]?.action ?? items[selected]?.result;
  const noResults = !calc && query.trim() && response && response.total === 0 && !loading;

  const open = useCallback(
    async (result, mode) => {
      if (!result) return;
      setLaunchError(null);
      if (result.kind === "calculator") {
        try {
          await copyText(result.title);
          api.hideSpotlight();
        } catch (e) {
          setLaunchError(errorMessage(e));
        }
        return;
      }
      if (mode === "reveal" && !result.reveal) mode = "open";
      try {
        await api.executeResult(result.id, mode, query);
      } catch (e) {
        setLaunchError(errorMessage(e));
        loadRecent();
      }
    },
    [query, loadRecent],
  );

  const onKeyDown = (e) => {
    const input = e.currentTarget;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        select(moveSelection(items.length, selected, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        select(moveSelection(items.length, selected, -1));
        break;
      case "Tab":
        e.preventDefault();
        select(jumpSection(items, selected, e.shiftKey ? -1 : 1));
        break;
      case "Enter":
        e.preventDefault();
        open(current, e.ctrlKey || e.metaKey || e.altKey ? "reveal" : "open");
        break;
      case "Escape":
        e.preventDefault();
        api.hideSpotlight();
        break;
      case "ArrowRight":
      case "End": {
        const suggestion = shown?.suggestion;
        const atEnd = input.selectionStart === query.length && input.selectionEnd === query.length;
        if (suggestion && atEnd && suggestion.length > query.length) {
          e.preventDefault();
          setQuery(suggestion);
        }
        break;
      }
      case "Backspace":
        if ((e.ctrlKey || e.altKey) && input.selectionStart === input.selectionEnd) {
          e.preventDefault();
          const next = deletePreviousSegment(query, input.selectionStart ?? query.length);
          setQuery(next.value);
          requestAnimationFrame(() => input.setSelectionRange(next.caret, next.caret));
        }
        break;
      default:
    }
  };

  const hasList = sections.length > 0;
  return (
    <div
      className="flex h-full w-full items-start justify-center"
      style={{ padding: MARGIN }}
      // the margin is part of the window on Windows (Linux passes clicks through): a click there is outside
      onMouseDown={(e) => e.target === e.currentTarget && api.hideSpotlight()}
    >
      <motion.div
        key={showKey}
        initial={{ opacity: 0, scale: 0.985, y: -6 }}
        animate={closing ? { opacity: 0, scale: 0.985, y: 0 } : { opacity: 1, scale: 1, y: 0 }}
        // the fade-out must end before the window hides (FADE_OUT, 120 ms, in window_manager.rs)
        transition={{ duration: closing ? 0.1 : 0.16, ease: EASE }}
        className="w-full overflow-hidden rounded-[16px] border border-panel-border bg-panel shadow-panel"
      >
        <motion.div
          initial={false}
          animate={{ height: contentHeight ?? "auto" }}
          transition={RESIZE}
          className="overflow-hidden"
        >
          <div ref={setContent}>
            <SearchBar
              ref={inputRef}
              value={query}
              onChange={(v) => {
                setQuery(v);
                setLaunchError(null);
              }}
              onKeyDown={onKeyDown}
              loading={loading}
              suggestion={shown?.suggestion}
            />
            <AnimatePresence initial={false}>
              {(hasList || noResults || error || launchError) && (
                <motion.div
                  key="body"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.1 }}
                  className="border-t border-separator"
                >
                  {hasList && (
                    <ResultList sections={sections} items={items} selected={selected} onSelect={setSelected} onOpen={open} />
                  )}
                  {noResults && (
                    <div className="flex items-center gap-3 px-5 py-4 text-secondary">
                      <SearchX size={20} strokeWidth={1.8} />
                      <div>
                        <div className="text-[13px] text-label">No results for “{query.trim()}”</div>
                        <div className="text-[12px]">Try another name, or check which folders Lummo indexes in Settings.</div>
                      </div>
                    </div>
                  )}
                  {(error || launchError) && (
                    <div className="flex items-center gap-2 px-5 py-3 text-[12px] text-danger">
                      <CircleAlert size={15} />
                      <span className="truncate">{launchError || errorMessage(error)}</span>
                    </div>
                  )}
                  {hasList && (
                    <SpotlightFooter index={shown?.index} />
                  )}
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </motion.div>
      </motion.div>
    </div>
  );
}
