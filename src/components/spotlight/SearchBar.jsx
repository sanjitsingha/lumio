import { forwardRef, useLayoutEffect, useRef, useState } from "react";
import { LoaderCircle, Search } from "lucide-react";

/**
 * The input row. The typed text sits on a soft rounded highlight, and
 * `suggestion` (from search history) is drawn as ghost text after it,
 * accepted with → or End. Both are drawn by a copy of the text behind the
 * transparent input, kept in step with the input's horizontal scrolling.
 */
export const SearchBar = forwardRef(function SearchBar({ value, onChange, onKeyDown, loading, suggestion }, ref) {
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
    <div className="flex h-[58px] items-center gap-3 px-4">
      <Search size={22} strokeWidth={2} className="shrink-0 text-secondary" aria-hidden />
      <div className="relative min-w-0 flex-1">
        {value && (
          // starts 8 px left of the text (pl-2 puts the text back) so the highlight's
          // left padding is not clipped
          <div
            aria-hidden
            className="pointer-events-none absolute inset-y-0 right-0 -left-2 flex items-center overflow-hidden pl-2 whitespace-pre text-[21px] font-light tracking-[var(--lm-spotlight-tracking)]"
          >
            <span className="flex items-center" style={{ transform: `translateX(${-scroll}px)` }}>
              {/* padding and equal negative margin: the highlight grows around the text without moving it */}
              <span className="-mx-2 rounded-[8px] bg-fill-strong px-2 py-[3px] text-transparent">{value}</span>
              {ghost && <span className="text-tertiary">{ghost}</span>}
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
          className="relative w-full bg-transparent text-[21px] font-light tracking-[var(--lm-spotlight-tracking)] text-label outline-none placeholder:tracking-[0.01em] placeholder:text-tertiary"
        />
      </div>
      {loading && <LoaderCircle size={18} className="shrink-0 animate-spin text-secondary" aria-label="Searching" />}
    </div>
  );
});
