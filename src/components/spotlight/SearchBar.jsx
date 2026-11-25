import { forwardRef, useLayoutEffect, useRef, useState } from "react";
import { ChevronDown, LoaderCircle, Search } from "lucide-react";
import { cx } from "../../utils/cx";

/**
 * The input row. `suggestion`, which goes on from the typed text ("calc" →
 * "calculator"), is drawn as ghost text after it; `name`, the whole name of a
 * result the text names without starting it ("photoshop" → "Adobe Photoshop
 * 2027"), after a dash. Either is accepted with → or End. Both are drawn by a
 * copy of the text behind the transparent input, kept in step with the
 * input's horizontal scrolling.
 * `kind` names what the selected result is ("Application"), at the right end.
 * With an empty field that end holds the Tab hint, which opens the quick panel.
 */
export const SearchBar = forwardRef(function SearchBar(
  { value, onChange, onKeyDown, loading, suggestion, name, kind, quickOpen, onToggleQuick },
  ref,
) {
  const input = useRef(null);
  const [scroll, setScroll] = useState(0);
  const syncScroll = () => setScroll(input.current?.scrollLeft ?? 0);
  // typing moves the caret, and with it the input's scroll position
  useLayoutEffect(syncScroll, [value]);

  const ghost =
    suggestion && value && suggestion.toLowerCase().startsWith(value.toLowerCase()) && suggestion.length > value.length
      ? suggestion.slice(value.length)
      : "";
  return (
    <div className="flex h-[48px] items-center gap-3 px-4">
      <Search size={18} strokeWidth={2} className="shrink-0 text-secondary" aria-hidden />
      <div className="relative min-w-0 flex-1 font-search">
        {value && (
          <div aria-hidden className="pointer-events-none absolute inset-0 flex items-center overflow-hidden whitespace-pre text-[18px]">
            <span className="flex items-center" style={{ transform: `translateX(${-scroll}px)` }}>
              <span className="text-transparent">{value}</span>
              {ghost && (
                <>
                  {/* the space joining it to the text stays outside the grey; the grey's left padding is
                      taken back by an equal negative margin, so the first letter sits right at the cursor */}
                  {ghost.startsWith(" ") && <span className="text-tertiary"> </span>}
                  <span className="-ml-1.5 rounded-[4px] bg-fill-strong py-[1px] pr-1.5 pl-1.5 text-tertiary">{ghost.trimStart()}</span>
                </>
              )}
              {!ghost && name && (
                <>
                  <span className="text-tertiary"> — </span>
                  <span className="rounded-[4px] bg-fill-strong px-1.5 py-[1px] text-tertiary">{name}</span>
                </>
              )}
            </span>
          </div>
        )}
        <input
          ref={(el) => {
            input.current = el;
            if (typeof ref === "function") ref(el);
            else if (ref) ref.current = el;
          }}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          onSelect={syncScroll}
          onScroll={syncScroll}
          placeholder="Search anything..."
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          aria-label="Search"
          className="relative w-full bg-transparent text-[18px] text-label outline-none placeholder:text-tertiary"
        />
      </div>
      {kind && <span className="shrink-0 text-[12px] text-secondary">{kind}</span>}
      {onToggleQuick && !value.trim() && (
        <button
          type="button"
          tabIndex={-1}
          aria-expanded={quickOpen}
          // keeps the focus, and with it the keyboard, in the search field
          onMouseDown={(e) => e.preventDefault()}
          onClick={onToggleQuick}
          className="flex shrink-0 items-center gap-1.5 rounded-[4px] py-1 pr-1 pl-1.5 text-[11px] text-secondary hover:bg-fill"
        >
          <kbd className="rounded-[3px] bg-fill-strong px-1.5 py-px font-sans text-[10px] font-medium text-label">Tab</kbd>
          Quick files
          <ChevronDown size={12} aria-hidden className={cx("transition-transform duration-200", quickOpen && "rotate-180")} />
        </button>
      )}
      {loading && <LoaderCircle size={15} className="shrink-0 animate-spin text-secondary" aria-label="Searching" />}
    </div>
  );
});
