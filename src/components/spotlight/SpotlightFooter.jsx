import { LoaderCircle } from "lucide-react";

/** File-index progress below the results; nothing at all once the index is ready. */
export function SpotlightFooter({ index }) {
  const status = index?.indexing ? (
    <>
      <LoaderCircle size={11} className="animate-spin" />
      Indexing files…
    </>
  ) : index && !index.ready && index.files === 0 ? (
    "Preparing file search…"
  ) : null;
  if (!status) return null;
  return (
    <div className="flex h-[30px] items-center gap-1.5 border-t border-separator px-4 text-[11px] text-secondary">
      {status}
    </div>
  );
}
