import { useState } from "react";
import {
  AppWindow,
  Calculator,
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileCog,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Folder,
  FolderOpen,
  Globe,
  Presentation,
  Settings,
  SlidersHorizontal,
  SquareTerminal,
} from "lucide-react";

const SYMBOLS = {
  settings: { Icon: Settings, tint: "#8E8E93" },
  calculator: { Icon: Calculator, tint: "#FF9500" },
  "folder-open": { Icon: FolderOpen, tint: "#1E90FF" },
  globe: { Icon: Globe, tint: "#30B0C7" },
  "square-terminal": { Icon: SquareTerminal, tint: "#3A3A3C" },
  "sliders-horizontal": { Icon: SlidersHorizontal, tint: "#5856D6" },
  "app-window": { Icon: AppWindow, tint: "#8E8E93" },
};

const FILES = {
  pdf: { Icon: FileText, tint: "#E5484D" },
  document: { Icon: FileText, tint: "#2F80ED" },
  spreadsheet: { Icon: FileSpreadsheet, tint: "#2DA44E" },
  presentation: { Icon: Presentation, tint: "#F2994A" },
  image: { Icon: FileImage, tint: "#AF52DE" },
  video: { Icon: FileVideo, tint: "#FF2D55" },
  audio: { Icon: FileAudio, tint: "#FF6B3D" },
  archive: { Icon: FileArchive, tint: "#A2845E" },
  code: { Icon: FileCode, tint: "#5E5CE6" },
  executable: { Icon: FileCog, tint: "#8E8E93" },
  generic: { Icon: File, tint: "#8E8E93" },
};

const isWindows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

export function appIconUrl(key) {
  return isWindows ? `http://lummo-icon.localhost/app/${key}` : `lummo-icon://localhost/app/${key}`;
}

function Tile({ tint, Icon, size }) {
  return (
    <span
      className="flex items-center justify-center rounded-[8px] text-white shadow-control"
      style={{
        width: size,
        height: size,
        background: `linear-gradient(180deg, color-mix(in srgb, ${tint} 82%, white) 0%, ${tint} 100%)`,
      }}
    >
      <Icon size={size * 0.56} strokeWidth={2} />
    </span>
  );
}

export function ResultIcon({ icon, size = 30, selected = false }) {
  const [failed, setFailed] = useState(false);

  if (icon.type === "app" && !failed) {
    return (
      <img
        src={appIconUrl(icon.key)}
        alt=""
        width={size}
        height={size}
        draggable={false}
        onError={() => setFailed(true)}
        className="object-contain"
        style={{ width: size, height: size }}
      />
    );
  }
  if (icon.type === "symbol" && icon.name === "folder") {
    return (
      <Folder
        size={size}
        strokeWidth={1.4}
        fill={selected ? "rgba(255,255,255,0.25)" : "color-mix(in srgb, #5AC8FA 70%, transparent)"}
        color={selected ? "currentColor" : "#2E9BE6"}
      />
    );
  }
  if (icon.type === "file") {
    const { Icon, tint } = FILES[icon.category] || FILES.generic;
    return <Icon size={size * 0.92} strokeWidth={1.5} color={selected ? "currentColor" : tint} />;
  }
  const symbol = SYMBOLS[icon.name] || SYMBOLS["app-window"];
  return <Tile tint={symbol.tint} Icon={symbol.Icon} size={size} />;
}
