"use client";

import { Inter } from "next/font/google";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

// self-hosted by Next.js at build time; no request to Google from the page
const inter = Inter({ subsets: ["latin"], display: "swap" });

/*
 * A headline that types itself, with Lummo's Caps Lock indicator under the
 * text cursor: it pops in when Caps Lock goes on, glides along as the capitals
 * are typed, and fades away when Caps Lock goes off. Artwork,
 * placement and timings are the app's (src-tauri/src/capslock in the Lummo repo).
 */

const HEADLINE = "get spotlight search and a caps lock indicator";
/** Typed under the headline in turn; `{ caps }` is typed with Caps Lock on. */
const LINES = [
  [{ caps: "BETTER" }, " than apple's 🍊"],
  ["no mac required. ", { caps: "NO, REALLY." }],
  ["so you never ", { caps: "TYPE LIKE THIS" }, " by accident"],
  ["for windows ", { caps: "AND" }, " linux. sorry, apple"],
];
const plain = (line) => line.map((part) => (typeof part === "string" ? part : part.caps)).join("");

/** The indicator's size, relative to the headline's font size. */
const INDICATOR_EM = 0.8;
/** Lummo's "Normal" animation speed. */
const ANIMATION_MS = 170;
/** How quickly the indicator catches up with the cursor (Lummo: 0.4 × the animation time). */
const GLIDE_S = 0.07;

/** Milliseconds before the next character: a little uneven, like a person typing. */
const RHYTHM = [0, 30, -15, 45, 10, -20, 60, 5, -10, 35];
const typingDelay = (char, i) => 85 + RHYTHM[i % RHYTHM.length] + (char === " " ? 50 : 0);

export default function LummoCapsHeadline({ as: Tag = "h1", className = "", indicator = "dark" }) {
  const root = useRef(null);
  const caret = useRef(null);
  const [active, setActive] = useState(false);
  const [text, setText] = useState("");
  // "hidden" | "on"
  const [caps, setCaps] = useState("hidden");
  // counts appearances, so each one pops in fresh at the cursor
  const [appearance, setAppearance] = useState(0);
  const [size, setSize] = useState(0);
  const [target, setTarget] = useState(null);
  const glide = useGlide(target, appearance);

  // where the indicator goes: Lummo's "near" placement, 6 px below the cursor and 4 px left of it (for a 40 px body)
  const measure = useCallback(() => {
    const c = caret.current;
    if (!c) return;
    const body = Math.round(parseFloat(getComputedStyle(root.current).fontSize) * INDICATOR_EM);
    const next = { x: Math.round(c.offsetLeft - body * 0.1), y: Math.round(c.offsetTop + c.offsetHeight + body * 0.15) };
    setSize(body);
    setTarget((t) => (t?.x === next.x && t.y === next.y ? t : next));
  }, []);
  useLayoutEffect(measure, [text, measure]);
  useEffect(() => {
    const observer = new ResizeObserver(measure);
    observer.observe(root.current);
    // Inter may arrive after the first layout and move the cursor
    document.fonts?.ready.then(measure);
    return () => observer.disconnect();
  }, [measure]);

  // plays only while on screen, and starts from the beginning when scrolled back to
  useEffect(() => {
    const observer = new IntersectionObserver(([entry]) => setActive(entry.isIntersecting), { threshold: 0.5 });
    observer.observe(root.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!active) return undefined;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setText(plain(LINES[0]));
      return () => setText("");
    }

    const timers = new Set();
    const later = (fn, ms) => {
      const id = setTimeout(() => timers.delete(id) && fn(), ms);
      timers.add(id);
    };
    // a cancelled wait never resolves, which stops the loop where it is
    const wait = (ms) => new Promise((resolve) => later(resolve, ms));

    (async () => {
      for (let n = 0; ; n = (n + 1) % LINES.length) {
        let typed = 0;
        for (const part of LINES[n]) {
          const withCaps = typeof part !== "string";
          if (withCaps) {
            await wait(300);
            setAppearance((a) => a + 1);
            setCaps("on");
            await wait(380);
          }
          for (const char of Array.from(withCaps ? part.caps : part)) {
            setText((t) => t + char);
            await wait(typingDelay(char, typed++));
          }
          if (withCaps) {
            await wait(160);
            // like Lummo (and a Mac): Caps Lock off just fades the indicator away
            setCaps("hidden");
          }
        }
        await wait(2600);
        for (let i = Array.from(plain(LINES[n])).length; i > 0; i--) {
          setText((t) => Array.from(t).slice(0, -1).join(""));
          await wait(28);
        }
        await wait(500);
      }
    })();
    return () => {
      timers.forEach(clearTimeout);
      setText("");
      setCaps("hidden");
    };
  }, [active]);

  const dark = indicator === "dark";
  return (
    <div
      ref={root}
      className={`${inter.className} relative pb-[1em] text-center text-[clamp(1.9rem,4.6vw,3.4rem)] font-normal leading-[1.25] tracking-[-0.025em] text-[#3a3a3c] ${className}`}
    >
      <Tag>
        <span className="sr-only">{`${HEADLINE}, ${plain(LINES[0]).toLowerCase()}`}</span>
        <span aria-hidden className="block">
          {HEADLINE}
        </span>
        <span aria-hidden className="block whitespace-pre-wrap">
          {text}
          <span
            ref={caret}
            key={text}
            className="ml-[0.04em] inline-block h-[1.05em] w-[0.055em] rounded-full bg-[#c7c7cc] align-[-0.2em]"
            style={{ animation: "lummo-caret 1.06s linear infinite" }}
          />
        </span>
      </Tag>

      <div ref={glide} aria-hidden className="pointer-events-none absolute left-0 top-0" style={{ width: size, height: size }}>
        {/* fades out at full size; each appearance pops in from 86% */}
        <div
          className="h-full w-full"
          style={{ opacity: caps === "hidden" ? 0 : 1, transition: caps === "hidden" ? `opacity ${ANIMATION_MS}ms ease` : "none" }}
        >
          {appearance > 0 && (
            <div
              key={appearance}
              className="flex h-full w-full items-center justify-center"
              style={{
                borderRadius: size * 0.27,
                background: dark ? "rgba(40,40,44,0.96)" : "rgba(252,252,253,0.97)",
                border: `1px solid ${dark ? "rgba(255,255,255,0.14)" : "rgba(0,0,0,0.09)"}`,
                boxShadow: dark ? "0 6px 18px rgba(0,0,0,0.3)" : "0 6px 18px rgba(0,0,0,0.16)",
                animation: `lummo-pop ${ANIMATION_MS}ms cubic-bezier(0.2, 0.8, 0.2, 1) both`,
              }}
            >
              <CapsLockGlyph dark={dark} size={size} />
            </div>
          )}
        </div>
      </div>

      <style>{`
        @keyframes lummo-caret { 0%, 50% { opacity: 1 } 50.01%, 100% { opacity: 0 } }
        @keyframes lummo-pop { from { opacity: 0; transform: scale(0.86) } to { opacity: 1; transform: scale(1) } }
      `}</style>
    </div>
  );
}

/** The ⇪ glyph and its lit "LED", as Lummo draws them. */
function CapsLockGlyph({ dark, size }) {
  const glyph = dark ? "#F5F5F7" : "#1D1D1F";
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" aria-hidden>
      <path
        d="M50 17 L79 47 L62.5 47 L62.5 62 L37.5 62 L37.5 47 L21 47 Z"
        fill={glyph}
        stroke={glyph}
        strokeWidth={3.5}
        strokeLinejoin="round"
      />
      <rect x="35" y="70.5" width="30" height="7.5" rx="3" fill={dark ? "#30D158" : "#28BE50"} />
    </svg>
  );
}

/**
 * Moves the returned element toward `target` on Lummo's glide: a critically
 * damped spring that starts gently, never overshoots, and keeps its momentum
 * when the next keystroke moves the cursor again. A new `appearance` starts
 * exactly at the target.
 */
function useGlide(target, appearance) {
  const el = useRef(null);
  const motion = useRef({ pos: null, vel: { x: 0, y: 0 }, frame: 0 });
  const latest = useRef(target);
  useLayoutEffect(() => {
    latest.current = target;
  });

  useLayoutEffect(() => {
    const m = motion.current;
    if (!latest.current) return;
    cancelAnimationFrame(m.frame);
    m.pos = { ...latest.current };
    m.vel = { x: 0, y: 0 };
    place(el.current, m.pos);
  }, [appearance]);

  useEffect(() => {
    const m = motion.current;
    if (!target) return undefined;
    if (!m.pos) {
      m.pos = { ...target };
      place(el.current, m.pos);
      return undefined;
    }
    let last = performance.now();
    const step = (now) => {
      const dt = Math.min(Math.max(now - last, 0) / 1000, 1 / 30);
      last = now;
      m.pos = { x: smoothDamp(m.pos.x, target.x, m.vel, "x", dt), y: smoothDamp(m.pos.y, target.y, m.vel, "y", dt) };
      place(el.current, m.pos);
      if (m.pos.x !== target.x || m.pos.y !== target.y) m.frame = requestAnimationFrame(step);
    };
    m.frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(m.frame);
  }, [target]);

  return el;
}

const place = (node, p) => {
  if (node) node.style.transform = `translate3d(${p.x}px, ${p.y}px, 0)`;
};

/** Lummo's `smooth_damp` (src-tauri/src/capslock/mod.rs). */
function smoothDamp(current, target, vel, axis, dt) {
  const omega = 2 / GLIDE_S;
  const x = omega * dt;
  const decay = 1 / (1 + x + 0.48 * x * x + 0.235 * x * x * x);
  const change = current - target;
  const temp = (vel[axis] + omega * change) * dt;
  vel[axis] = (vel[axis] - omega * temp) * decay;
  const next = target + (change + temp) * decay;
  const overshot = target > current === next > target;
  if (overshot || (Math.abs(next - target) < 0.5 && Math.abs(vel[axis]) < 10)) {
    vel[axis] = 0;
    return target;
  }
  return next;
}
