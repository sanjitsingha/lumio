import { useCallback, useState } from "react";
import { Power, RotateCcw, Trash2 } from "lucide-react";
import { Button, Group, Row, Select, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { api, errorMessage } from "../../../services/tauri";
import { usePlatform } from "../../../hooks/usePlatform";
import { toast } from "../../../stores/toastStore";

/** Lummo is dark only, so the theme is shown but cannot be changed. */
const THEMES = [{ value: "dark", label: "Dark" }];
const ANIMATIONS = [
  { value: "system", label: "Automatic" },
  { value: "reduced", label: "Reduced" },
  { value: "full", label: "Full" },
];

const TEXT_SIZES = [
  { value: "small", label: "Small" },
  { value: "medium", label: "Medium" },
  { value: "large", label: "Large" },
];

function useAction() {
  const [busy, setBusy] = useState(null);
  const run = useCallback(async (key, fn, success) => {
    setBusy(key);
    try {
      await fn();
      if (success) toast.success(success);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }, []);
  return [busy, run];
}

export default function GeneralPane() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const [busy, run] = useAction();
  const [confirmReset, setConfirmReset] = useState(false);
  const g = settings.general;
  const a = settings.appearance;
  const set = (patch) => update({ general: patch });
  const setAppearance = (patch) => update({ appearance: patch });
  const platform = usePlatform();

  return (
    <>
      <PaneHeader pane="general" />
      <Group title="General">
        <ToggleRow
          label="Launch at startup"
          checked={g.launchAtStartup}
          onChange={(v) => set({ launchAtStartup: v })}
        />
        <ToggleRow
          label="Show in system tray"
          checked={g.showInTray}
          onChange={(v) => set({ showInTray: v })}
        />
        {platform?.os === "linux" && (
          <ToggleRow
            label="Window manager"
            description="Type “window” in Spotlight to snap, move or center the window you are using"
            checked={settings.spotlight.searchWindows}
            onChange={(v) => update({ spotlight: { searchWindows: v } })}
          />
        )}
      </Group>

      <Group title="Appearance">
        <Row label="Theme">
          <Select label="Theme" value="dark" options={THEMES} onChange={() => {}} locked />
        </Row>
        <Row label="Text size" description="Adjust the text of the Lummo interface">
          <Select label="Text size" value={a.textSize} options={TEXT_SIZES} onChange={(v) => setAppearance({ textSize: v })} />
        </Row>
        <Row label="Animation">
          <Select label="Animation" value={a.motion} options={ANIMATIONS} onChange={(v) => setAppearance({ motion: v })} />
        </Row>
      </Group>

      <Group title="Maintenance">
        <Row label="Clear cache">
          <Button
            busy={busy === "cache"}
            icon={Trash2}
            onClick={() => run("cache", api.clearCache, "Cache cleared. Lummo is rebuilding it in the background.")}
          >
            Clear Cache
          </Button>
        </Row>
        <Row label="Clear search history">
          <Button busy={busy === "history"} onClick={() => run("history", api.clearSearchHistory, "Search history cleared.")}>
            Clear History
          </Button>
        </Row>
        <Row label="Reset all settings">
          {confirmReset ? (
            <>
              <Button onClick={() => setConfirmReset(false)}>Cancel</Button>
              <Button
                variant="destructive"
                busy={busy === "reset"}
                onClick={() =>
                  run("reset", () => api.resetSettings(null), "All settings were reset.").then(() => setConfirmReset(false))
                }
              >
                Reset Everything
              </Button>
            </>
          ) : (
            <Button variant="destructive" icon={RotateCcw} onClick={() => setConfirmReset(true)}>
              Reset…
            </Button>
          )}
        </Row>
      </Group>

      <Group>
        <Row label="Quit Lummo">
          <Button variant="destructive" icon={Power} onClick={() => api.quitApp()}>
            Quit
          </Button>
        </Row>
      </Group>
    </>
  );
}
