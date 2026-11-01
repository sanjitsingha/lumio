import { Power, TriangleAlert } from "lucide-react";
import { Button, Group, Row, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { usePlatform } from "../../../hooks/usePlatform";
import { api } from "../../../services/tauri";

export default function GeneralPane({ onCheckUpdates }) {
  const settings = useSettings();
  const update = useUpdateSettings();
  const platform = usePlatform();
  const g = settings.general;
  const set = (patch) => update({ general: patch });

  return (
    <>
      <PaneHeader pane="general" />
      <Group title="Startup">
        <ToggleRow
          label="Launch at startup"
          description={
            platform?.os === "windows"
              ? "Start Lumio when you sign in to Windows."
              : "Start Lumio when you log in (XDG autostart)."
          }
          checked={g.launchAtStartup}
          onChange={(v) => set({ launchAtStartup: v })}
        />
        <ToggleRow
          label="Start minimized"
          description="When started at login, stay in the background instead of opening this window."
          checked={g.startMinimized}
          onChange={(v) => set({ startMinimized: v })}
        />
      </Group>

      <Group title="Background">
        <ToggleRow
          label="Run in background"
          description="Keep shortcuts and the Caps Lock indicator working when no Lumio window is open."
          checked={g.runInBackground}
          onChange={(v) => set({ runInBackground: v })}
        />
        <ToggleRow
          label="Close window to background"
          description="The close button hides Settings and keeps Lumio running. When off, closing Settings quits Lumio."
          checked={g.closeToBackground && g.runInBackground}
          disabled={!g.runInBackground}
          onChange={(v) => set({ closeToBackground: v })}
        />
        <ToggleRow
          label="Show icon in system tray"
          description="Quick access to Spotlight, Settings and the Caps Lock indicator."
          checked={g.showTrayIcon}
          onChange={(v) => set({ showTrayIcon: v })}
        />
        {g.showTrayIcon && platform && !platform.traySupported && platform.trayDetail && (
          <div className="flex gap-2 px-3.5 py-2.5 text-[11.5px] leading-[15px] text-secondary">
            <TriangleAlert size={14} className="mt-[1px] shrink-0 text-warning" />
            <span>{platform.trayDetail}</span>
          </div>
        )}
      </Group>

      <Group title="Updates">
        <ToggleRow
          label="Automatically check for updates"
          description="Looks for a new release shortly after Lumio starts and every few hours, and notifies you."
          checked={g.checkForUpdates}
          onChange={(v) => set({ checkForUpdates: v })}
        />
        <ToggleRow
          label="Install updates automatically"
          description="Download, verify and install new versions without asking, then restart Lumio. Linux system packages still ask for your password."
          checked={g.autoInstallUpdates && g.checkForUpdates}
          disabled={!g.checkForUpdates}
          onChange={(v) => set({ autoInstallUpdates: v })}
        />
        <Row label="Check now" description="Look for a newer version of Lumio.">
          <Button onClick={onCheckUpdates}>Check for Updates…</Button>
        </Row>
      </Group>

      <Group>
        <Row label="Quit Lumio" description="Stops Lumio completely and releases its global shortcuts.">
          <Button variant="destructive" icon={Power} onClick={() => api.quitApp()}>
            Quit
          </Button>
        </Row>
      </Group>
    </>
  );
}
