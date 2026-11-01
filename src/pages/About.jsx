import { ExternalLink } from "lucide-react";
import { Button, Group, Row } from "../components/common/controls";
import { UpdateCard } from "../components/settings/UpdateCard";
import { usePlatform } from "../hooks/usePlatform";
import { api, errorMessage } from "../services/tauri";
import { toast } from "../stores/toastStore";

export default function About({ updates }) {
  const info = usePlatform();

  return (
    <>
      <div className="mt-4 mb-7 flex flex-col items-center text-center">
        <img src="/app-icon.svg" alt="" width={96} height={96} draggable={false} />
        <h1 className="mt-3 text-[22px] font-semibold tracking-[-0.01em] text-label">{info?.appName ?? "Lumio"}</h1>
        <p className="text-[12.5px] text-secondary">Version {info?.version ?? "…"}</p>
        <p className="mt-2 max-w-[380px] text-[12.5px] leading-[17px] text-secondary">
          {info?.description ?? "Spotlight-style search and a cursor-anchored Caps Lock indicator."}
        </p>
      </div>

      <UpdateCard updates={updates} />

      <Group>
        <Row label="Developer">
          <span className="selectable text-[12.5px] text-secondary">{info?.authors || "—"}</span>
        </Row>
        <Row label="License">
          <span className="text-[12.5px] text-secondary">{info?.license || "—"}</span>
        </Row>
        <Row label="Source code">
          {info?.repository ? (
            <Button variant="plain" icon={ExternalLink} onClick={() => api.openRepository().catch((e) => toast.error(errorMessage(e)))}>
              {info.repository.replace("https://", "")}
            </Button>
          ) : (
            <span className="text-[12.5px] text-tertiary">Not configured</span>
          )}
        </Row>
        <Row label="Identifier">
          <span className="selectable font-mono text-[11.5px] text-secondary">{info?.identifier}</span>
        </Row>
        <Row label="System">
          <span className="text-[12.5px] text-secondary">
            {info ? `${info.osName}${info.sessionType ? ` · ${info.sessionType}` : ""} · ${info.arch}` : "…"}
          </span>
        </Row>
      </Group>
      <p className="mt-6 text-center text-[11px] text-tertiary">Inspired by macOS. Not affiliated with Apple Inc.</p>
    </>
  );
}
