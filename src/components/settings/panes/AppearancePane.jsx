import { Check } from "lucide-react";
import { Group, Row, Segmented } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { ACCENT_ORDER, ACCENTS } from "../../../utils/theme";
import { cx } from "../../../utils/cx";

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
        <Row label="Accent color">
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

      <Group title="Motion">
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
