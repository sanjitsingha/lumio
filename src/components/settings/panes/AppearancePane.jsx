import { Check } from "lucide-react";
import { Group, Row, Segmented } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { ACCENT_ORDER, ACCENTS } from "../../../utils/theme";
import { cx } from "../../../utils/cx";

function ThemeCard({ mode, selected, onSelect }) {
  const half = (dark) => (
    <div className={cx("flex h-full flex-1 flex-col gap-1 p-1.5", dark ? "bg-[#1e1e21]" : "bg-[#f5f5f7]")}>
      <div className={cx("h-2 w-[45%] rounded-sm", dark ? "bg-[#3a3a3e]" : "bg-white")} />
      <div className={cx("flex-1 overflow-hidden rounded-[3px]", dark ? "bg-[#2c2c30]" : "bg-white")}>
        <div className="m-1 h-1.5 w-[60%] rounded-sm bg-accent" />
        <div className={cx("mx-1 h-1 w-[75%] rounded-sm", dark ? "bg-[#48484c]" : "bg-[#e3e3e8]")} />
      </div>
    </div>
  );
  const label = { system: "Automatic", light: "Light", dark: "Dark" }[mode];
  return (
    <button type="button" onClick={onSelect} className="flex flex-col items-center gap-1.5" aria-pressed={selected}>
      <div
        className={cx(
          "flex h-[64px] w-[96px] overflow-hidden rounded-[7px] border-[0.5px] border-separator-strong",
          selected ? "ring-[3px] ring-accent" : "ring-0",
        )}
      >
        {mode === "system" ? (
          <>
            {half(false)}
            {half(true)}
          </>
        ) : (
          half(mode === "dark")
        )}
      </div>
      <span className={cx("text-[12px]", selected ? "font-medium text-label" : "text-secondary")}>{label}</span>
    </button>
  );
}

export default function AppearancePane({ appearance }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const a = settings.appearance;
  const set = (patch) => update({ appearance: patch });
  const dark = appearance.theme === "dark";

  return (
    <>
      <PaneHeader pane="appearance" />
      <Group>
        <div className="flex items-center gap-4 px-3.5 py-3">
          <div className="flex-1 text-[13px] text-label">Appearance</div>
          <div className="flex gap-4">
            {["light", "dark", "system"].map((mode) => (
              <ThemeCard key={mode} mode={mode} selected={a.theme === mode} onSelect={() => set({ theme: mode })} />
            ))}
          </div>
        </div>
        <Row label="Accent color" description={a.accent === "system" ? "Following your system accent color." : undefined}>
          <div className="flex gap-2" role="radiogroup" aria-label="Accent color">
            {ACCENT_ORDER.map((name) => {
              const color = name === "system" ? null : ACCENTS[name][dark ? 1 : 0];
              const selected = a.accent === name;
              return (
                <button
                  key={name}
                  type="button"
                  role="radio"
                  aria-checked={selected}
                  aria-label={name}
                  title={name[0].toUpperCase() + name.slice(1)}
                  onClick={() => set({ accent: name })}
                  className={cx(
                    "flex h-[20px] w-[20px] items-center justify-center rounded-full shadow-[inset_0_0_0_0.5px_rgba(0,0,0,0.2)]",
                    selected && "ring-2 ring-offset-2 ring-offset-surface",
                  )}
                  style={{
                    background:
                      color ||
                      "conic-gradient(#FF3B30, #FF9500, #FFCC00, #34C759, #0A84FF, #AF52DE, #FF2D55, #FF3B30)",
                    "--tw-ring-color": color || "var(--lm-accent)",
                  }}
                >
                  {selected && <Check size={11} strokeWidth={3} className="text-white drop-shadow" />}
                </button>
              );
            })}
          </div>
        </Row>
      </Group>

      <Group title="Motion" footer="Reduced motion replaces movement with simple fades.">
        <Row label="Animations">
          <Segmented
            value={a.motion}
            onChange={(v) => set({ motion: v })}
            options={[
              { value: "system", label: "Automatic" },
              { value: "reduced", label: "Reduced" },
              { value: "full", label: "Full" },
            ]}
          />
        </Row>
      </Group>
    </>
  );
}
