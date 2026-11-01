import { useCallback, useEffect, useRef, useState } from "react";
import { CircleAlert, CircleCheck, Info, RotateCcw } from "lucide-react";
import { Button, Group, KeyCaps, StatusDot } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { ShortcutRecorder, CapturedShortcut } from "../../shortcuts/ShortcutRecorder";
import { usePlatform } from "../../../hooks/usePlatform";
import { api, errorMessage } from "../../../services/tauri";
import { toast } from "../../../stores/toastStore";
import { shortcutKeys } from "../../../utils/keys";

function SlotRow({ slot, os, recording, onRecord, onCancel, onSaved, report }) {
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(null);

  useEffect(() => {
    if (!recording) setAttempt(null);
  }, [recording]);

  const capture = useCallback(
    async (text) => {
      setBusy(true);
      setAttempt({ text, error: null });
      try {
        const next = await api.setShortcut(slot.id, text);
        onSaved(next, `${slot.label} is now ${text.replaceAll("+", " + ")}.`);
      } catch (e) {
        setAttempt({ text, error: errorMessage(e) });
      } finally {
        setBusy(false);
      }
    },
    [slot, onSaved],
  );

  const run = async (fn, message) => {
    try {
      onSaved(await fn(), message);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  const failing = !recording && slot.accelerator && !slot.registered && slot.error;
  return (
    <div className="px-3.5 py-2.5">
      <div className="flex min-h-[28px] items-center gap-4">
        <div className="min-w-0 flex-1">
          <div className="text-[13px] text-label">{slot.label}</div>
          <div className="text-[11.5px] text-secondary">{slot.description}</div>
        </div>
        {recording ? (
          <div className="flex items-center gap-2">
            <ShortcutRecorder os={os} busy={busy} onCapture={capture} onCancel={onCancel} />
            <Button onClick={onCancel}>Cancel</Button>
          </div>
        ) : (
          <div className="flex items-center gap-2.5">
            {slot.accelerator &&
              (slot.registered ? (
                <CircleCheck size={14} className="text-success" aria-label="Active" />
              ) : (
                <CircleAlert size={14} className="text-danger" aria-label="Not active" />
              ))}
            <KeyCaps keys={shortcutKeys(slot.accelerator, os)} />
            <Button onClick={onRecord} disabled={!report.available}>
              Change Shortcut
            </Button>
          </div>
        )}
      </div>
      {recording && attempt && (
        <div className="mt-2 flex items-start gap-2 text-[11.5px] leading-[15px]">
          <CapturedShortcut text={attempt.text} os={os} />
          {attempt.error ? (
            <span className="text-danger">{attempt.error} Try another combination.</span>
          ) : (
            <span className="text-secondary">Registering…</span>
          )}
        </div>
      )}
      {failing && <p className="mt-1.5 text-[11.5px] leading-[15px] text-danger">{slot.error.message}</p>}
      {!recording && (
        <div className="mt-1 flex gap-3 text-[11.5px]">
          {slot.accelerator && (
            <button type="button" className="text-accent hover:underline" onClick={() => run(() => api.setShortcut(slot.id, null), `${slot.label} cleared.`)}>
              Clear Shortcut
            </button>
          )}
          {slot.accelerator !== slot.defaultAccelerator && (
            <button
              type="button"
              className="text-accent hover:underline"
              onClick={() => run(() => api.resetShortcuts(slot.id), `${slot.label} reset to default.`)}
            >
              Reset to Default{slot.defaultAccelerator ? ` (${slot.defaultAccelerator.replaceAll("+", " + ")})` : ""}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default function ShortcutsPane() {
  const platform = usePlatform();
  const [report, setReport] = useState(null);
  const [recordingSlot, setRecordingSlot] = useState(null);

  useEffect(() => {
    api.getShortcutStatus().then(setReport).catch((e) => toast.error(errorMessage(e)));
  }, []);

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

  const startRecording = async (slotId) => {
    try {
      setReport(await api.beginShortcutRecording());
      setRecordingSlot(slotId);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  const onSaved = useCallback((next, message) => {
    setRecordingSlot(null);
    setReport(next);
    if (message) toast.success(message);
  }, []);

  if (!report) return <PaneHeader pane="shortcuts" />;
  const os = platform?.os;
  const failures = report.slots.filter((s) => s.accelerator && !s.registered).length;

  return (
    <>
      <PaneHeader pane="shortcuts" subtitle="Global shortcuts work from any application." />
      <Group>
        <div className="flex items-start gap-3 px-3.5 py-3">
          <StatusDot tone={!report.available ? "error" : failures ? "warn" : "ok"} />
          <div className="-mt-1 text-[12px] leading-[16px]">
            <div className="text-[13px] text-label">
              {report.available ? `Using ${report.backendLabel}` : "Global shortcuts are unavailable"}
            </div>
            <div className="text-secondary">
              {report.available
                ? failures
                  ? `${failures} shortcut${failures > 1 ? "s" : ""} could not be registered; see below.`
                  : "All assigned shortcuts are registered with the system."
                : report.unavailableReason}
            </div>
          </div>
        </div>
      </Group>

      <Group title="Shortcuts">
        {report.slots.map((slot) => (
          <SlotRow
            key={slot.id}
            slot={slot}
            os={os}
            report={report}
            recording={recordingSlot === slot.id}
            onRecord={() => startRecording(slot.id)}
            onCancel={stopRecording}
            onSaved={onSaved}
          />
        ))}
      </Group>

      <div className="mb-5 flex items-start gap-2 px-1 text-[11.5px] leading-[15px] text-secondary">
        <Info size={13} className="mt-[1px] shrink-0" />
        <span>
          Shortcuts need Ctrl, Alt or Super (F13–F24 may be used alone). Some combinations are reserved by the
          system and never reach Lumio. While recording, Lumio's own shortcuts are paused.
          {os === "linux" && " Tip: you can also bind any system keyboard shortcut to the command “lumio --toggle”."}
        </span>
      </div>

      <div className="flex justify-end">
        <Button
          icon={RotateCcw}
          onClick={async () => {
            try {
              onSaved(await api.resetShortcuts(null), "All shortcuts restored to their defaults.");
            } catch (e) {
              toast.error(errorMessage(e));
            }
          }}
        >
          Reset All Shortcuts
        </Button>
      </div>
    </>
  );
}
