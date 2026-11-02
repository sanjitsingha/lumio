import { useCallback, useEffect, useState } from "react";
import { LoaderCircle, Plus, RefreshCw, RotateCcw, X } from "lucide-react";
import { Button, Group, Row, Select, Slider, ToggleRow } from "../../common/controls";
import { PaneHeader } from "../PaneHeader";
import { ShortcutEditor } from "../../shortcuts/ShortcutEditor";
import { useSettings, useUpdateSettings } from "../../../hooks/useSettings";
import { usePlatform } from "../../../hooks/usePlatform";
import { api, errorMessage } from "../../../services/tauri";
import { toast } from "../../../stores/toastStore";

const SPOTLIGHT_SHORTCUTS = ["spotlight.toggle"];

function formatTime(seconds) {
  if (!seconds) return "never";
  return new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function useIndexStatus() {
  const [status, setStatus] = useState(null);
  const refresh = useCallback(() => api.getIndexStatus().then(setStatus).catch(() => {}), []);
  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [refresh]);
  return [status, refresh];
}

function ListEditor({ items, placeholder, onChange, validate, mono = false }) {
  const [draft, setDraft] = useState("");
  const add = () => {
    const value = draft.trim();
    if (!value) return;
    const problem = validate?.(value);
    if (problem) {
      toast.error(problem);
      return;
    }
    if (!items.includes(value)) onChange([...items, value]);
    setDraft("");
  };
  return (
    <div className="px-3.5 py-2.5">
      <div className="flex flex-wrap gap-1.5">
        {items.map((item) => (
          <span
            key={item}
            className={`inline-flex max-w-full items-center gap-1 rounded-[6px] bg-fill py-[3px] pr-1 pl-2 text-[12px] text-label ${mono ? "font-mono text-[11.5px]" : ""}`}
          >
            <span className="truncate">{item}</span>
            <button
              type="button"
              aria-label={`Remove ${item}`}
              onClick={() => onChange(items.filter((i) => i !== item))}
              className="rounded p-0.5 text-secondary hover:bg-fill-strong hover:text-label"
            >
              <X size={12} />
            </button>
          </span>
        ))}
        {items.length === 0 && <span className="text-[12px] text-tertiary">None</span>}
      </div>
      <form
        className="mt-2 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={placeholder}
          spellCheck={false}
          className="h-[24px] flex-1 rounded-[6px] bg-fill px-2 text-[12px] text-label outline-none placeholder:text-tertiary focus:ring-2 focus:ring-accent/40"
        />
        <Button type="submit" icon={Plus} disabled={!draft.trim()}>
          Add
        </Button>
      </form>
    </div>
  );
}

const isAbsolute = (p) => p.startsWith("/") || /^[a-zA-Z]:[\\/]/.test(p) || p.startsWith("\\\\");

/** "System default" plus the installed browsers; a saved one that is gone stays listed as such. */
function useBrowserOptions(current) {
  const [browsers, setBrowsers] = useState([]);
  useEffect(() => {
    api.getBrowsers().then(setBrowsers).catch(() => setBrowsers([]));
  }, []);
  const options = [{ value: "", label: "System default" }, ...browsers.map((b) => ({ value: b.id, label: b.name }))];
  if (current && !browsers.some((b) => b.id === current)) options.push({ value: current, label: `${current} (not found)` });
  return options;
}

export default function SpotlightPane() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const platform = usePlatform();
  const [status, refreshStatus] = useIndexStatus();
  const s = settings.spotlight;
  const browserOptions = useBrowserOptions(s.webBrowser);
  const set = (patch) => update({ spotlight: patch });
  const [maxResults, setMaxResults] = useState(s.maxResults);
  const [delay, setDelay] = useState(s.searchDelayMs);
  useEffect(() => setMaxResults(s.maxResults), [s.maxResults]);
  useEffect(() => setDelay(s.searchDelayMs), [s.searchDelayMs]);

  return (
    <>
      <PaneHeader pane="spotlight" subtitle="Find apps, files, folders and actions from anywhere." />
      <Group>
        <ToggleRow label="Enable Spotlight" checked={s.enabled} onChange={(v) => set({ enabled: v })} />
      </Group>

      <Group title="Keyboard shortcut">
        <ShortcutEditor slotIds={SPOTLIGHT_SHORTCUTS} os={platform?.os} />
      </Group>

      <Group title="Search in">
        <ToggleRow label="Applications" checked={s.searchApplications} onChange={(v) => set({ searchApplications: v })} disabled={!s.enabled} />
        <ToggleRow label="Files" checked={s.searchFiles} onChange={(v) => set({ searchFiles: v })} disabled={!s.enabled} />
        <ToggleRow label="Folders" checked={s.searchFolders} onChange={(v) => set({ searchFolders: v })} disabled={!s.enabled} />
        <ToggleRow
          label="Quick actions"
          description="Settings, Calculator, File Manager, Browser and Terminal."
          checked={s.searchActions}
          onChange={(v) => set({ searchActions: v })}
          disabled={!s.enabled}
        />
        <ToggleRow
          label="Web search"
          description="Ends every search with “Search Google for …”."
          checked={s.webSearch}
          onChange={(v) => set({ webSearch: v })}
          disabled={!s.enabled}
        />
        <Row
          label="Browser for web searches"
          description="Missing or uninstalled browsers fall back to the system default."
          disabled={!s.enabled || !s.webSearch}
        >
          <Select
            value={s.webBrowser}
            options={browserOptions}
            onChange={(v) => set({ webBrowser: v })}
            disabled={!s.enabled || !s.webSearch}
          />
        </Row>
      </Group>

      <Group title="Results">
        <Row label="Maximum results" description="Total number of results shown at once.">
          <Slider value={maxResults} min={5} max={60} onChange={setMaxResults} onCommit={(v) => set({ maxResults: v })} />
        </Row>
        <Row label="Search delay" description="Wait this long after typing before searching. Lower feels faster.">
          <Slider
            value={delay}
            min={0}
            max={500}
            step={10}
            onChange={setDelay}
            onCommit={(v) => set({ searchDelayMs: v })}
            format={(v) => `${v} ms`}
          />
        </Row>
      </Group>

      <Group title="History" footer="History is stored only on this computer and is used for Recent items, suggestions and ranking.">
        <ToggleRow label="Remember search history" checked={s.keepHistory} onChange={(v) => set({ keepHistory: v })} />
        <Row label="Clear search history" description="Forget recent items, past searches and usage counts.">
          <Button
            onClick={() =>
              api
                .clearSearchHistory()
                .then(() => toast.success("Search history cleared."))
                .catch((e) => toast.error(errorMessage(e)))
            }
          >
            Clear History
          </Button>
        </Row>
      </Group>

      <Group title="File index">
        <Row
          label={
            status?.indexing ? (
              <span className="inline-flex items-center gap-1.5">
                <LoaderCircle size={13} className="animate-spin" /> Indexing…
              </span>
            ) : (
              "Index status"
            )
          }
          description={
            status
              ? `${status.files.toLocaleString()} files · ${status.folders.toLocaleString()} folders · ${status.apps} apps · updated ${formatTime(status.lastIndexed)}${status.limitReached ? " · entry limit reached" : ""}${status.watching ? " · watching for changes" : ""}`
              : "Loading…"
          }
        >
          <Button
            icon={RefreshCw}
            onClick={() => {
              api.rebuildIndex().then(refreshStatus);
              toast.info("Rebuilding the search index…");
            }}
          >
            Rebuild
          </Button>
        </Row>
        {status?.error && <div className="px-3.5 py-2 text-[11.5px] text-danger">{status.error}</div>}
        <ToggleRow
          label="Include hidden files and folders"
          description="Items whose names start with a dot (Linux) or that are marked hidden (Windows)."
          checked={s.indexHidden}
          onChange={(v) => set({ indexHidden: v })}
        />
      </Group>

      <Group title="Folders to index" footer="Leave empty to index your home folder. Use absolute paths.">
        <ListEditor
          items={s.indexRoots}
          mono
          placeholder={platform?.os === "windows" ? "C:\\Users\\you\\Documents" : "/home/you/Documents"}
          validate={(v) => (isAbsolute(v) ? null : "Enter an absolute folder path.")}
          onChange={(indexRoots) => set({ indexRoots })}
        />
      </Group>

      <Group
        title="Exclusions"
        footer="Folder or file names (wildcards like *.tmp allowed) or absolute paths that Spotlight skips."
      >
        <ListEditor items={s.exclusions} placeholder="node_modules" onChange={(exclusions) => set({ exclusions })} />
      </Group>
      <div className="mb-2 flex justify-end">
        <Button
          icon={RotateCcw}
          onClick={() =>
            api
              .resetSettings("spotlight")
              .then(() => toast.success("Spotlight settings restored."))
              .catch((e) => toast.error(errorMessage(e)))
          }
        >
          Restore Defaults
        </Button>
      </div>
    </>
  );
}
