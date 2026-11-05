import { useEffect, useState } from "react";
import { TriangleAlert } from "lucide-react";
import { Group, Row, Segmented, Select, Slider, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { IndicatorPreview } from "../../capslock/IndicatorPreview";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { api, errorMessage } from "../../../services/tauri";
import { toast } from "../../../stores/toastStore";

/** `typing` stands for the `untilTyping` setting; the others are `durationMs`. */
const UNTIL_TYPING = "typing";
const DURATIONS = [
  { value: UNTIL_TYPING, label: "Until you type" },
  { value: "0", label: "While Caps Lock is on" },
  { value: "200", label: "0.2 seconds" },
  { value: "300", label: "0.3 seconds" },
  { value: "500", label: "0.5 seconds" },
  { value: "800", label: "0.8 seconds" },
  { value: "1000", label: "1 second" },
  { value: "1500", label: "1.5 seconds" },
  { value: "2000", label: "2 seconds" },
  { value: "3000", label: "3 seconds" },
  { value: "5000", label: "5 seconds" },
  { value: "10000", label: "10 seconds" },
];

/** One short sentence when the indicator cannot work normally here. */
function problem(support) {
  if (!support) return null;
  if (support.monitorError) return `Something went wrong: ${support.monitorError}`;
  return support.mode === "cursor" ? null : support.summary || null;
}

export default function CapsLockPane({ appearance }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const [support, setSupport] = useState(null);
  const c = settings.capsLock;
  const set = (patch) => update({ capsLock: patch });
  const [size, setSize] = useState(c.size);
  const [opacity, setOpacity] = useState(Math.round(c.opacity * 100));
  const [radius, setRadius] = useState(c.cornerRadius);
  useEffect(() => setSize(c.size), [c.size]);
  useEffect(() => setOpacity(Math.round(c.opacity * 100)), [c.opacity]);
  useEffect(() => setRadius(c.cornerRadius), [c.cornerRadius]);

  useEffect(() => {
    api.getCapsLockSupport().then(setSupport).catch((e) => toast.error(errorMessage(e)));
  }, []);

  const durationOptions = DURATIONS.some((d) => d.value === String(c.durationMs))
    ? DURATIONS
    : [...DURATIONS, { value: String(c.durationMs), label: `${c.durationMs / 1000} seconds` }];
  const duration = c.untilTyping ? UNTIL_TYPING : String(c.durationMs);
  const setDuration = (v) => set(v === UNTIL_TYPING ? { untilTyping: true } : { untilTyping: false, durationMs: Number(v) });
  const message = problem(support);
  const atCursor = !support || support.mode === "cursor";

  return (
    <>
      <PaneHeader pane="capslock" subtitle="A small indicator at the text cursor when Caps Lock is on." />

      {message && (
        <div className="mb-5 flex gap-2 rounded-[10px] bg-fill px-3.5 py-2.5 text-[12px] leading-[16px] text-secondary">
          <TriangleAlert size={14} className="mt-[1px] shrink-0 text-warning" />
          <span>{message}</span>
        </div>
      )}

      <Group>
        <ToggleRow label="Show Caps Lock indicator" checked={c.enabled} onChange={(v) => set({ enabled: v })} />
        <div className="px-3.5 pb-3.5 pt-1">
          <IndicatorPreview
            settings={{ ...c, size, opacity: opacity / 100, cornerRadius: radius }}
            dark={appearance.theme === "dark"}
          />
        </div>
      </Group>

      <Group title="Placement">
        <Row label="Position" description="Relative to the text cursor." disabled={!atCursor}>
          <Segmented
            value={c.position}
            disabled={!atCursor}
            onChange={(v) => set({ position: v })}
            options={[
              { value: "near", label: "Near" },
              { value: "below", label: "Below" },
              { value: "above", label: "Above" },
            ]}
          />
        </Row>
        <ToggleRow
          label="Follow the text cursor"
          description="Keep the indicator next to the text cursor while you type."
          checked={c.followCursor}
          disabled={!atCursor}
          onChange={(v) => set({ followCursor: v })}
        />
      </Group>

      <Group title="Style">
        <Row label="Size">
          <Slider value={size} min={24} max={96} onChange={setSize} onCommit={(v) => set({ size: v })} format={(v) => `${v} px`} />
        </Row>
        <Row label="Opacity">
          <Slider value={opacity} min={30} max={100} onChange={setOpacity} onCommit={(v) => set({ opacity: v / 100 })} format={(v) => `${v}%`} />
        </Row>
        <Row label="Corner radius">
          <Slider value={radius} min={0} max={50} onChange={setRadius} onCommit={(v) => set({ cornerRadius: v })} format={(v) => `${v}%`} />
        </Row>
      </Group>

      <Group title="Timing">
        <Row label="Display duration" description="How long the indicator stays visible after Caps Lock is turned on.">
          <Select value={duration} options={durationOptions} onChange={setDuration} />
        </Row>
        <Row label="Animation">
          <Segmented
            value={c.animation}
            onChange={(v) => set({ animation: v })}
            options={[
              { value: "off", label: "Off" },
              { value: "fast", label: "Fast" },
              { value: "normal", label: "Normal" },
              { value: "slow", label: "Slow" },
            ]}
          />
        </Row>
        <ToggleRow
          label="Show when you click into a text field"
          description="With Caps Lock already on, the indicator appears as soon as a field gets focus."
          checked={c.showOnFocus}
          onChange={(v) => set({ showOnFocus: v })}
        />
      </Group>
    </>
  );
}
