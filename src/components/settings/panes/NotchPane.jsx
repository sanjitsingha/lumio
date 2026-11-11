import { Group, NumberField, Row, Segmented } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";

/** The longest each unit allows (`NotchSettings::keep_for_max` in `storage/settings.rs`). */
const MAX = { hours: 720, days: 365 };
const UNITS = [
  { value: "hours", label: "Hours" },
  { value: "days", label: "Days" },
];

export default function NotchPane() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const n = settings.notch;
  const set = (patch) => update({ notch: patch });

  // switching the unit keeps about the same length of time: 2 days become 48 hours
  const setUnit = (unit) => {
    if (unit === n.keepUnit) return;
    const keepFor = unit === "hours" ? n.keepFor * 24 : Math.max(1, Math.round(n.keepFor / 24));
    set({ keepUnit: unit, keepFor: Math.min(MAX[unit], keepFor) });
  };

  return (
    <>
      <PaneHeader pane="notch" subtitle="Rest the mouse at the top centre of the screen to see what you copied." />

      <Group
        title="Clipboard history"
        footer="Older copies leave the notch. The history is kept only on this PC, so it is still there after a restart; copies that password managers mark as private are never kept."
      >
        <Row label="Keep copies for" description="Counted from when each one was last copied.">
          <NumberField
            label="Keep copies for"
            value={n.keepFor}
            min={1}
            max={MAX[n.keepUnit]}
            onCommit={(keepFor) => set({ keepFor })}
          />
          <Segmented value={n.keepUnit} options={UNITS} onChange={setUnit} size="sm" />
        </Row>
      </Group>
    </>
  );
}
