import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { Check, File, Film, Image as ImageIcon, Link2 } from "lucide-react";
import { api, errorMessage } from "../../services/tauri";
import { cx } from "../../utils/cx";
import { iconUrl } from "./ResultIcon";

/** The section's height: border 1 + padding 24 + chips 24 + gap 12 + cards 120. The results list makes room for it. */
export const QUICK_PANEL_HEIGHT = 181;
/** How often the open section looks for new copies and screenshots. */
const REFRESH_MS = 1000;
/** How long "Copied" shows before Spotlight closes, handing focus back for the paste. */
const COPIED_MS = 450;
/** `kinds` are what a chip shows; All has none, so it shows everything. */
const FILTERS = [
  { label: "All", kinds: null, empty: "Copy something and it will appear here" },
  { label: "Text", kinds: ["text", "link"], empty: "No text yet" },
  { label: "Images", kinds: ["image"], empty: "No images yet" },
  { label: "Video", kinds: ["video"], empty: "No videos yet" },
  { label: "Files", kinds: ["file"], empty: "No files yet" },
];

const fromClip = (clip) => ({
  ...clip,
  key: `clip:${clip.id}`,
  src: clip.kind === "image" || clip.kind === "video" ? `${iconUrl("clip", clip.id)}?v=${clip.at}` : null,
  copy: () => api.copyClip(clip.id),
});

const fromShot = (shot) => ({
  kind: "image",
  key: `shot:${shot.name}`,
  at: shot.modified,
  name: shot.name,
  count: 1,
  src: `${iconUrl("screenshot", shot.key)}?v=${shot.modified}`,
  copy: () => api.copyScreenshot(shot.name),
});

function hostOf(link) {
  try {
    return new URL(link.trim()).hostname.replace(/^www\./, "");
  } catch {
    return link;
  }
}

/** A file (or a picture that could not be shown): its type's icon over its name. */
function FileCard({ item }) {
  const Icon = item.kind === "video" ? Film : item.kind === "image" ? ImageIcon : File;
  return (
    <span className="flex h-full w-full flex-col items-center justify-center gap-2 px-3 text-secondary">
      <Icon size={30} strokeWidth={1.25} aria-hidden />
      <span className="line-clamp-2 max-w-full text-center text-[11px] leading-[14px] break-words text-label">
        {item.name ?? "Image"}
      </span>
    </span>
  );
}

function Preview({ item }) {
  const [broken, setBroken] = useState(false);
  if (item.src && !broken) {
    return (
      <>
        <img
          src={item.src}
          alt=""
          draggable={false}
          loading="lazy"
          onError={() => setBroken(true)}
          className="block h-full w-full object-cover"
        />
        {item.kind === "video" && (
          <span className="absolute bottom-1.5 left-1.5 rounded-[3px] bg-black/60 p-1 text-white">
            <Film size={12} aria-hidden />
          </span>
        )}
      </>
    );
  }
  if (item.kind === "text") {
    return (
      <span className="line-clamp-7 px-2.5 pt-2 text-left text-[11px] leading-[15px] break-words whitespace-pre-wrap text-label">
        {item.text}
      </span>
    );
  }
  if (item.kind === "link") {
    return (
      <span className="flex h-full flex-col justify-center gap-1 px-2.5 text-left">
        <span className="flex items-center gap-1.5 text-[12px] leading-[16px] font-medium text-label">
          <Link2 size={13} className="shrink-0" aria-hidden />
          <span className="truncate">{hostOf(item.text)}</span>
        </span>
        <span className="line-clamp-4 text-[10.5px] leading-[14px] break-all text-secondary">{item.text}</span>
      </span>
    );
  }
  return <FileCard item={item} />;
}

/**
 * Quick files, the section that opens below Spotlight with Tab: everything
 * copied (text, links, files, images, videos) and every screenshot, newest
 * first. The chips narrow it to one
 * kind, and a click on a card copies it again. A press here must not take
 * focus from the search field, which handles the keyboard.
 */
export function QuickPanel({ onError }) {
  const [items, setItems] = useState(null);
  const [filter, setFilter] = useState(FILTERS[0]);
  const [copied, setCopied] = useState(null);
  const row = useRef(null);
  const mounted = useRef(false);
  const copying = useRef(false);
  const closeTimer = useRef(null);

  useEffect(() => {
    mounted.current = true;
    let alive = true;
    const load = async () => {
      const [clips, shots] = await Promise.all([api.getClips().catch(() => []), api.getScreenshots().catch(() => [])]);
      // a copy moves to the front: not while its "Copied" is showing
      if (!alive || copying.current) return;
      setItems([...clips.map(fromClip), ...shots.map(fromShot)].sort((a, b) => b.at - a.at));
    };
    load();
    const timer = setInterval(load, REFRESH_MS);
    return () => {
      alive = false;
      mounted.current = false;
      clearInterval(timer);
      clearTimeout(closeTimer.current);
    };
  }, []);

  const copy = async (item) => {
    if (copying.current) return;
    copying.current = true;
    try {
      await item.copy();
      if (!mounted.current) return;
      setCopied(item.key);
      closeTimer.current = setTimeout(() => api.hideSpotlight(), COPIED_MS);
    } catch (e) {
      copying.current = false;
      onError(errorMessage(e));
    }
  };

  const choose = (next) => {
    setFilter(next);
    if (row.current) row.current.scrollLeft = 0;
  };

  const visible = filter.kinds ? items?.filter((item) => filter.kinds.includes(item.kind)) : items;

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.1 }}
      onMouseDown={(e) => e.preventDefault()}
      style={{ height: QUICK_PANEL_HEIGHT }}
      className="flex flex-col gap-3 border-t border-separator py-3"
    >
      <div className="flex h-6 shrink-0 items-center gap-1 px-[14px]">
        {FILTERS.map((chip) => (
          <button
            key={chip.label}
            type="button"
            tabIndex={-1}
            aria-pressed={chip === filter}
            onClick={() => choose(chip)}
            className={cx(
              "h-6 rounded-[4px] px-2.5 text-[11px] font-medium transition-colors",
              chip === filter ? "bg-fill-strong text-label" : "text-secondary hover:bg-fill",
            )}
          >
            {chip.label}
          </button>
        ))}
      </div>
      {visible?.length === 0 ? (
        <div className="flex h-[120px] items-center justify-center text-[11px] text-secondary">{filter.empty}</div>
      ) : (
        <div
          ref={row}
          // a mouse wheel moves the row sideways
          onWheel={(e) => (e.currentTarget.scrollLeft += e.deltaY)}
          className="flex h-[120px] gap-2 overflow-x-auto px-[14px] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
        >
          {visible?.map((item) => (
            <button
              key={item.key}
              type="button"
              tabIndex={-1}
              title={item.text ?? item.name ?? "Copied image"}
              aria-label={`Copy ${item.kind}: ${item.text ?? item.name ?? "image"}`}
              onClick={() => copy(item)}
              className="relative block h-[120px] w-[192px] shrink-0 overflow-hidden rounded-[4px] border border-separator bg-fill transition-opacity hover:opacity-80"
            >
              <Preview item={item} />
              {item.count > 1 && (
                <span className="absolute top-1.5 right-1.5 rounded-full bg-black/60 px-1.5 text-[10px] leading-[15px] font-medium text-white">
                  {item.count}
                </span>
              )}
              {copied === item.key && (
                <span className="absolute inset-0 flex flex-col items-center justify-center gap-1 bg-black/70 text-[12px] font-medium text-white">
                  <Check size={18} strokeWidth={2.5} aria-hidden />
                  Copied
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </motion.div>
  );
}
