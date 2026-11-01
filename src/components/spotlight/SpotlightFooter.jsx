import { LoaderCircle } from "lucide-react";

function Hint({ keys, label }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className="flex gap-0.5">
        {keys.map((k) => (
          <kbd
            key={k}
            className="min-w-[18px] rounded-[4px] bg-fill-strong px-1 text-center font-sans text-[10.5px] leading-[17px] text-secondary"
          >
            {k}
          </kbd>
        ))}
      </span>
      <span>{label}</span>
    </span>
  );
}

export function SpotlightFooter({ index, canReveal, multipleSections }) {
  return (
    <div className="flex h-[30px] items-center justify-between border-t border-separator px-4 text-[11px] text-secondary">
      <span className="flex items-center gap-1.5">
        {index?.indexing ? (
          <>
            <LoaderCircle size={11} className="animate-spin" />
            Indexing files…
          </>
        ) : index && !index.ready && index.files === 0 ? (
          "Preparing file search…"
        ) : null}
      </span>
      <span className="flex items-center gap-3.5">
        <Hint keys={["↵"]} label="Open" />
        {canReveal && <Hint keys={["Ctrl", "↵"]} label="Show in Folder" />}
        {multipleSections && <Hint keys={["Tab"]} label="Next Section" />}
        <Hint keys={["Esc"]} label="Close" />
      </span>
    </div>
  );
}
