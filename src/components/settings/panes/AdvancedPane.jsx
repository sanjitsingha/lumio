import { useCallback, useEffect, useRef, useState } from "react";
import { ClipboardCopy, FolderOpen, RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import { Button, Group, Row } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
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

function diagnosticsText(d) {
  if (!d) return "";
  const lines = [
    `Lumio ${d.appVersion}`,
    `OS: ${d.platform.osName.includes(d.platform.osVersion) ? d.platform.osName : `${d.platform.osName} ${d.platform.osVersion}`} (${d.platform.arch})`,
    `Session: ${d.platform.sessionType ?? "n/a"}  Desktop: ${d.platform.desktop ?? "n/a"}`,
    `WebView: ${d.webviewVersion ?? "unknown"}`,
    `Uptime: ${d.uptimeSeconds}s`,
    `Tray host: ${d.platform.traySupported ? "yes" : "no"}`,
    `Autostart entry: ${d.autostartRegistered ? "present" : "absent"}`,
    `Shortcuts: ${d.shortcuts.backendLabel}${d.shortcuts.available ? "" : ` (unavailable: ${d.shortcuts.unavailableReason})`}`,
    ...d.shortcuts.slots.map(
      (s) => `  ${s.id}: ${s.accelerator ?? "none"} ${s.accelerator ? (s.registered ? "[registered]" : `[failed: ${s.error?.message ?? "?"}]`) : ""}`,
    ),
    `Caps Lock: mode=${d.capsLock.mode} detection=${d.capsLock.detection.method} pointer=${d.capsLock.cursor.method} overlay=${d.capsLock.overlay.method} monitoring=${d.capsLock.monitoring}`,
    `Index: ${d.index.files} files, ${d.index.folders} folders, ${d.index.apps} apps, indexing=${d.index.indexing}, watching=${d.index.watching}, limitReached=${d.index.limitReached}`,
    `History entries: ${d.historyEntries}`,
    ...d.paths.map(([k, v]) => `${k}: ${v}`),
  ];
  return lines.join("\n");
}

export default function AdvancedPane() {
  const [busy, run] = useAction();
  const [diagnostics, setDiagnostics] = useState(null);
  const [logs, setLogs] = useState([]);
  const [confirmReset, setConfirmReset] = useState(false);
  const logRef = useRef(null);

  const loadDiagnostics = useCallback(() => api.getDiagnostics().then(setDiagnostics).catch((e) => toast.error(errorMessage(e))), []);
  const loadLogs = useCallback(
    () =>
      api
        .readLogTail(300)
        .then((lines) => {
          setLogs(lines);
          requestAnimationFrame(() => logRef.current && (logRef.current.scrollTop = logRef.current.scrollHeight));
        })
        .catch((e) => toast.error(errorMessage(e))),
    [],
  );

  useEffect(() => {
    loadDiagnostics();
    loadLogs();
  }, [loadDiagnostics, loadLogs]);

  return (
    <>
      <PaneHeader pane="advanced" />
      <Group title="Maintenance">
        <Row label="Clear cache" description="Deletes the cached file index and rebuilds it from disk.">
          <Button busy={busy === "cache"} icon={Trash2} onClick={() => run("cache", api.clearCache, "Cache cleared. Re-indexing in the background.")}>
            Clear Cache
          </Button>
        </Row>
        <Row label="Clear search history" description="Forget recent items, suggestions and usage-based ranking.">
          <Button busy={busy === "history"} onClick={() => run("history", api.clearSearchHistory, "Search history cleared.")}>
            Clear History
          </Button>
        </Row>
        <Row label="Reset all settings" description="Restore every preference, including shortcuts, to its default.">
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

      <Group title="Diagnostic information">
        <pre className="selectable max-h-[220px] overflow-auto px-3.5 py-3 font-mono text-[11px] leading-[16px] whitespace-pre-wrap text-label">
          {diagnostics ? diagnosticsText(diagnostics) : "Loading…"}
        </pre>
        <div className="flex justify-end gap-2 px-3.5 py-2">
          <Button icon={RefreshCw} onClick={loadDiagnostics}>
            Refresh
          </Button>
          <Button
            icon={ClipboardCopy}
            onClick={() =>
              navigator.clipboard
                .writeText(diagnosticsText(diagnostics))
                .then(() => toast.success("Diagnostics copied to the clipboard."))
                .catch(() => toast.error("Copying to the clipboard is not available."))
            }
          >
            Copy
          </Button>
        </div>
      </Group>

      <Group title="Application logs">
        <pre
          ref={logRef}
          className="selectable max-h-[260px] min-h-[120px] overflow-auto bg-fill px-3.5 py-3 font-mono text-[10.5px] leading-[15px] whitespace-pre-wrap text-secondary"
        >
          {logs.length ? logs.join("\n") : "No log entries yet."}
        </pre>
        <div className="flex justify-end gap-2 px-3.5 py-2">
          <Button icon={RefreshCw} onClick={loadLogs}>
            Refresh
          </Button>
          <Button icon={FolderOpen} onClick={() => run("logs", api.openLogFolder)}>
            Open Log Folder
          </Button>
        </div>
      </Group>
    </>
  );
}
