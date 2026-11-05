import { motion } from "framer-motion";

/** The ⇪ glyph, same geometry as the native renderer (`capslock/render.rs`). */
export function CapsLockGlyph({ dark, size }) {
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

/** Indicator body, as the native overlay draws it. */
function Indicator({ settings, dark, style }) {
  const size = settings.size;
  return (
    <motion.div
      key={`${settings.position}-${size}`}
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: settings.opacity, scale: 1 }}
      // the native spring at Normal speed (capslock/mod.rs `spring_pop`)
      transition={{ opacity: { duration: 0.17 }, scale: { type: "spring", stiffness: 900, damping: 27 } }}
      className="absolute flex items-center justify-center"
      style={{
        ...style,
        width: size,
        height: size,
        borderRadius: (size * settings.cornerRadius) / 100,
        background: dark ? "rgba(40,40,44,0.96)" : "rgba(252,252,253,0.97)",
        border: `1px solid ${dark ? "rgba(255,255,255,0.14)" : "rgba(0,0,0,0.09)"}`,
        boxShadow: dark ? "0 6px 18px rgba(0,0,0,0.45)" : "0 6px 18px rgba(0,0,0,0.16)",
      }}
    >
      <CapsLockGlyph dark={dark} size={size} />
    </motion.div>
  );
}

/**
 * Live preview: a sheet of paper being typed on, with the blinking text
 * cursor and the indicator where Lummo puts it (offsets match
 * `place_at_caret`).
 */
export function IndicatorPreview({ settings, dark }) {
  const size = settings.size;
  const caret = { height: 18 };
  const offset = {
    near: { left: -4, top: caret.height + 6 },
    below: { left: -size / 2, top: caret.height + 6 },
    above: { left: -size / 2, top: -6 - size },
  }[settings.position];
  return (
    <div className="relative h-[160px] w-full overflow-hidden rounded-[10px] bg-fill p-3">
      <div
        className="relative h-full w-full rounded-[8px] border border-black/[0.06] bg-white"
        style={{ boxShadow: "0 1px 3px rgba(0,0,0,0.08)" }}
      >
        <div className="absolute flex items-center" style={{ left: 22, top: settings.position === "above" ? 92 : 22 }}>
          <span className="text-[15px] leading-[18px] text-[#1D1D1F]">Hello, I am typing a messa</span>
          <span className="relative inline-block" style={{ width: 0, height: caret.height }}>
            <motion.span
              className="absolute left-0 top-0 block w-[2px] rounded-[1px] bg-[#007AFF]"
              style={{ height: caret.height }}
              animate={{ opacity: [1, 1, 0, 0] }}
              transition={{ duration: 1.06, repeat: Infinity, times: [0, 0.5, 0.5, 1] }}
            />
            <Indicator settings={settings} dark={dark} style={offset} />
          </span>
        </div>
      </div>
    </div>
  );
}
