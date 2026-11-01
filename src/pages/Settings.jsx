import { useEffect, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { TriangleAlert, X } from "lucide-react";
import { Sidebar, PANES } from "../components/settings/Sidebar";
import GeneralPane from "../components/settings/panes/GeneralPane";
import SpotlightPane from "../components/settings/panes/SpotlightPane";
import ShortcutsPane from "../components/settings/panes/ShortcutsPane";
import CapsLockPane from "../components/settings/panes/CapsLockPane";
import AppearancePane from "../components/settings/panes/AppearancePane";
import AdvancedPane from "../components/settings/panes/AdvancedPane";
import About from "./About";
import { Toaster } from "../components/common/Toaster";
import { useSettingsState } from "../hooks/useSettings";
import { usePlatform } from "../hooks/usePlatform";
import { settingsStore } from "../stores/settingsStore";
import { EVENTS, subscribe } from "../services/events";
import { useUpdates } from "../hooks/useUpdates";
import { toast } from "../stores/toastStore";

const validPane = (id) => (PANES.some((p) => p.id === id) ? id : "general");

export default function SettingsPage({ appearance }) {
  const { settings, notices } = useSettingsState();
  const platform = usePlatform();
  const [pane, setPane] = useState(() => validPane(window.location.hash.slice(1)));
  const updates = useUpdates();

  useEffect(() => {
    const offNavigate = subscribe(EVENTS.navigate, (id) => setPane(validPane(id)));
    const offNotice = subscribe(EVENTS.notice, (message) => toast.info(message));
    return () => {
      offNavigate();
      offNotice();
    };
  }, []);

  const onCheckUpdates = () => {
    setPane("about");
    updates.check(false);
  };

  if (!settings) return null;

  const content = {
    general: <GeneralPane onCheckUpdates={onCheckUpdates} />,
    spotlight: <SpotlightPane onNavigate={setPane} />,
    shortcuts: <ShortcutsPane />,
    capslock: <CapsLockPane appearance={appearance} />,
    appearance: <AppearancePane appearance={appearance} />,
    advanced: <AdvancedPane />,
    about: <About updates={updates} />,
  }[pane];

  return (
    <div className="flex h-full">
      <Sidebar active={pane} onSelect={setPane} appName={platform?.appName} version={platform?.version} />
      <main className="scroll-area relative flex-1">
        <div className="mx-auto max-w-[620px] px-8 pt-7 pb-10">
          {notices.length > 0 && (
            <div className="mb-5 flex gap-2.5 rounded-[10px] border-[0.5px] border-warning/40 bg-warning/10 px-3.5 py-2.5">
              <TriangleAlert size={15} className="mt-[1px] shrink-0 text-warning" />
              <div className="flex-1 text-[12px] leading-[16px] text-label">
                {notices.map((n) => (
                  <p key={n}>{n}</p>
                ))}
              </div>
              <button type="button" aria-label="Dismiss" onClick={() => settingsStore.dismissNotices()} className="text-secondary">
                <X size={14} />
              </button>
            </div>
          )}
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={pane}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.14, ease: "easeOut" }}
            >
              {content}
            </motion.div>
          </AnimatePresence>
        </div>
      </main>
      <Toaster />
    </div>
  );
}
