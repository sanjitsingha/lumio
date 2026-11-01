import { forwardRef } from "react";
import { CornerDownLeft } from "lucide-react";
import { ResultIcon } from "./ResultIcon";
import { cx } from "../../utils/cx";

const KIND_LABEL = { app: "Application", action: "Action", folder: "Folder", file: "File" };

export const ResultRow = forwardRef(function ResultRow({ result, selected, onHover, onOpen }, ref) {
  return (
    <div
      ref={ref}
      role="option"
      aria-selected={selected}
      onMouseMove={onHover}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onOpen}
      className={cx(
        "mx-2 flex h-[46px] items-center gap-3 rounded-[9px] px-2.5",
        selected ? "bg-accent text-on-accent" : "text-label",
      )}
    >
      <span className="flex h-[30px] w-[30px] shrink-0 items-center justify-center">
        <ResultIcon icon={result.icon} selected={selected} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[14px] leading-[18px] font-medium">{result.title}</span>
        <span
          className={cx(
            "block truncate text-[12px] leading-[16px]",
            selected ? "text-on-accent/80" : "text-secondary",
          )}
          title={result.subtitle}
        >
          {selected && result.note ? result.note : result.subtitle || KIND_LABEL[result.kind]}
        </span>
      </span>
      {selected && (
        <span className="flex shrink-0 items-center gap-1 text-[11px] opacity-85">
          <CornerDownLeft size={13} strokeWidth={2.2} />
        </span>
      )}
    </div>
  );
});
