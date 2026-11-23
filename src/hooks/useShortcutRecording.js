import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorMessage } from "../services/tauri";
import { toast } from "../stores/toastStore";

/** The shortcut status and the one recording a page can have at a time. */
export function useShortcutRecording() {
  const [report, setReport] = useState(null);
  const [recordingSlot, setRecordingSlot] = useState(null);

  const refresh = useCallback(() => api.getShortcutStatus().then(setReport).catch((e) => toast.error(errorMessage(e))), []);
  useEffect(() => {
    refresh();
  }, [refresh]);

  const stopRecording = useCallback(async () => {
    setRecordingSlot(null);
    try {
      setReport(await api.endShortcutRecording());
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }, []);

  // never leave shortcuts suspended if the window loses focus or the pane closes mid-recording
  const recordingRef = useRef(null);
  recordingRef.current = recordingSlot;
  useEffect(() => {
    if (!recordingSlot) return undefined;
    const onBlur = () => stopRecording();
    window.addEventListener("blur", onBlur);
    return () => window.removeEventListener("blur", onBlur);
  }, [recordingSlot, stopRecording]);
  useEffect(
    () => () => {
      if (recordingRef.current) api.endShortcutRecording().catch(() => {});
    },
    [],
  );

  const startRecording = useCallback(async (slotId) => {
    try {
      setReport(await api.beginShortcutRecording());
      setRecordingSlot(slotId);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }, []);

  const onSaved = useCallback((next, message) => {
    setRecordingSlot(null);
    setReport(next);
    if (message) toast.success(message);
  }, []);

  return { report, recordingSlot, refresh, startRecording, stopRecording, onSaved };
}
