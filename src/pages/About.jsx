import { Group, Row, ToggleRow } from "../components/common/controls";
import { UpdateCard } from "../components/settings/UpdateCard";
import { usePlatform } from "../hooks/usePlatform";
import { useSettings, useUpdateSettings } from "../hooks/useSettings";

export default function About({ updates }) {
  const info = usePlatform();
  const settings = useSettings();
  const update = useUpdateSettings();
  const g = settings.general;
  const set = (patch) => update({ general: patch });

  return (
    <>
      <div className="mt-4 mb-7 flex flex-col items-center text-center">
        <img src="/app-icon.png" alt="" width={96} height={96} draggable={false} />
        <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.01em] text-label">{info?.appName ?? "Lummo"}</h1>
        <p className="text-[12.5px] text-secondary">Version {info?.version ?? "…"}</p>
      </div>

      <UpdateCard updates={updates}>
        <ToggleRow
          label="Install updates automatically"
          checked={g.autoInstallUpdates}
          onChange={(v) => set({ autoInstallUpdates: v })}
        />
      </UpdateCard>

      <Group title="Privacy">
        <ToggleRow
          label="Share usage statistics"
          description="Anonymous counts, never what you type."
          checked={g.shareUsageStatistics}
          onChange={(v) => set({ shareUsageStatistics: v })}
        />
      </Group>

      <Group>
        <Row label="Developer">
          <span className="selectable text-[12.5px] text-secondary">{info?.authors || "—"}</span>
        </Row>
        <Row label="License">
          <span className="text-[12.5px] text-secondary">{info?.license || "—"}</span>
        </Row>
      </Group>
      <p className="mt-6 text-center text-[11px] text-tertiary">Inspired by macOS. Not affiliated with Apple Inc.</p>
    </>
  );
}
