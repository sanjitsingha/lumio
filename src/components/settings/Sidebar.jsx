import { cx } from "../../utils/cx";

export const PANES = [
  { id: "general", label: "General", icon: "/settings-icons/general.svg" },
  { id: "spotlight", label: "Spotlight", icon: "/settings-icons/spotlight.svg" },
  { id: "capslock", label: "Caps Lock Indicator", icon: "/settings-icons/capslock.svg" },
  { id: "appearance", label: "Appearance", icon: "/settings-icons/appearance.svg" },
  { id: "about", label: "About", icon: "/settings-icons/about.svg" },
];

export function PaneIcon({ icon, size = 22 }) {
  return <img src={icon} alt="" width={size} height={size} draggable={false} className="shrink-0" />;
}

export function Sidebar({ active, onSelect, appName, version }) {
  return (
    <nav aria-label="Settings sections" className="flex w-[228px] shrink-0 flex-col border-r-[0.5px] border-separator-strong bg-sidebar">
      <ul className="flex flex-col gap-2.5 px-2.5 pt-4">
        {PANES.map((pane) => {
          const selected = pane.id === active;
          return (
            <li key={pane.id}>
              <button
                type="button"
                onClick={() => onSelect(pane.id)}
                aria-current={selected ? "page" : undefined}
                className={cx(
                  "flex h-[36px] w-full items-center gap-2.5 rounded-[8px] pr-2 pl-1 text-left text-[13px]",
                  selected ? "bg-fill-strong text-label" : "text-label hover:bg-fill",
                )}
              >
                <PaneIcon icon={pane.icon} size={28} />
                <span className="truncate">{pane.label}</span>
              </button>
            </li>
          );
        })}
      </ul>
      <div className="mt-auto flex items-center gap-2.5 px-4 pt-4 pb-5">
        <img src="/app-icon.png" alt="" width={34} height={34} draggable={false} />
        <div className="min-w-0">
          <div className="truncate text-[14px] font-semibold text-label">{appName || "Lummo"}</div>
          <div className="text-[11px] text-secondary">{version ? `Version ${version}` : " "}</div>
        </div>
      </div>
    </nav>
  );
}
