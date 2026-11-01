import { useEffect, useState } from "react";
import { Check, Eye, X } from "lucide-react";
import { Button, Group, Row, Segmented, Select, Slider, StatusDot, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { IndicatorPreview } from "../../capslock/IndicatorPreview";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { api, errorMessage } from "../../../services/tauri";
import { toast } from "../../../stores/toastStore";

const MODE = {
  cursor: { tone: "ok", title: "Follows the mouse pointer" },
  osd: { tone: "warn", title: "Shown as a system on-screen display" },
  unavailable: { tone: "error", title: "Not available in this session" },
};

function CapabilityLine({ label, cap }) {
  return (
    <div className="flex gap-2.5 px-3.5 py-2">
      {cap.available ? (
        <Check size={14} className="mt-[1px] shrink-0 text-success" />
      ) : (
        <X size={14} className="mt-[1px] shrink-0 text-danger" />
      )}
      <div className="text-[12px] leading-[16px]">
        <span className="text-label">{label}</span>
        <span className="text-secondary"> — {cap.method}</span>
        <div className="text-[11.5px] text-secondary">{cap.detail}</div>
      </div>
    </div>
  );
}

const DURATIONS = [
  { value: "0", label: "While Caps Lock is on" },
  { value: "1000", label: "1 second" },
  { value: "1500", label: "1.5 seconds" },
  { value: "2000", label: "2 seconds" },
  { value: "3000", label: "3 seconds" },
  { value: "5000", label: "5 seconds" },
  { value: "10000", label: "10 seconds" },
];

export default function CapsLockPane({ appearance }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const [support, setSupport] = useState(null);
  const c = settings.capsLock;
  const set = (patch) => update({ capsLock: patch });
  const [size, setSize] = useState(c.size);
  const [opacity, setOpacity] = useState(Math.round(c.opacity * 100));
  useEffect(() => setSize(c.size), [c.size]);
  useEffect(() => setOpacity(Math.round(c.opacity * 100)), [c.opacity]);

  useEffect(() => {
    const load = () => api.getCapsLockSupport().then(setSupport).catch((e) => toast.error(errorMessage(e)));
    load();
    const timer = setInterval(load, 1500); // reflects live Caps Lock state
    return () => clearInterval(timer);
  }, []);

  const previewDark = c.appearance === "dark" || (c.appearance === "system" && appearance.theme === "dark");
  const durationOptions = DURATIONS.some((d) => d.value === String(c.durationMs))
    ? DURATIONS
    : [...DURATIONS, { value: String(c.durationMs), label: `${c.durationMs / 1000} seconds` }];
  const cursorMode = support?.mode === "cursor";
  const mode = support ? MODE[support.mode] : null;

  return (
    <>
      <PaneHeader pane="capslock" subtitle="A small indicator next to the pointer when Caps Lock is turned on." />

      {support && (
        <Group title="Compatibility">
          <div className="flex items-start gap-3 px-3.5 py-3">
            <StatusDot tone={mode.tone} />
            <div className="-mt-1 min-w-0 flex-1">
              <div className="text-[13px] text-label">{mode.title}</div>
              <div className="text-[12px] leading-[16px] text-secondary">{support.summary}</div>
              {support.alternatives.map((alt) => (
                <div key={alt} className="mt-1 text-[12px] leading-[16px] text-secondary">
                  → {alt}
                </div>
              ))}
              {support.monitorError && <div className="mt-1 text-[12px] text-danger">{support.monitorError}</div>}
            </div>
            {support.capsLockOn !== null && support.capsLockOn !== undefined && (
              <span className="shrink-0 rounded-full bg-fill px-2 py-0.5 text-[11px] text-secondary">
                Caps Lock {support.capsLockOn ? "on" : "off"}
              </span>
            )}
          </div>
          <CapabilityLine label="Caps Lock detection" cap={support.detection} />
          <CapabilityLine label="Pointer position" cap={support.cursor} />
          <CapabilityLine label="Floating overlay" cap={support.overlay} />
        </Group>
      )}

      <Group>
        <ToggleRow label="Show Caps Lock indicator" checked={c.enabled} onChange={(v) => set({ enabled: v })} />
        <div className="px-3.5 py-3">
          <IndicatorPreview settings={{ ...c, size, opacity: opacity / 100 }} dark={previewDark} />
          <div className="mt-2.5 flex items-center justify-between">
            <span className="text-[11.5px] text-secondary">
              {cursorMode ? "Preview shows the real indicator at your pointer." : "Preview uses the system display."}
            </span>
            <Button icon={Eye} disabled={!support || support.mode === "unavailable"} onClick={() => api.previewCapsLock(true)}>
              Show Preview
            </Button>
          </div>
        </div>
      </Group>

      <Group title="Placement" footer={!cursorMode && support ? "Placement options apply when the pointer position is available (Windows, X11)." : undefined}>
        <Row label="Position" description="Relative to the mouse pointer." disabled={!cursorMode}>
          <Segmented
            value={c.position}
            disabled={!cursorMode}
            onChange={(v) => set({ position: v })}
            options={[
              { value: "near", label: "Near cursor" },
              { value: "below", label: "Below cursor" },
              { value: "above", label: "Above cursor" },
            ]}
          />
        </Row>
        <ToggleRow
          label="Follow the pointer"
          description="Keep the indicator next to the pointer while it moves."
          checked={c.followCursor}
          disabled={!cursorMode}
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
        <Row label="Appearance">
          <Segmented
            value={c.appearance}
            onChange={(v) => set({ appearance: v })}
            options={[
              { value: "system", label: "Automatic" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
          />
        </Row>
      </Group>

      <Group title="Timing">
        <Row label="Display duration" description="How long the indicator stays visible after Caps Lock is turned on.">
          <Select value={String(c.durationMs)} options={durationOptions} onChange={(v) => set({ durationMs: Number(v) })} />
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
          label="Briefly show when turned off"
          description="Flash the “off” state before the indicator disappears."
          checked={c.showWhenDisabled}
          onChange={(v) => set({ showWhenDisabled: v })}
        />
      </Group>

      {support?.osdAvailable && (
        <Group title="Wayland" footer="Used only when the pointer position is unavailable.">
          <Row label="Fallback display">
            <Segmented
              value={c.waylandFallback}
              onChange={(v) => set({ waylandFallback: v })}
              options={[
                { value: "osd", label: "System OSD" },
                { value: "none", label: "Off" },
              ]}
            />
          </Row>
        </Group>
      )}
    </>
  );
}
