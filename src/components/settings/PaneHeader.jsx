import { PANES } from "./Sidebar";

/** The pane's name for screen readers; the sidebar already shows it on screen. */
export function PaneHeader({ pane }) {
  const label = PANES.find((p) => p.id === pane)?.label;
  return <h1 className="sr-only">{label}</h1>;
}
