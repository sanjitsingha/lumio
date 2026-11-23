import { useCallback, useEffect, useState } from "react";
import { CircleAlert, CircleCheck } from "lucide-react";
import { Button, KeyCaps } from "../common/controls";
import { ShortcutRecorder, CapturedShortcut } from "./ShortcutRecorder";
import { api, errorMessage } from "../../services/tauri";
import { toast } from "../../stores/toastStore";
import { cx } from "../../utils/cx";
import { shortcutKeys } from "../../utils/keys";

/** While `disabled` the shortcut is released, so the row shows the `saved` one, dimmed. */
function SlotRow({ slot, os, available, disabled, saved, recording, onRecord, onCancel, onSaved }) {
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

  const failing = !disabled && !recording && slot.accelerator && !slot.registered && slot.error;
  return (
    <div className={cx("px-3.5 py-2.5", disabled && "opacity-50")}>
      <div className="flex min-h-[28px] items-center gap-4">
        <div className="min-w-0 flex-1 text-[13px] text-label">{slot.label}</div>
        {recording ? (
          <div className="flex items-center gap-2">
            <ShortcutRecorder os={os} busy={busy} onCapture={capture} onCancel={onCancel} />
            <Button onClick={onCancel}>Cancel</Button>
          </div>
        ) : (
          <div className="flex items-center gap-2.5">
            {!disabled &&
              slot.accelerator &&
              (slot.registered ? (
                <CircleCheck size={14} className="text-success" aria-label="Active" />
              ) : (
                <CircleAlert size={14} className="text-danger" aria-label="Not active" />
              ))}
            <KeyCaps keys={shortcutKeys(disabled ? saved : slot.accelerator, os)} />
            <Button onClick={onRecord} disabled={!available || disabled}>
              Change
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
            <span className="text-secondary">Saving…</span>
          )}
        </div>
      )}
      {recording && !attempt && (
        <p className="mt-1.5 text-[11.5px] leading-[15px] text-secondary">
          Press the new shortcut, with Ctrl, Alt or Super.
        </p>
      )}
      {failing && <p className="mt-1.5 text-[11.5px] leading-[15px] text-danger">{slot.error.message}</p>}
      {!recording && !disabled && (
        <div className="mt-1 flex gap-3 text-[11.5px]">
          {slot.accelerator && (
            <button type="button" className="text-accent hover:underline" onClick={() => run(() => api.setShortcut(slot.id, null), `${slot.label} cleared.`)}>
              Clear
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

/** Rows to record, clear and reset the global shortcuts with the given slot ids. */
export function ShortcutEditor({ slotIds, os, shortcuts, disabled = false, saved = {} }) {
  const { report, recordingSlot, startRecording, stopRecording, onSaved } = shortcuts;
  if (!report) return null;
  const slots = report.slots.filter((s) => slotIds.includes(s.id));
  return (
    <>
      {!report.available && (
        <div className="flex gap-2 px-3.5 py-2.5 text-[11.5px] leading-[15px] text-danger">
          <CircleAlert size={14} className="mt-[1px] shrink-0" />
          <span>{report.unavailableReason}</span>
        </div>
      )}
      {slots.map((slot) => (
        <SlotRow
          key={slot.id}
          slot={slot}
          os={os}
          available={report.available}
          disabled={disabled}
          saved={saved[slot.id]}
          recording={recordingSlot === slot.id}
          onRecord={() => startRecording(slot.id)}
          onCancel={stopRecording}
          onSaved={onSaved}
        />
      ))}
    </>
  );
}
