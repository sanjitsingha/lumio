import { useSyncExternalStore } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { toastStore } from "../../stores/toastStore";

const ICONS = { info: Info, success: CircleCheck, error: CircleAlert };
const TONES = { info: "text-accent", success: "text-success", error: "text-danger" };

export function Toaster() {
  const toasts = useSyncExternalStore(toastStore.subscribe, toastStore.getState);
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-[340px] flex-col gap-2" aria-live="polite">
      <AnimatePresence initial={false}>
        {toasts.map((t) => {
          const Icon = ICONS[t.kind] || Info;
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, y: 12, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, scale: 0.98 }}
              transition={{ duration: 0.18 }}
              className="pointer-events-auto flex items-start gap-2.5 rounded-[10px] border-[0.5px] border-separator bg-raised px-3 py-2.5 shadow-panel"
            >
              <Icon size={16} className={`mt-[1px] shrink-0 ${TONES[t.kind]}`} />
              <span className="selectable flex-1 text-[12.5px] leading-[17px] text-label">{t.message}</span>
              <button type="button" onClick={() => toastStore.dismiss(t.id)} className="text-tertiary hover:text-secondary" aria-label="Dismiss">
                <X size={14} />
              </button>
            </motion.div>
          );
        })}
      </AnimatePresence>
    </div>
  );
}
