"use client";

import Image from "next/image";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

/*
 * Lumio's Spotlight playing on a laptop screen, in a loop: it opens, a few
 * searches are typed one after another (results follow every keystroke), and
 * it closes again. The panel's sizes and colours
 * are copied from the app (src/components/spotlight in the Lumio repo); the
 * motion is softer than the app's, for the page.
 */

/** The laptop picture (public/lumio/laptop.png) and the screen inside it, in image pixels. */
const LAPTOP = { width: 1488, height: 831 };
const SCREEN = { x: 180, y: 21, width: 1128, height: 754 };

/** The scene is laid out on a desktop this wide, then scaled to the screen: smaller makes the panel bigger. */
const DESKTOP_WIDTH = 1152;
const DESKTOP_HEIGHT = (DESKTOP_WIDTH * SCREEN.height) / SCREEN.width;
/** Lumio's window opens 22% down the screen; the panel sits 36 px inside it. */
const PANEL_TOP = Math.round(DESKTOP_HEIGHT * 0.22) + 36;
const PANEL_WIDTH = 680;

/** Settles gently, without a bounce. */
const EASE_OUT = "cubic-bezier(0.22, 1, 0.36, 1)";
const EASE_IN = "cubic-bezier(0.4, 0, 1, 1)";
/** How long the panel's height, moving rows and the selection take to settle. */
const MOVE_MS = 380;
const FONT = 'system-ui, -apple-system, "Inter", "Segoe UI Variable Text", "Segoe UI", "Cantarell", "Ubuntu", sans-serif';

/** Typed one after another while Spotlight stays open; each is selected and typed over by the next. */
const QUERIES = ["Chrome", "Terminal", "invoice", "249*12", "who is the pm of india"];
/** How long each search's results stay up before the next one. */
const HOLD_MS = 1800;

/** The demo's "computer": what Lumio finds. `icon` may also be an image in public/, e.g. "/lumio/spotify.png". */
const APPS = [
  { title: "Chrome", subtitle: "Application", icon: "chrome" },
  { title: "Calculator", subtitle: "Application", icon: "calculator" },
  { title: "Terminal", subtitle: "Application", icon: "terminal" },
];
const FOLDERS = [{ title: "Invoices", subtitle: "~\\Documents", icon: "folder" }];
const FILES = [
  { title: "chrome-logo.png", subtitle: "~\\Pictures", icon: "image" },
  { title: "Chrome setup guide.pdf", subtitle: "~\\Documents", icon: "pdf" },
  { title: "Invoice March 2026.pdf", subtitle: "~\\Documents\\Invoices", icon: "pdf" },
  { title: "Invoice template.docx", subtitle: "~\\Documents", icon: "document" },
  { title: "Weekly report.docx", subtitle: "~\\Documents", icon: "document" },
];

const matches = (title, text) => title.toLowerCase().split(/[\s._-]+/).some((word) => word.startsWith(text));

/** Sections in Lumio's order: Calculator, Top Hit, Applications, Folders, Files, Web. */
function search(query) {
  const raw = query.trim();
  if (!raw) return [];
  const text = raw.toLowerCase();
  const sections = [];
  const calc = calculate(raw);
  if (calc) sections.push({ title: "Calculator", rows: [{ key: "calc", kind: "calc", ...calc }] });
  const apps = APPS.filter((a) => matches(a.title, text));
  // a strong match is promoted to Top Hit (and not repeated under Applications)
  if (text.length >= 3 && apps[0]?.title.toLowerCase().startsWith(text)) {
    sections.push({ title: "Top Hit", rows: [apps.shift()] });
  }
  if (apps.length) sections.push({ title: "Applications", rows: apps });
  const folders = FOLDERS.filter((f) => matches(f.title, text));
  if (folders.length) sections.push({ title: "Folders", rows: folders });
  const files = FILES.filter((f) => matches(f.title, text));
  if (files.length) sections.push({ title: "Files", rows: files });
  // the browser chosen in Lumio's settings (Chrome) supplies the icon
  sections.push({
    title: "Web",
    rows: [{ key: "web", title: `Search Google for “${raw}”`, subtitle: "Web search", icon: "chrome" }],
  });
  return sections;
}

/** "249*12" -> { expression: "249 × 12", answer: "2988" }; null unless it is a complete sum. */
function calculate(text) {
  const compact = text.replace(/\s+/g, "");
  const tokens = compact.match(/\d+(?:\.\d+)?|[-+*/]/g);
  if (!tokens || tokens.join("") !== compact || tokens.length < 3 || tokens.length % 2 === 0) return null;
  if (tokens.some((t, i) => (i % 2 === 0) !== /\d/.test(t))) return null;
  // × and ÷ first, then + and -
  const sums = [Number(tokens[0])];
  for (let i = 1; i < tokens.length; i += 2) {
    const n = Number(tokens[i + 1]);
    const op = tokens[i];
    if (op === "*") sums.push(sums.pop() * n);
    else if (op === "/") sums.push(sums.pop() / n);
    else sums.push(op === "-" ? -n : n);
  }
  const value = sums.reduce((a, b) => a + b, 0);
  if (!Number.isFinite(value)) return null;
  const expression = tokens.map((t) => ({ "*": " × ", "/": " ÷ ", "+": " + ", "-": " - " })[t] ?? t).join("");
  return { expression, answer: String(Number(value.toPrecision(12))) };
}

/** Milliseconds before the next character: a little uneven, like a person typing. */
const RHYTHM = [0, 35, -20, 50, 10, -25, 70, 5, -10, 40];
const typingDelay = (char, i) => 95 + RHYTHM[i % RHYTHM.length] + (char === " " ? 60 : 0);

// panel: "hidden" | "open" | "closing"; selectAll: the typed text is selected, about to be typed over
const CLOSED = { panel: "hidden", query: "", selectAll: false };

export default function LumioSpotlightDemo({
  src = "/lumio/laptop.png",
  alt = "Lumio Spotlight searching apps, files and the web on a Windows laptop",
  className = "",
  priority = false,
}) {
  const root = useRef(null);
  const screen = useRef(null);
  const [scale, setScale] = useState(0);
  const [active, setActive] = useState(false);
  const [ui, setUi] = useState(CLOSED);
  const sections = useMemo(() => search(ui.query), [ui.query]);

  // the scene is drawn at desktop size and scaled to however wide the screen is
  useEffect(() => {
    const el = screen.current;
    const observer = new ResizeObserver(() => setScale(el.clientWidth / DESKTOP_WIDTH));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // plays only while on screen, and starts from the beginning when scrolled back to
  useEffect(() => {
    const observer = new IntersectionObserver(([entry]) => setActive(entry.isIntersecting), { threshold: 0.35 });
    observer.observe(root.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!active) return undefined;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setUi({ ...CLOSED, panel: "open", query: QUERIES[0] });
      return () => setUi(CLOSED);
    }

    const timers = new Set();
    // a cancelled wait never resolves, which stops the loop where it is
    const wait = (ms) =>
      new Promise((resolve) => {
        const id = setTimeout(() => timers.delete(id) && resolve(), ms);
        timers.add(id);
      });

    const play = async () => {
      await wait(1000);
      setUi({ ...CLOSED, panel: "open" });
      await wait(700);
      for (const [n, query] of QUERIES.entries()) {
        if (n > 0) {
          // select everything; the next key typed replaces it
          setUi((s) => ({ ...s, selectAll: true }));
          await wait(450);
        }
        for (let i = 0; i < query.length; i++) {
          // like Lumio, every keystroke brings new results
          setUi((s) => ({ ...s, query: query.slice(0, i + 1), selectAll: false }));
          await wait(typingDelay(query[i + 1], i));
        }
        await wait(HOLD_MS);
      }
      setUi((s) => ({ ...s, panel: "closing" }));
      // like Lumio, the next opening starts with an empty box
      await wait(260);
      setUi(CLOSED);
      await wait(400);
    };

    (async () => {
      for (;;) await play();
    })();
    return () => {
      timers.forEach(clearTimeout);
      setUi(CLOSED);
    };
  }, [active]);

  return (
    <div ref={root} className={`relative w-full select-none ${className}`}>
      <Image
        src={src}
        alt={alt}
        width={LAPTOP.width}
        height={LAPTOP.height}
        priority={priority}
        draggable={false}
        className="block h-auto w-full"
      />
      <div
        ref={screen}
        aria-hidden
        className="pointer-events-none absolute overflow-hidden"
        style={{
          left: `${(SCREEN.x / LAPTOP.width) * 100}%`,
          top: `${(SCREEN.y / LAPTOP.height) * 100}%`,
          width: `${(SCREEN.width / LAPTOP.width) * 100}%`,
          height: `${(SCREEN.height / LAPTOP.height) * 100}%`,
        }}
      >
        <div
          className="relative antialiased"
          style={{
            width: DESKTOP_WIDTH,
            height: DESKTOP_HEIGHT,
            transform: `scale(${scale})`,
            transformOrigin: "0 0",
            visibility: scale ? "visible" : "hidden",
            fontFamily: FONT,
          }}
        >
          <Panel ui={ui} sections={sections} />
        </div>
      </div>
      <style>{`
        @keyframes lumio-blink { 0%, 50% { opacity: 1 } 50.01%, 100% { opacity: 0 } }
        @keyframes lumio-fade-in { from { opacity: 0 } to { opacity: 1 } }
        @keyframes lumio-row-in { from { opacity: 0; transform: translateY(6px) } to { opacity: 1; transform: translateY(0) } }
      `}</style>
    </div>
  );
}

const PANEL_MOTION = {
  hidden: { opacity: 0, transform: "translateY(-10px) scale(0.96)", transition: "none" },
  open: {
    opacity: 1,
    transform: "translateY(0px) scale(1)",
    transition: `opacity 260ms ${EASE_OUT}, transform 480ms ${EASE_OUT}`,
  },
  closing: {
    opacity: 0,
    transform: "translateY(-4px) scale(0.97)",
    transition: `opacity 220ms ${EASE_IN}, transform 220ms ${EASE_IN}`,
  },
};

function Panel({ ui, sections }) {
  // deeper than the app's shadow, which must fade out within its window's 36 px margin
  return (
    <div
      className="absolute overflow-hidden rounded-[16px] border border-black/[0.14] bg-white/[0.96] text-[#1d1d1f] shadow-[0_50px_100px_-16px_rgba(0,0,0,0.6),0_24px_48px_-10px_rgba(0,0,0,0.38),0_4px_12px_rgba(0,0,0,0.14)]"
      style={{
        left: (DESKTOP_WIDTH - PANEL_WIDTH) / 2,
        top: PANEL_TOP,
        width: PANEL_WIDTH,
        transformOrigin: "50% 0",
        ...PANEL_MOTION[ui.panel],
      }}
    >
      <AutoHeight animate={ui.panel !== "hidden"}>
        <SearchBar query={ui.query} selectAll={ui.selectAll} />
        {sections.length > 0 && (
          <div className="border-t border-black/[0.08]" style={{ animation: `lumio-fade-in 220ms ${EASE_OUT} both` }}>
            <Results sections={sections} />
          </div>
        )}
      </AutoHeight>
    </div>
  );
}

/** Follows its contents' height smoothly as results come and go. */
function AutoHeight({ animate, children }) {
  const inner = useRef(null);
  const [height, setHeight] = useState(null);
  useEffect(() => {
    const el = inner.current;
    const observer = new ResizeObserver(() => setHeight(el.offsetHeight));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return (
    <div
      style={{
        height: height ?? "auto",
        overflow: "hidden",
        transition: animate ? `height ${MOVE_MS}ms ${EASE_OUT}` : "none",
      }}
    >
      <div ref={inner}>{children}</div>
    </div>
  );
}

/**
 * The result list. Rows that stay glide to their new place, new rows fade in,
 * and the blue selection slides along with the first row (Lumio selects it).
 */
function Results({ sections }) {
  const selected = 0;
  const list = useRef(null);
  const tops = useRef(new Map());
  const [pill, setPill] = useState(null);

  useLayoutEffect(() => {
    const next = new Map();
    for (const item of list.current.querySelectorAll("[data-flip]")) {
      const top = item.offsetTop;
      const before = tops.current.get(item.dataset.flip);
      next.set(item.dataset.flip, top);
      // drawn where it was, then eased to where it is now
      if (before !== undefined && before !== top) {
        item.animate([{ transform: `translateY(${before - top}px)` }, { transform: "translateY(0)" }], {
          duration: MOVE_MS,
          easing: EASE_OUT,
        });
      }
    }
    tops.current = next;

    const row = list.current.querySelector(`[data-row="${selected}"]`);
    if (!row) return;
    const top = row.offsetTop;
    const height = row.offsetHeight;
    // the first selection appears in place; later ones slide
    setPill((p) => (p?.top === top && p.height === height ? p : { top, height, glide: p !== null }));
  }, [sections]);

  let index = 0;
  return (
    <div ref={list} className="relative pb-2">
      {pill && (
        <div
          className="absolute left-2 right-2 top-0 rounded-[9px] bg-[#007aff]"
          style={{
            height: pill.height,
            transform: `translateY(${pill.top}px)`,
            transition: pill.glide ? `transform ${MOVE_MS}ms ${EASE_OUT}, height ${MOVE_MS}ms ${EASE_OUT}` : "none",
          }}
        />
      )}
      {/* one flat list, so a row keeps its element when it moves to another section */}
      {sections.flatMap((section) => [
        <div
          key={`h:${section.title}`}
          data-flip={`h:${section.title}`}
          className="relative px-[22px] pb-1 pt-2.5 text-[11px] font-semibold tracking-[0.01em] text-[#86868b]"
          style={{ animation: `lumio-fade-in 260ms ${EASE_OUT} both` }}
        >
          {section.title}
        </div>,
        ...section.rows.map((row) => {
          const key = `r:${row.key ?? row.title}`;
          const i = index++;
          return <Row key={key} flip={key} index={i} row={row} selected={i === selected} />;
        }),
      ])}
    </div>
  );
}

function SearchBar({ query, selectAll }) {
  return (
    <div className="flex h-[58px] items-center gap-3 px-4">
      <Glyph size={22} strokeWidth={2} className="shrink-0 text-[#86868b]">
        <path d="m21 21-4.34-4.34" />
        <circle cx="11" cy="11" r="8" />
      </Glyph>
      <div className="min-w-0 flex-1 whitespace-pre text-[21px] font-light leading-[1.5] tracking-[-0.01em]">
        {query ? (
          // the typed text sits on a soft rounded highlight
          <span className="-mx-2 inline-block rounded-[8px] bg-black/[0.08] px-2 py-[3px]">
            {selectAll ? (
              // Lumio's text selection colour; a selection hides the caret
              <span className="bg-[rgba(0,122,255,0.3)]">{query}</span>
            ) : (
              <>
                {query}
                <Caret key={query} gap={1} />
              </>
            )}
          </span>
        ) : (
          <span className="text-[#aeaeb2]">
            <Caret gap={-1} />
            Search anything...
          </span>
        )}
      </div>
    </div>
  );
}

/** Solid while typing (each keystroke restarts it), blinking when idle. `gap`: px from the text. */
function Caret({ gap }) {
  return (
    <span className="relative inline-block h-0 w-0 align-middle">
      <span
        className="absolute top-0 h-[26px] w-[2px] -translate-y-1/2 animate-[lumio-blink_1.06s_linear_infinite] bg-[#1d1d1f]"
        style={{ left: gap }}
      />
    </span>
  );
}

/** A result; the blue highlight behind it is the sliding pill in Results. */
function Row({ flip, index, row, selected }) {
  const props = {
    "data-flip": flip,
    "data-row": index,
    className: `relative mx-2 flex h-[46px] items-center gap-3 rounded-[9px] transition-colors duration-200 ${
      row.kind === "calc" ? "px-3" : "px-2.5"
    } ${selected ? "text-white" : "text-[#1d1d1f]"}`,
    style: { animation: `lumio-row-in 280ms ${EASE_OUT} both` },
  };
  if (row.kind === "calc") {
    return (
      <div {...props}>
        <span className="flex min-w-0 flex-1 items-baseline gap-1.5 text-[16px] leading-[22px]">
          <span className="truncate">{row.expression}</span>
          <span className="shrink-0">
            = <span className="font-semibold">{row.answer}</span>
          </span>
        </span>
        <ReturnGlyph shown={selected} />
      </div>
    );
  }
  return (
    <div {...props}>
      <span className="flex h-[30px] w-[30px] shrink-0 items-center justify-center">
        <RowIcon icon={row.icon} selected={selected} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[14px] font-medium leading-[18px]">{row.title}</span>
        <span
          className={`block truncate text-[12px] leading-[16px] transition-colors duration-200 ${
            selected ? "text-white/80" : "text-[#86868b]"
          }`}
        >
          {row.subtitle}
        </span>
      </span>
      <ReturnGlyph shown={selected} />
    </div>
  );
}

function ReturnGlyph({ shown }) {
  return (
    <span className="flex shrink-0 items-center transition-opacity duration-200" style={{ opacity: shown ? 0.85 : 0 }}>
      <Glyph size={13} strokeWidth={2.2}>
        <path d="M20 4v7a4 4 0 0 1-4 4H4" />
        <path d="m9 10-5 5 5 5" />
      </Glyph>
    </span>
  );
}

/* Icons: Lumio draws files and symbols with Lucide (lucide.dev, ISC licence); apps show their own icon. */

function Glyph({ size, strokeWidth = 2, className, style, children }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      style={style}
      aria-hidden
    >
      {children}
    </svg>
  );
}

const FILE_OUTLINE =
  "M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z";
const FILE_TINTS = { image: "#AF52DE", pdf: "#E5484D", document: "#2F80ED" };
const ICON_FADE = "color 200ms ease, fill 200ms ease";

/** Lumio's tile for an app without its own icon: a white symbol on a tinted gradient. */
function Tile({ from, to, children }) {
  return (
    <span
      className="flex h-[30px] w-[30px] items-center justify-center rounded-[8px] text-white shadow-[0_0.5px_1px_rgba(0,0,0,0.12),0_1px_3px_rgba(0,0,0,0.06)]"
      style={{ background: `linear-gradient(180deg, ${from} 0%, ${to} 100%)` }}
    >
      <Glyph size={16.8}>{children}</Glyph>
    </span>
  );
}

function RowIcon({ icon, selected }) {
  if (icon.startsWith("/")) return <Image src={icon} alt="" width={30} height={30} className="h-[30px] w-[30px] object-contain" />;
  if (icon === "chrome") return <ChromeIcon size={30} />;
  if (icon === "calculator") {
    return (
      <Tile from="#FFA82E" to="#FF9500">
        <rect width="16" height="20" x="4" y="2" rx="2" />
        <path d="M8 6h8M16 14v4M16 10h.01M12 10h.01M8 10h.01M12 14h.01M8 14h.01M12 18h.01M8 18h.01" />
      </Tile>
    );
  }
  if (icon === "terminal") {
    return (
      <Tile from="#5D5D5F" to="#3A3A3C">
        <path d="m7 11 2-2-2-2M11 13h4" />
        <rect width="18" height="18" x="3" y="3" rx="2" ry="2" />
      </Tile>
    );
  }
  if (icon === "folder") {
    return (
      <Glyph
        size={30}
        strokeWidth={1.4}
        style={{
          color: selected ? "#fff" : "#2E9BE6",
          fill: selected ? "rgba(255,255,255,0.25)" : "rgba(90,200,250,0.7)",
          transition: ICON_FADE,
        }}
      >
        <path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" />
      </Glyph>
    );
  }
  return (
    <Glyph size={27.6} strokeWidth={1.5} style={{ color: selected ? "#fff" : FILE_TINTS[icon], transition: ICON_FADE }}>
      <path d={FILE_OUTLINE} />
      <path d="M14 2v5a1 1 0 0 0 1 1h5" />
      {icon === "image" ? (
        <>
          <circle cx="10" cy="12" r="2" />
          <path d="m20 17-1.296-1.296a2.41 2.41 0 0 0-3.408 0L9 22" />
        </>
      ) : (
        <path d="M10 9H8M16 13H8M16 17H8" />
      )}
    </Glyph>
  );
}

/** Three segments, each bounded by a line tangent to the white ring, around a blue centre. */
function ChromeIcon({ size }) {
  return (
    <svg width={size} height={size} viewBox="0 0 48 48" aria-hidden>
      <circle cx="24" cy="24" r="22" fill="#EA4335" />
      <path d="M24 14H43.596A22 22 0 0 1 22.862 45.971L32.66 29A10 10 0 0 0 24 14Z" fill="#FBBC04" />
      <path d="M32.66 29L22.862 45.971A22 22 0 0 1 5.542 12.029L15.34 29A10 10 0 0 0 32.66 29Z" fill="#34A853" />
      <circle cx="24" cy="24" r="10" fill="#fff" />
      <circle cx="24" cy="24" r="8" fill="#1A73E8" />
    </svg>
  );
}
