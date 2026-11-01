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
import { deletePreviousSegment } from "../utils/keys";
import { flatten, jumpSection, moveSelection } from "../utils/navigation";

/** Transparent margin around the panel that holds its drop shadow. */
const MARGIN = 20;

export default function SpotlightPage() {
  const settings = useSettings();
  const delay = settings?.spotlight?.searchDelayMs ?? 60;
  const { query, setQuery, response, staleResponse, loading, error } = useSearch(delay);
  const [selected, setSelected] = useState(0);
  const [recent, setRecent] = useState([]);
  const [showKey, setShowKey] = useState(0);
  const [launchError, setLaunchError] = useState(null);
  const inputRef = useRef(null);
  const panelRef = useRef(null);

  useAutoHeight(panelRef, MARGIN);

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
      setShowKey((k) => k + 1);
      setSelected(0);
      setLaunchError(null);
      loadRecent();
      // the webview receives focus a moment after the window is mapped
      requestAnimationFrame(focusInput);
      setTimeout(focusInput, 60);
    });
    const offHidden = subscribe(EVENTS.spotlightHidden, () => setRecent([]));
    return () => {
      offShown();
      offHidden();
    };
  }, [focusInput, loadRecent]);

  // Show previous results while the next ones load, to avoid flicker.
  const shown = response || (query.trim() ? staleResponse : null);
  const sections = useMemo(() => {
    if (query.trim()) return shown?.sections ?? [];
    return recent.length ? [{ id: "recent", title: "Recent", results: recent }] : [];
  }, [query, shown, recent]);
  const items = useMemo(() => flatten(sections), [sections]);

  useEffect(() => setSelected(0), [shown?.requestId, recent]);

  const current = items[selected]?.result;
  const noResults = query.trim() && response && response.total === 0 && !loading;

  const open = useCallback(
    async (result, mode) => {
      if (!result) return;
      if (mode === "reveal" && !result.reveal) mode = "open";
      setLaunchError(null);
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
        setSelected((s) => moveSelection(items.length, s, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelected((s) => moveSelection(items.length, s, -1));
        break;
      case "Tab":
        e.preventDefault();
        setSelected((s) => jumpSection(items, s, e.shiftKey ? -1 : 1));
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
    <div className="flex h-full w-full items-start justify-center" style={{ padding: MARGIN }}>
      <motion.div
        key={showKey}
        ref={panelRef}
        initial={{ opacity: 0, scale: 0.985, y: -6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.16, ease: [0.2, 0.8, 0.2, 1] }}
        className="w-full overflow-hidden rounded-[16px] border-[0.5px] border-panel-border bg-panel shadow-panel"
      >
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
                    <div className="text-[12px]">Try another name, or check which folders Lumio indexes in Settings.</div>
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
                <SpotlightFooter
                  index={shown?.index}
                  canReveal={!!current?.reveal}
                  multipleSections={sections.length > 1}
                />
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}
