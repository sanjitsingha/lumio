import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

/** Makes every px font size follow Settings → Text size (`--lm-text-scale`, set in useAppearance). */
const scaleFontSizes = {
  postcssPlugin: "lummo-text-scale",
  Declaration: {
    "font-size"(decl) {
      if (/^\d*\.?\d+px$/.test(decl.value)) decl.value = `calc(${decl.value} * var(--lm-text-scale, 1))`;
    },
  },
};

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  css: { postcss: { plugins: [scaleFontSizes] } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "es2022",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: "jsdom",
    include: ["tests/**/*.test.{js,jsx}"],
  },
});
