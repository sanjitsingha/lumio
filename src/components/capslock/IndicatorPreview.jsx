import { motion } from "framer-motion";

/** The ⇪ glyph, same geometry as the native renderer (`capslock/render.rs`). */
export function CapsLockGlyph({ on, dark, size }) {
  const glyph = dark ? "#F5F5F7" : "#1D1D1F";
  const led = on ? (dark ? "#30D158" : "#28BE50") : dark ? "rgba(152,152,157,0.55)" : "rgba(134,134,139,0.55)";
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" aria-hidden>
      <path
        d="M50 17 L79 47 L62.5 47 L62.5 62 L37.5 62 L37.5 47 L21 47 Z"
        fill={on ? glyph : "none"}
        stroke={glyph}
        strokeOpacity={on ? 1 : 0.62}
        strokeWidth={on ? 3.5 : 5.5}
        strokeLinejoin="round"
      />
      <rect x="35" y="70.5" width="30" height="7.5" rx="3" fill={led} />
    </svg>
  );
}

/** Live, scaled preview of the indicator next to a pointer. */
export function IndicatorPreview({ settings, dark }) {
  const size = settings.size;
  const offset = {
    near: { left: 16 + 16, top: 20 + 14 },
    below: { left: 16 - size / 2, top: 14 + 28 },
    above: { left: 16 - size / 2, top: 14 - 12 - size },
  }[settings.position];
  return (
    <div
      className="relative h-[150px] w-full overflow-hidden rounded-[8px]"
      style={{
        background: dark
          ? "linear-gradient(135deg, #2b2d3a 0%, #1b1c24 100%)"
          : "linear-gradient(135deg, #dde6f5 0%, #f2eef7 100%)",
      }}
    >
      <div className="absolute" style={{ left: "50%", top: settings.position === "above" ? "62%" : "22%" }}>
        {/* pointer */}
        <svg width="16" height="22" viewBox="0 0 16 22" className="absolute" style={{ left: 0, top: 14 }} aria-hidden>
          <path d="M1 1 L1 17 L5 13 L8 20 L11 19 L8 12 L14 12 Z" fill="#fff" stroke="#000" strokeWidth="1.2" strokeLinejoin="round" />
        </svg>
        <motion.div
          key={`${settings.position}-${size}`}
          initial={{ opacity: 0, scale: 0.86 }}
          animate={{ opacity: settings.opacity, scale: 1 }}
          transition={{ duration: 0.2 }}
          className="absolute flex items-center justify-center"
          style={{
            left: offset.left,
            top: offset.top,
            width: size,
            height: size,
            borderRadius: size * 0.27,
            background: dark ? "rgba(40,40,44,0.96)" : "rgba(252,252,253,0.97)",
            border: `1px solid ${dark ? "rgba(255,255,255,0.14)" : "rgba(0,0,0,0.09)"}`,
            boxShadow: dark ? "0 6px 18px rgba(0,0,0,0.45)" : "0 6px 18px rgba(0,0,0,0.16)",
          }}
        >
          <CapsLockGlyph on dark={dark} size={size} />
        </motion.div>
      </div>
    </div>
  );
}
