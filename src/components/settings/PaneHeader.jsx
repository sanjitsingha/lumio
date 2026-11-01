import { PaneIcon, PANES } from "./Sidebar";

export function PaneHeader({ pane, subtitle }) {
  const meta = PANES.find((p) => p.id === pane);
  return (
    <header className="mb-5 flex items-center gap-3">
      {meta && <PaneIcon Icon={meta.Icon} tint={meta.tint} size={30} />}
      <div>
        <h1 className="text-[20px] leading-[24px] font-semibold tracking-[-0.01em] text-label">{meta?.label}</h1>
        {subtitle && <p className="text-[12px] text-secondary">{subtitle}</p>}
      </div>
    </header>
  );
}
