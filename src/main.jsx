import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { error as logError } from "@tauri-apps/plugin-log";
import App from "./App";
import "./styles/index.css";

// Frontend failures end up in the same log file as the backend's.
window.addEventListener("error", (e) => logError(`[ui] ${e.message} at ${e.filename}:${e.lineno}`).catch(() => {}));
window.addEventListener("unhandledrejection", (e) =>
  logError(`[ui] unhandled rejection: ${e.reason?.message ?? JSON.stringify(e.reason)}`).catch(() => {}),
);

// A desktop utility should not show the webview's "Reload / Inspect" menu.
if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => {
    if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault();
  });
}

createRoot(document.getElementById("root")).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
