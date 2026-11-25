import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CircleAlert, SearchX } from "lucide-react";
import { SearchBar } from "../components/spotlight/SearchBar";
import { ResultList } from "../components/spotlight/ResultList";
import { SpotlightFooter } from "../components/spotlight/SpotlightFooter";
import { QUICK_PANEL_HEIGHT, QuickPanel } from "../components/spotlight/QuickPanel";
import { useSearch } from "../hooks/useSearch";
import { useAutoHeight } from "../hooks/useAutoHeight";
import { useConversionResults } from "../hooks/useConversionResults";
import { useSettings } from "../hooks/useSettings";
import { usePlatform } from "../hooks/usePlatform";
import { api, errorMessage } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";
import { calculate } from "../utils/calc";
import { suggestFromResults, suggestFullName } from "../utils/suggestResults";
import { copyText } from "../utils/clipboard";
import { answerConversion, suggestCurrency } from "../utils/currency";
import { answerMeasurement, suggestMeasurement } from "../utils/units";
import { answerTime, loadPlaces, suggestTime } from "../utils/worldTime";
import { deletePreviousSegment } from "../utils/keys";
import { expansionFor, flatten, jumpSection, kindLabel, moveSelection, selectItem } from "../utils/navigation";

/** Transparent margin around the panel that holds its drop shadow (`SPOTLIGHT_MARGIN` in window_manager.rs). */
const MARGIN = 36;
/** The panel's top and bottom border (`border`, 1 px each). */
const PANEL_BORDERS = 2;
const EASE = [0.2, 0.8, 0.2, 1];
/** How the panel's height follows its contents as results come and go. */
const RESIZE = { duration: 0.2, ease: EASE };
/** What the results list gives up for the quick panel; its height cap already spares 29 px (ResultList.jsx). */
const QUICK_PANEL_RESERVE = QUICK_PANEL_HEIGHT - 29;

export default function SpotlightPage() {
  const settings = useSettings();
  // Quick files is there once the platform has loaded, on builds without the notch
  const platform = usePlatform();
  const hasQuickFiles = platform ? !platform.notchAvailable : false;
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
  // the area below the results, opened with Tab while the search field is empty
  const [quickOpen, setQuickOpen] = useState(false);
  // the saved exchange rates; conversions never go to the network
  const [rates, setRates] = useState(null);
  const inputRef = useRef(null);
  // the panel's contents at their natural size; state, not a ref, because the
  // panel is remounted (key) every time Spotlight opens
  const [content, setContent] = useState(null);

  const contentHeight = useAutoHeight(content, 2 * MARGIN + PANEL_BORDERS, RESIZE.duration * 1000);

  const loadRecent = useCallback(() => {
    api.getRecentItems(6).then(setRecent).catch(() => setRecent([]));
  }, []);

  const loadRates = useCallback(() => {
    api.getCurrencyRates().then(setRates).catch(() => {});
  }, []);

  const focusInput = useCallback(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    input.select();
  }, []);

  useEffect(() => {
    loadRecent();
    loadRates();
    focusInput();
    const offShown = subscribe(EVENTS.spotlightShown, () => {
      setClosing(false);
      setShowKey((k) => k + 1);
      setSelected(0);
      setLaunchError(null);
      setQuickOpen(false);
      loadRecent();
      loadRates();
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
  }, [focusInput, loadRates, loadRecent, reset]);

  // Show previous results while the next ones load, to avoid flicker.
  const shown = response || (query.trim() ? staleResponse : null);
  // computed here rather than in the backend, so it follows every keystroke
  const calc = useMemo(() => calculate(query), [query]);
  // money first, so that "1 pound to inr" is currency and "1 pound to kg" a measurement
  const [placesLoaded, setPlacesLoaded] = useState(false);
  // when Spotlight opens, and again while typing if that did not work
  useEffect(() => {
    if (!placesLoaded) loadPlaces().then(() => setPlacesLoaded(true), () => {});
  }, [placesLoaded, query]);
  const conversion = useMemo(
    () => (calc ? null : (answerConversion(query, rates) ?? answerMeasurement(query) ?? answerTime(query))),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- answerTime reads the places once they are loaded
    [query, calc, rates, placesLoaded],
  );
  const related = useConversionResults(conversion);
  // "time" goes on as "time in New York", "5 f" as "5 feet", "calc" as "calculator"; otherwise what the search history suggests
  const suggested = useMemo(
    () => (query.trim() ? (suggestTime(query) ?? suggestMeasurement(query) ?? suggestCurrency(query) ?? suggestFromResults(query, shown?.sections) ?? shown?.suggestion ?? null) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps -- suggestTime reads the places once they are loaded
    [query, shown?.suggestion, shown?.sections, placesLoaded],
  );
  // otherwise "photoshop" names Adobe Photoshop 2027 without starting it: its whole name follows a dash
  const named = useMemo(
    () => (suggested || !query.trim() ? null : suggestFullName(query, shown?.sections)),
    [query, suggested, shown?.sections],
  );
  const sections = useMemo(() => {
    if (query.trim()) {
      const found = shown?.sections ?? [];
      if (conversion) {
        // as tall as Quick files (ConversionRow), with only what relates to it below; Enter copies the title
        const kind = conversion.time ? "time" : conversion.measure ? "measurement" : "conversion";
        const result = { ...conversion, id: "conversion:answer", kind, title: conversion.answer, reveal: false, actions: [] };
        const answer = { id: "conversion", title: conversion.section ?? (conversion.measure ? "Measurement" : "Currency"), results: [result] };
        return related.length ? [answer, { id: "related", title: "Related", results: related }] : [answer];
      }
      if (!calc) return found;
      // the equation beside its result (CalculatorRow); Enter copies the title
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
  }, [query, shown, recent, calc, conversion, related]);
  const items = useMemo(() => flatten(sections, expanded), [sections, expanded]);

  // new results start at the first item, showing its actions if it is an app
  useEffect(() => {
    setSelected(0);
    setExpanded(expansionFor(flatten(sections)[0]));
  }, [shown?.requestId, recent, Boolean(conversion), related]);

  // keyboard moves expand and collapse app actions; hovering only highlights,
  // so rows never shift under the pointer
  const select = (index) => {
    const next = selectItem(sections, expanded, items, index);
    setExpanded(next.expanded);
    setSelected(next.selected);
  };

  // an action row runs that action
  const current = items[selected]?.action ?? items[selected]?.result;
  const noResults = !calc && !conversion && query.trim() && response && response.total === 0 && !loading;

  const open = useCallback(
    async (result, mode) => {
      if (!result) return;
      setLaunchError(null);
      if (result.kind === "calculator" || result.kind === "conversion" || result.kind === "measurement" || result.kind === "time") {
        // a conversion has nothing to copy until rates are saved
        if (!result.title) return;
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
        // while searching, Tab keeps moving between the result sections
        if (hasQuickFiles && !query.trim() && !e.shiftKey) setQuickOpen((isOpen) => !isOpen);
        else select(jumpSection(items, selected, e.shiftKey ? -1 : 1));
        break;
      case "Enter":
        e.preventDefault();
        open(current, e.ctrlKey || e.metaKey || e.altKey ? "reveal" : "open");
        break;
      case "Escape":
        e.preventDefault();
        if (quickOpen) setQuickOpen(false);
        else api.hideSpotlight();
        break;
      case "ArrowRight":
      case "End": {
        // a completion is put in after the text; a whole name replaces it
        const accepted = suggested && suggested.length > query.length ? suggested : named;
        const atEnd = input.selectionStart === query.length && input.selectionEnd === query.length;
        if (accepted && atEnd) {
          e.preventDefault();
          setQuery(accepted);
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
  // results, recent items, "no results" or an error below the search field
  const hasBody = Boolean(hasList || noResults || error || launchError);
  // what the selected result is, at the right of the search field while searching
  const kind = query.trim() ? kindLabel(items[selected]) : null;
  return (
    <div
      className="flex h-full w-full items-start justify-center"
      style={{ padding: MARGIN }}
      // the margin is part of the window on Windows (Linux passes clicks through): a click there is outside
      onMouseDown={(e) => e.target === e.currentTarget && api.hideSpotlight()}
    >
      <motion.div
        key={showKey}
        // no opening animation: the panel is there as soon as the window is
        initial={false}
        animate={closing ? { opacity: 0, scale: 0.985, y: 0 } : { opacity: 1, scale: 1, y: 0 }}
        // the fade-out must end before the window hides (FADE_OUT, 120 ms, in window_manager.rs)
        transition={{ duration: closing ? 0.1 : 0.16, ease: EASE }}
        className="w-full overflow-hidden rounded-[8px] border border-panel-border bg-panel lm-panel-sheen shadow-panel"
      >
        <motion.div
          initial={false}
          animate={{ height: contentHeight ?? "auto" }}
          transition={RESIZE}
          className="overflow-hidden"
        >
          {/* the panel's inner padding, counted in the results list's height cap (ResultList.jsx) */}
          <div ref={setContent} className="p-2">
            <SearchBar
              ref={inputRef}
              value={query}
              onChange={(v) => {
                setQuery(v);
                setLaunchError(null);
                if (v.trim()) setQuickOpen(false);
              }}
              onKeyDown={onKeyDown}
              loading={loading}
              suggestion={suggested}
              name={named}
              kind={kind}
              quickOpen={quickOpen}
              onToggleQuick={hasQuickFiles ? () => setQuickOpen((isOpen) => !isOpen) : undefined}
            />
            <AnimatePresence initial={false}>
              {hasBody && (
                <motion.div
                  key="body"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.1 }}
                  className="border-t border-separator"
                >
                  {hasList && (
                    <ResultList
                      sections={sections}
                      items={items}
                      selected={selected}
                      onSelect={setSelected}
                      onOpen={open}
                      reserve={quickOpen ? QUICK_PANEL_RESERVE : 0}
                    />
                  )}
                  {noResults && (
                    <div className="flex items-center gap-3 px-5 py-4 text-secondary">
                      <SearchX size={17} strokeWidth={1.8} />
                      <div>
                        <div className="text-[12px] text-label">No results for “{query.trim()}”</div>
                        <div className="text-[11px]">Try another name, or check which folders Lummo indexes in Settings.</div>
                      </div>
                    </div>
                  )}
                  {(error || launchError) && (
                    <div className="flex items-center gap-2 px-5 py-3 text-[11px] text-danger">
                      <CircleAlert size={13} />
                      <span className="truncate">{launchError || errorMessage(error)}</span>
                    </div>
                  )}
                  {hasList && !conversion && (
                    <SpotlightFooter index={shown?.index} />
                  )}
                </motion.div>
              )}
            </AnimatePresence>
            <AnimatePresence initial={false}>{quickOpen && <QuickPanel key="quick" onError={setLaunchError} />}</AnimatePresence>
          </div>
        </motion.div>
      </motion.div>
    </div>
  );
}
