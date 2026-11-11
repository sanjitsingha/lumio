import { Check, File } from "lucide-react";

/** The rounded thumbnail of a copied image or of an image, video or PDF file. */
function Thumb({ src, title }) {
  return (
    <img
      src={src}
      alt=""
      draggable={false}
      title={title}
      className="block h-[62px] w-auto max-w-[120px] min-w-10 rounded-md object-cover"
    />
  );
}

/** How many files a copy holds, on the corner of its picture. */
function CountBadge({ count }) {
  if (count < 2) return null;
  return (
    <span className="absolute top-1 -right-1 rounded-full bg-white/25 px-1.5 text-[10px] leading-[15px] font-medium text-white backdrop-blur-sm">
      {count}
    </span>
  );
}

function Content({ clip }) {
  if (clip.kind === "image") return <Thumb src={clip.src} title={`Image, ${clip.width} × ${clip.height}`} />;
  if (clip.kind === "files") {
    const [first] = clip.files;
    const unlisted = clip.count - clip.files.length;
    const tooltip = clip.files.map((f) => f.path).join("\n") + (unlisted > 0 ? `\n…and ${unlisted} more` : "");
    // photos, videos and PDFs show what is in them, like a copied image
    if (first?.thumb) {
      return (
        <span className="relative block">
          <Thumb src={first.thumb} title={tooltip} />
          <CountBadge count={clip.count} />
        </span>
      );
    }
    return (
      <div className="flex w-[84px] flex-col items-center" title={tooltip}>
        <span className="relative">
          {first?.icon ? (
            <img src={first.icon} alt="" draggable={false} className="block h-12 w-12" />
          ) : (
            <File size={44} strokeWidth={1.25} className="m-0.5 text-white/60" />
          )}
          <CountBadge count={clip.count} />
        </span>
        <span className="block max-w-full truncate text-[11px] leading-[14px] text-white/55">{first?.name}</span>
      </div>
    );
  }
  return (
    <p
      className="m-0 line-clamp-5 w-[92px] break-words whitespace-pre-wrap text-[11px] leading-[12.4px] text-white/90"
      title={clip.text}
    >
      {clip.text}
    </p>
  );
}

/**
 * One copy in the notch's clipboard carousel, drawn straight on the panel:
 * images and files with a thumbnail as rounded pictures, other files as their
 * icon over the name, text as a few lines. About 62 px tall
 * (`THUMB_HEIGHT` in `notch/clipboard.rs` is twice that). `copied` shows a
 * brief confirmation after a click put it back on the clipboard.
 */
export function ClipItem({ clip, copied, ...gestures }) {
  return (
    <div
      {...gestures}
      className="relative flex h-[70px] items-center rounded-lg p-1 transition-colors hover:bg-white/[0.07] active:bg-white/[0.11]"
    >
      <Content clip={clip} />
      <span
        className={
          "pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-0.5 rounded-lg bg-black/60 text-[11px] font-medium text-white transition-opacity duration-150 " +
          (copied ? "opacity-100" : "opacity-0")
        }
      >
        <Check size={16} strokeWidth={2.5} />
        Copied
      </span>
    </div>
  );
}
