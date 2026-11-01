import { ArrowBigUpDash, Info, Keyboard, Paintbrush, Search, Settings, Wrench } from "lucide-react";
import { cx } from "../../utils/cx";

export const PANES = [
  { id: "general", label: "General", Icon: Settings, tint: "#8E8E93" },
  { id: "spotlight", label: "Spotlight", Icon: Search, tint: "#007AFF" },
  { id: "shortcuts", label: "Keyboard Shortcuts", Icon: Keyboard, tint: "#6E6E73" },
  { id: "capslock", label: "Caps Lock Indicator", Icon: ArrowBigUpDash, tint: "#34C759" },
  { id: "appearance", label: "Appearance", Icon: Paintbrush, tint: "#AF52DE" },
  { id: "advanced", label: "Advanced", Icon: Wrench, tint: "#636366" },
  { id: "about", label: "About", Icon: Info, tint: "#0A84FF" },
];

export function PaneIcon({ Icon, tint, size = 22 }) {
  return (
    <span
      className="flex shrink-0 items-center justify-center rounded-[6px] text-white"
      style={{
        width: size,
        height: size,
        background: `linear-gradient(180deg, color-mix(in srgb, ${tint} 80%, white) 0%, ${tint} 100%)`,
        boxShadow: "0 0.5px 1px rgba(0,0,0,0.18)",
      }}
    >
      <Icon size={size * 0.62} strokeWidth={2.2} />
    </span>
  );
}

export function Sidebar({ active, onSelect, appName, version }) {
  return (
    <nav aria-label="Settings sections" className="flex w-[228px] shrink-0 flex-col border-r-[0.5px] border-separator-strong bg-sidebar">
      <div className="flex items-center gap-2.5 px-4 pt-5 pb-4">
        <img src="/app-icon.svg" alt="" width={34} height={34} draggable={false} />
        <div className="min-w-0">
          <div className="truncate text-[14px] font-semibold text-label">{appName || "Lumio"}</div>
          <div className="text-[11px] text-secondary">{version ? `Version ${version}` : " "}</div>
        </div>
      </div>
      <ul className="flex flex-col gap-[2px] px-2.5">
        {PANES.map((pane) => {
          const selected = pane.id === active;
          return (
            <li key={pane.id}>
              <button
                type="button"
                onClick={() => onSelect(pane.id)}
                aria-current={selected ? "page" : undefined}
                className={cx(
                  "flex h-[30px] w-full items-center gap-2.5 rounded-[7px] px-2 text-left text-[13px]",
                  selected ? "bg-accent text-on-accent" : "text-label hover:bg-fill",
                )}
              >
                <PaneIcon Icon={pane.Icon} tint={pane.tint} size={20} />
                <span className="truncate">{pane.label}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
