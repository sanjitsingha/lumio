import { forwardRef } from "react";
import { LoaderCircle, Search } from "lucide-react";

/**
 * The input row. `suggestion` (from search history) is drawn as ghost text
 * after the typed query and accepted with → or End.
 */
export const SearchBar = forwardRef(function SearchBar({ value, onChange, onKeyDown, loading, suggestion }, ref) {
  const ghost =
    suggestion && value && suggestion.toLowerCase().startsWith(value.toLowerCase()) && suggestion.length > value.length
      ? suggestion.slice(value.length)
      : "";
  return (
    <div className="flex h-[58px] items-center gap-3 px-4">
      <Search size={22} strokeWidth={2} className="shrink-0 text-secondary" aria-hidden />
      <div className="relative min-w-0 flex-1">
        {ghost && (
          <div
            aria-hidden
            className="pointer-events-none absolute inset-0 flex items-center overflow-hidden whitespace-pre text-[21px] font-light tracking-[-0.01em]"
          >
            <span className="invisible">{value}</span>
            <span className="text-tertiary">{ghost}</span>
          </div>
        )}
        <input
          ref={ref}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Search anything..."
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          aria-label="Search"
          className="relative w-full bg-transparent text-[21px] font-light tracking-[-0.01em] text-label outline-none placeholder:text-tertiary"
        />
      </div>
      {loading && <LoaderCircle size={18} className="shrink-0 animate-spin text-secondary" aria-label="Searching" />}
    </div>
  );
});
