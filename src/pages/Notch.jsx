import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useAnimate } from "framer-motion";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { emit, listen } from "@tauri-apps/api/event";
import { ClipItem } from "../components/notch/ClipItem";
import { cx } from "../utils/cx";

/**
 * The notch (Windows only, see `notch/mod.rs`): nothing
 * shows until the backend reports the mouse resting at the top centre of the
 * screen; then a panel drops out of the screen's edge, showing the clipboard
 * history as a carousel.
 *
 * The panel is exactly as wide as its items, between `MIN_WIDTH` and
 * `MAX_WIDTH`; past that the row scrolls. The items are laid out once at that
 * final width and the panel's shape springs open around them, so nothing
 * inside moves while it opens or closes. Copying something while it is closed
 * opens it by itself for a moment to show the copy arriving. Sizes are CSS px
 * and match `MAX_WIDTH`, `HEIGHT` and `SHADOW_MARGIN` in `notch/mod.rs`.
 */
/** Closed: a transparent sliver at the screen's edge that the panel grows out of. */
const COLLAPSED = { width: 160, height: 0, radius: 8 };
/** The narrowest the open panel gets, with only a copy or two in it. */
const MIN_WIDTH = 240;
const EXPANDED_HEIGHT = 96;
const EXPANDED_RADIUS = 20;
const EMPTY_WIDTH = 300;
const MAX_WIDTH = 640;
/** The window's transparent margin beside and below the panel, for its shadow. */
const SHADOW_MARGIN = 40;
/** `MAX_CLIPS` in `notch/clipboard.rs`. */
const MAX_CLIPS = 50;
/** How far in from an edge items fade out while more of the row is hidden past it. */
const FADE = 84;
/** The fade's opacity at points across it: eased, so items melt away right at the edge. */
const FADE_CURVE = [
  [0, 0],
  [0.25, 0.08],
  [0.5, 0.32],
  [0.75, 0.7],
  [1, 1],
];
/** A press that moves this far (px) is a drag, not a click. */
const DRAG_DISTANCE = 5;
/** How long "Copied" shows on an item after a click. */
const COPIED_MS = 1100;
/**
 * A copy made while the notch is closed: it opens, the copy rises into the
 * row `RISE_DELAY_MS` later (pushing the others aside), wiggles as it lands,
 * and the notch closes again `SHOWCASE_MS` after the last copy.
 */
const SHOWCASE_MS = 2000;
const RISE_DELAY_MS = 260;
const WIGGLE_DELAY_MS = 380;

// Springs keep their speed when interrupted, so moving in and out quickly
// never jerks: a little bounce opening, like the Dynamic Island, none closing.
// It shows at once when opening, and fades only as it finishes closing, so it
// is never seen as a line at the top of the screen.
const OPEN_SPRING = { type: "spring", visualDuration: 0.42, bounce: 0.2, opacity: { duration: 0.08 } };
const CLOSE_SPRING = { type: "spring", visualDuration: 0.3, bounce: 0, opacity: { duration: 0.12, delay: 0.16 } };
const SHADOW_OPEN = "0 16px 36px -12px rgba(0,0,0,0.65), 0 4px 12px -6px rgba(0,0,0,0.4)";
const SHADOW_CLOSED = "0 0px 0px 0px rgba(0,0,0,0), 0 0px 0px 0px rgba(0,0,0,0)";
/** The items come in once the panel has mostly opened, and leave before it closes. */
const CONTENT_SHOWN = {
  opacity: 1,
  scale: 1,
  filter: "blur(0px)",
  transition: { delay: 0.07, duration: 0.3, ease: [0.22, 1, 0.36, 1] },
};
const CONTENT_HIDDEN = {
  opacity: 0,
  scale: 0.94,
  filter: "blur(6px)",
  transition: { duration: 0.12, ease: [0.4, 0, 1, 1] },
};
const ITEM_SPRING = { type: "spring", visualDuration: 0.4, bounce: 0.15 };
/** How items appear: popping in, or (a fresh copy) rising from below the panel's edge. */
const POP_IN = { opacity: 0, y: 0, scale: 0.8, filter: "blur(4px)" };
const FROM_BELOW = { opacity: 0, y: 84, scale: 0.9, filter: "blur(0px)" };
const SETTLED = { opacity: 1, y: 0, scale: 1, filter: "blur(0px)" };
const RISE_TRANSITION = { ...ITEM_SPRING, y: { type: "spring", visualDuration: 0.45, bounce: 0.32 } };
/** A fresh copy's little shake as it lands. */
const WIGGLE = { rotate: [0, -7, 6, -4, 2, 0], scale: [1, 1.07, 1.04, 1.02, 1, 1] };

/**
 * The clipboard history, newest first, kept in step with the backend.
 * Something copied is handed to `onCopied(clip)`, which puts it in the list
 * with `toFront` when it likes; one arriving quietly (on the clipboard at
 * start, or clicked in the notch) goes to the front at once.
 */
function useClips(onCopied) {
  const [clips, setClips] = useState([]);
  const copied = useRef(onCopied);
  copied.current = onCopied;
  // a copy made (or clicked) again comes back with its old id and moves to the front
  const toFront = useCallback(
    (clip) => setClips((old) => [clip, ...old.filter((c) => c.id !== clip.id)].slice(0, MAX_CLIPS)),
    [],
  );
  useEffect(() => {
    const stops = Promise.all([
      listen("notch-clips", (e) => setClips(e.payload)),
      listen("notch-clip", (e) => copied.current(e.payload)),
      listen("notch-clip-quiet", (e) => toFront(e.payload)),
    ]);
    // ask for the history only once the listeners are in place
    stops.then(() => emit("notch-ready"));
    return () => {
      stops.then((fns) => fns.forEach((stop) => stop()));
    };
  }, [toFront]);
  return { clips, toFront };
}

/**
 * A horizontally scrolling row: the wheel and `page(±1)` scroll it smoothly,
 * `edges` says on which sides part of it is hidden and `contentWidth` how
 * wide all of it is (whatever the row's own width). `shown`: it is rendered.
 */
function useCarousel(shown) {
  const row = useRef(null);
  const content = useRef(null);
  // where the current smooth scroll is heading, so quick wheel turns add up
  const target = useRef(0);
  const [edges, setEdges] = useState({ left: false, right: false });
  const [contentWidth, setContentWidth] = useState(0);

  const measure = useCallback(() => {
    const el = row.current;
    if (!el || !content.current) return;
    const max = el.scrollWidth - el.clientWidth;
    setEdges({ left: el.scrollLeft > 1, right: el.scrollLeft < max - 1 });
    setContentWidth(content.current.offsetWidth);
  }, []);

  useEffect(() => {
    if (!row.current || !content.current) return;
    const observer = new ResizeObserver(measure);
    observer.observe(row.current);
    observer.observe(content.current);
    return () => observer.disconnect();
  }, [measure, shown]);

  const scrollTo = (left) => {
    const el = row.current;
    if (!el) return;
    target.current = Math.max(0, Math.min(el.scrollWidth - el.clientWidth, left));
    el.scrollTo({ left: target.current, behavior: "smooth" });
  };

  return {
    row,
    content,
    edges,
    contentWidth,
    onScroll: measure,
    onWheel: (e) => scrollTo(target.current + e.deltaY + e.deltaX),
    page: (direction) => scrollTo(target.current + direction * (row.current?.clientWidth ?? 0) * 0.7),
    reset: () => {
      target.current = 0;
      row.current?.scrollTo({ left: 0 });
    },
  };
}

/**
 * Click puts a copy back on the clipboard; pressing and moving drags a file
 * or image out (the backend runs the drag). Returns the pointer handlers for
 * one item.
 */
function useClipGestures(onCopy) {
  const press = useRef(null);
  return (clip) => ({
    onPointerDown: (e) => {
      if (e.button === 0) press.current = { id: clip.id, x: e.clientX, y: e.clientY, dragged: false };
    },
    onPointerMove: (e) => {
      const p = press.current;
      if (!p || p.id !== clip.id || p.dragged || !(e.buttons & 1)) return;
      if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_DISTANCE) return;
      p.dragged = true;
      // text is copied by clicking only
      if (clip.kind !== "text") emit("notch-drag", { id: clip.id });
    },
    onPointerUp: () => {
      const p = press.current;
      press.current = null;
      if (p && p.id === clip.id && !p.dragged) onCopy(clip);
    },
  });
}

/** Items fade out towards each edge that has more of the row behind it. */
function fadeMask({ left, right }) {
  const leftStops = left ? FADE_CURVE.map(([at, alpha]) => `rgba(0,0,0,${alpha}) ${at * FADE}px`) : ["#000 0px"];
  const rightStops = right
    ? [...FADE_CURVE].reverse().map(([at, alpha]) => `rgba(0,0,0,${alpha}) calc(100% - ${at * FADE}px)`)
    : ["#000 100%"];
  return `linear-gradient(to right, ${[...leftStops, ...rightStops].join(", ")})`;
}

/** A thin white chevron over the faded edge, shown while more of the row is hidden past it. */
function Arrow({ side, visible, onClick }) {
  const Icon = side === "left" ? ChevronLeft : ChevronRight;
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={side === "left" ? "Newer copies" : "Older copies"}
      onClick={onClick}
      className={cx(
        "absolute top-1/2 flex h-10 w-7 -translate-y-1/2 items-center justify-center text-white/85 drop-shadow-[0_1px_3px_rgba(0,0,0,0.6)] transition-[opacity,color,transform] duration-200 hover:scale-110 hover:text-white active:scale-90",
        side === "left" ? "left-2" : "right-2",
        visible ? "opacity-100" : "pointer-events-none opacity-0",
      )}
    >
      <Icon size={22} strokeWidth={1.75} />
    </button>
  );
}

/**
 * Shows each copy arriving: while the notch is closed it opens by itself
 * (`showing`), the copy goes into the row once the panel is open and wiggles
 * as it lands, and the notch closes `SHOWCASE_MS` after the last copy. The
 * mouse opening the notch takes over. `rising` is the id of the copy that
 * comes up from below.
 */
function useShowcase(expanded, toFront, wiggle) {
  const [showing, setShowing] = useState(false);
  const rising = useRef(null);
  const open = useRef(expanded);
  open.current = expanded;
  const showingRef = useRef(false);
  const openedAt = useRef(0);
  const endTimer = useRef(0);
  const pending = useRef(new Set());

  const later = (ms, run) => {
    const timer = setTimeout(() => {
      pending.current.delete(timer);
      run();
    }, ms);
    pending.current.add(timer);
  };

  const copied = useCallback(
    (clip) => {
      const arrive = () => {
        rising.current = clip.id;
        toFront(clip);
        later(WIGGLE_DELAY_MS, () => wiggle(clip.id));
      };
      if (open.current) {
        arrive();
        return;
      }
      if (!showingRef.current) {
        showingRef.current = true;
        openedAt.current = performance.now();
        setShowing(true);
      }
      // copies arriving while it is still opening wait their turn, in order
      later(Math.max(0, openedAt.current + RISE_DELAY_MS - performance.now()), arrive);
      clearTimeout(endTimer.current);
      endTimer.current = setTimeout(() => {
        showingRef.current = false;
        setShowing(false);
      }, SHOWCASE_MS);
    },
    [toFront, wiggle],
  );

  useEffect(() => {
    if (!expanded) return;
    clearTimeout(endTimer.current);
    showingRef.current = false;
    setShowing(false);
  }, [expanded]);
  useEffect(
    () => () => {
      clearTimeout(endTimer.current);
      pending.current.forEach(clearTimeout);
    },
    [],
  );
  return { showing, copied, rising };
}

export default function NotchPage() {
  const [expanded, setExpanded] = useState(false);
  const [copiedId, setCopiedId] = useState(null);
  const onCopied = useRef(null);
  const { clips, toFront } = useClips((clip) => onCopied.current?.(clip));
  const carousel = useCarousel(clips.length > 0);
  const [scope, animate] = useAnimate();
  const wiggle = useCallback(
    (id) => {
      const item = scope.current?.querySelector(`[data-clip-id="${id}"]`);
      if (item) animate(item, WIGGLE, { duration: 0.6, ease: "easeInOut" });
    },
    [scope, animate],
  );
  const showcase = useShowcase(expanded, toFront, wiggle);
  onCopied.current = showcase.copied;
  // open for the mouse, or by itself to show a copy arriving
  const open = expanded || showcase.showing;

  const copiedTimer = useRef(0);
  const gestures = useClipGestures((clip) => {
    emit("notch-copy", { id: clip.id });
    setCopiedId(clip.id);
    clearTimeout(copiedTimer.current);
    copiedTimer.current = setTimeout(() => setCopiedId(null), COPIED_MS);
  });

  useEffect(() => {
    const unlisten = listen("notch-expanded", (e) => setExpanded(Boolean(e.payload)));
    return () => {
      unlisten.then((stop) => stop());
      clearTimeout(copiedTimer.current);
    };
  }, []);

  // the next open starts at the newest copy again; once the items have faded, so it never shows
  useEffect(() => {
    if (open) return;
    const timer = setTimeout(carousel.reset, 300);
    return () => clearTimeout(timer);
  }, [open]);

  // while it shows a copy by itself, reaching for the panel opens it for the mouse at once
  useEffect(() => {
    emit("notch-showcase", showcase.showing);
  }, [showcase.showing]);

  // as wide as the items, between `MIN_WIDTH` and the most the screen gives it
  const width = clips.length
    ? Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, carousel.contentWidth)))
    : EMPTY_WIDTH;
  // the backend collapses the panel once the mouse leaves its width
  useEffect(() => {
    emit("notch-width", width);
  }, [width]);

  const outline = (w, h, radius, boxShadow, opacity) => ({
    width: w,
    height: h,
    borderBottomLeftRadius: radius,
    borderBottomRightRadius: radius,
    boxShadow,
    // fully hidden when closed: even at no height its bottom rim would show as a line
    opacity,
  });
  const shape = open
    ? outline(width, EXPANDED_HEIGHT, EXPANDED_RADIUS, SHADOW_OPEN, 1)
    : outline(COLLAPSED.width, COLLAPSED.height, COLLAPSED.radius, SHADOW_CLOSED, 0);
  const mask = fadeMask(carousel.edges);
  return (
    <div className="flex h-full items-start justify-center" style={{ paddingInline: SHADOW_MARGIN }}>
      <motion.div
        // the top edge sits on the screen's edge, so only the sides and bottom get the rim
        className="relative shrink-0 overflow-hidden border border-t-0 border-white/15 bg-[#1c1c1e]"
        initial={false}
        animate={shape}
        transition={open ? OPEN_SPRING : CLOSE_SPRING}
      >
        {/* laid out at the final width and centred, so the opening panel only reveals it */}
        <motion.div
          ref={scope}
          className="absolute top-0 left-1/2"
          style={{ width, height: EXPANDED_HEIGHT, x: "-50%", transformOrigin: "50% 0%" }}
          initial={false}
          animate={open ? CONTENT_SHOWN : CONTENT_HIDDEN}
        >
          {clips.length === 0 ? (
            <div className="flex h-full items-center justify-center text-[12px] text-white/50">
              Copy something and it shows up here
            </div>
          ) : (
            <>
              <div
                ref={carousel.row}
                onScroll={carousel.onScroll}
                onWheel={carousel.onWheel}
                className="h-full overflow-x-auto [scrollbar-width:none]"
                style={{ maskImage: mask, WebkitMaskImage: mask }}
              >
                <div
                  ref={carousel.content}
                  // centred while the items need less than the strip's width
                  className="mx-auto flex h-full w-max items-center gap-3 px-6"
                >
                  <AnimatePresence initial={false} mode="popLayout">
                    {clips.map((clip) => {
                      const rises = showcase.rising.current === clip.id;
                      return (
                        <motion.div
                          key={clip.id}
                          // the others glide aside as a copy pushes in at the front
                          layout="position"
                          initial={rises ? FROM_BELOW : POP_IN}
                          animate={SETTLED}
                          exit={POP_IN}
                          transition={rises ? RISE_TRANSITION : ITEM_SPRING}
                          className="shrink-0"
                        >
                          {/* its own element, so the wiggle never fights the layout animation */}
                          <div data-clip-id={clip.id}>
                            <ClipItem clip={clip} copied={copiedId === clip.id} {...gestures(clip)} />
                          </div>
                        </motion.div>
                      );
                    })}
                  </AnimatePresence>
                </div>
              </div>
              <Arrow side="left" visible={carousel.edges.left} onClick={() => carousel.page(-1)} />
              <Arrow side="right" visible={carousel.edges.right} onClick={() => carousel.page(1)} />
            </>
          )}
        </motion.div>
      </motion.div>
    </div>
  );
}
