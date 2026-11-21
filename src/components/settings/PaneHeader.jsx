import { PaneIcon, PANES } from "./Sidebar";

export function PaneHeader({ pane }) {
  const meta = PANES.find((p) => p.id === pane);
  return (
    <header className="mb-5 flex items-center gap-3">
      {meta && <PaneIcon icon={meta.icon} size={36} />}
      <h1 className="text-[20px] leading-[24px] font-semibold tracking-[-0.01em] text-label">{meta?.label}</h1>
    </header>
  );
}
