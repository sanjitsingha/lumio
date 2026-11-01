import { useCallback, useEffect, useState } from "react";
import { api, errorMessage } from "../services/tauri";
import { EVENTS, subscribe } from "../services/events";
import { toast } from "../stores/toastStore";

/** Live update status from the backend plus actions (check, download, install). */
export function useUpdates() {
  const [status, setStatus] = useState(null);

  useEffect(() => {
    let alive = true;
    api.getUpdateStatus().then((s) => alive && setStatus(s)).catch(() => {});
    const off = subscribe(EVENTS.updateStatus, setStatus);
    return () => {
      alive = false;
      off();
    };
  }, []);

  const run = useCallback(async (action, onDone) => {
    try {
      const result = await action();
      if (result && typeof result === "object" && "phase" in result) setStatus(result);
      onDone?.(result);
    } catch (e) {
      toast.error(errorMessage(e));
      api.getUpdateStatus().then(setStatus).catch(() => {});
    }
  }, []);

  const check = useCallback(
    (quiet = false) =>
      run(api.checkForUpdates, (s) => {
        if (quiet || !s) return;
        if (s.phase.state === "upToDate") toast.success("Lumio is up to date.");
        if (s.phase.state === "available") toast.info(`Lumio ${s.latest.version} is available.`);
      }),
    [run],
  );
  const download = useCallback(() => run(api.downloadUpdate), [run]);
  const install = useCallback(() => run(api.installUpdate), [run]);
  const openReleasePage = useCallback(() => run(api.openReleasePage), [run]);

  return { status, check, download, install, openReleasePage };
}
