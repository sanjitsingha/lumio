import { useCallback, useState } from "react";
import { Power, RotateCcw, Trash2 } from "lucide-react";
import { Button, Group, Row, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { api, errorMessage } from "../../../services/tauri";
import { toast } from "../../../stores/toastStore";

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

export default function GeneralPane({ onCheckUpdates }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const [busy, run] = useAction();
  const [confirmReset, setConfirmReset] = useState(false);
  const g = settings.general;
  const set = (patch) => update({ general: patch });

  return (
    <>
      <PaneHeader pane="general" />
      <Group title="Startup">
        <ToggleRow
          label="Launch at startup"
          checked={g.launchAtStartup}
          onChange={(v) => set({ launchAtStartup: v })}
        />
      </Group>

      <Group title="Updates">
        <ToggleRow
          label="Install updates automatically"
          checked={g.autoInstallUpdates}
          onChange={(v) => set({ autoInstallUpdates: v })}
        />
        <Row label="Check now">
          <Button onClick={onCheckUpdates}>Check for Updates…</Button>
        </Row>
      </Group>

      <Group title="Privacy">
        <ToggleRow
          label="Share usage statistics"
          description="Anonymous counts, never what you type."
          checked={g.shareUsageStatistics}
          onChange={(v) => set({ shareUsageStatistics: v })}
        />
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
