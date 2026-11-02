import { CircleAlert, CircleCheck, Download, ExternalLink, LoaderCircle, RefreshCw, Sparkles } from "lucide-react";
import { Button, Group } from "../common/controls";

function formatBytes(n) {
  if (!n && n !== 0) return "";
  return n > 1024 * 1024 ? `${(n / 1024 / 1024).toFixed(1)} MB` : `${Math.round(n / 1024)} KB`;
}

function timeAgo(seconds) {
  if (!seconds) return null;
  const diff = Math.max(0, Date.now() / 1000 - seconds);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.round(diff / 60)} min ago`;
  if (diff < 86400) return `${Math.round(diff / 3600)} h ago`;
  return new Date(seconds * 1000).toLocaleDateString();
}

/** Update status, release notes and the check / download / install flow. */
export function UpdateCard({ updates }) {
  const { status, check, download, install, openReleasePage } = updates;
  if (!status) return null;
  const phase = status.phase.state;
  const latest = status.latest;
  const busy = ["checking", "downloading", "installing"].includes(phase);

  if (!status.configured) {
    return (
      <Group title="Updates">
        <div className="px-3.5 py-3 text-[12px] leading-[16px] text-secondary">
          Updates are not configured for this build. Set <code className="font-mono">repository</code> in{" "}
          <code className="font-mono">src-tauri/Cargo.toml</code> to your GitHub repository and publish releases with the
          release workflow (see <code className="font-mono">docs/RELEASING.md</code>).
        </div>
      </Group>
    );
  }

  const progress =
    phase === "downloading" && status.phase.total ? Math.min(100, (status.phase.downloaded / status.phase.total) * 100) : null;

  let icon = <RefreshCw size={16} className="text-secondary" />;
  let title = "Check for updates";
  let detail = status.lastChecked ? `Last checked ${timeAgo(status.lastChecked)}.` : "Not checked yet.";
  if (phase === "checking") {
    icon = <LoaderCircle size={16} className="animate-spin text-accent" />;
    title = "Checking for updates…";
  } else if (phase === "upToDate") {
    icon = <CircleCheck size={16} className="text-success" />;
    title = `Lummo ${status.currentVersion} is up to date`;
  } else if (phase === "available" || phase === "ready") {
    icon = <Sparkles size={16} className="text-accent" />;
    title = `Lummo ${latest?.version} is available`;
    detail = `You have ${status.currentVersion}${latest?.date ? ` · released ${latest.date}` : ""}.`;
  } else if (phase === "downloading") {
    icon = <LoaderCircle size={16} className="animate-spin text-accent" />;
    title = `Downloading Lummo ${latest?.version}…`;
    detail = status.phase.total
      ? `${formatBytes(status.phase.downloaded)} of ${formatBytes(status.phase.total)}`
      : formatBytes(status.phase.downloaded);
  } else if (phase === "installing") {
    icon = <LoaderCircle size={16} className="animate-spin text-accent" />;
    title = `Installing Lummo ${latest?.version}…`;
    detail = "Lummo will restart when the installation finishes.";
  } else if (phase === "error") {
    icon = <CircleAlert size={16} className="text-danger" />;
    title = "Update failed";
    detail = status.phase.message;
  }

  const offerInstall = (phase === "available" || phase === "ready" || (phase === "error" && latest)) && latest;

  return (
    <Group title="Updates">
      <div className="px-3.5 py-3">
        <div className="flex items-start gap-3">
          <span className="mt-[1px]">{icon}</span>
          <div className="min-w-0 flex-1">
            <div className="text-[13px] text-label">{title}</div>
            <div className="selectable text-[11.5px] leading-[15px] text-secondary">{detail}</div>
          </div>
          <div className="flex shrink-0 gap-2">
            {offerInstall && status.canInstall && (
              <Button variant="primary" icon={Download} disabled={busy} onClick={install}>
                {phase === "ready" ? "Install & Restart" : "Update Now"}
              </Button>
            )}
            {offerInstall && !status.canInstall && phase !== "ready" && (
              <Button icon={Download} disabled={busy} onClick={download}>
                Download
              </Button>
            )}
            {!offerInstall && (
              <Button busy={phase === "checking"} disabled={busy} onClick={() => check(false)}>
                Check Now
              </Button>
            )}
          </div>
        </div>

        {progress !== null && (
          <div className="mt-2.5 h-[5px] overflow-hidden rounded-full bg-fill-strong">
            <div className="h-full rounded-full bg-accent transition-[width] duration-150" style={{ width: `${progress}%` }} />
          </div>
        )}

        {latest?.notes && (phase === "available" || phase === "ready" || phase === "downloading") && (
          <div className="selectable mt-2.5 max-h-[140px] overflow-auto rounded-[7px] bg-fill px-3 py-2 text-[11.5px] leading-[16px] whitespace-pre-wrap text-label">
            {latest.notes}
          </div>
        )}

        {status.installNote && (offerInstall || !status.canInstall) && (
          <p className="mt-2 text-[11.5px] leading-[15px] text-secondary">{status.installNote}</p>
        )}
        {phase === "ready" && !status.canInstall && (
          <p className="mt-2 text-[11.5px] leading-[15px] text-success">The update was downloaded and its signature verified.</p>
        )}
      </div>
      {status.releasePage && (
        <div className="flex items-center justify-between px-3.5 py-2.5">
          <span className="text-[11.5px] text-secondary">Prefer to update manually? Download the installer yourself.</span>
          <Button variant="plain" icon={ExternalLink} onClick={openReleasePage}>
            Release Page
          </Button>
        </div>
      )}
    </Group>
  );
}
