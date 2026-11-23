import { CommandGroups } from "../CommandGroups";
import { PaneHeader } from "../PaneHeader";
import { usePlatform } from "../../../hooks/usePlatform";
import { useShortcutRecording } from "../../../hooks/useShortcutRecording";

export default function ShortcutsPane() {
  const platform = usePlatform();
  const shortcuts = useShortcutRecording();
  return (
    <>
      <PaneHeader pane="shortcuts" />
      <CommandGroups os={platform?.os} shortcuts={shortcuts} />
    </>
  );
}
