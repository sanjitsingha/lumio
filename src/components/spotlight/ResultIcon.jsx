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
  SquareTerminal,
} from "lucide-react";

/** Lucide icon names the backend sends (quick actions, fallbacks), drawn as tinted tiles. */
export const SYMBOLS = {
  settings: { Icon: Settings, tint: "#8E8E93" },
  calculator: { Icon: Calculator, tint: "#FF9500" },
  "folder-open": { Icon: FolderOpen, tint: "#1E90FF" },
  globe: { Icon: Globe, tint: "#30B0C7" },
  "square-terminal": { Icon: SquareTerminal, tint: "#3A3A3C" },
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

/** `kind` is `app` or `file`, the routes of the backend's icon protocol. */
export function iconUrl(kind, key) {
  return isWindows ? `http://lummo-icon.localhost/${kind}/${key}` : `lummo-icon://localhost/${kind}/${key}`;
}

const radius = (size) => Math.round(size * 0.18);

function Tile({ tint, Icon, size }) {
  return (
    <span
      className="flex items-center justify-center text-white shadow-control"
      style={{
        width: size,
        height: size,
        borderRadius: radius(size),
        background: `linear-gradient(180deg, color-mix(in srgb, ${tint} 82%, white) 0%, ${tint} 100%)`,
      }}
    >
      <Icon size={size * 0.56} strokeWidth={2} />
    </span>
  );
}

function SystemIcon({ src, size, onError }) {
  return (
    <img
      src={src}
      alt=""
      width={size}
      height={size}
      draggable={false}
      onError={onError}
      className="object-contain"
      style={{ width: size, height: size }}
    />
  );
}

/** The white (dark grey in dark mode) rounded square that app, file and folder icons sit on. */
function Plate({ size, children }) {
  return (
    <span
      className="flex items-center justify-center bg-icon-tile shadow-control"
      style={{ width: size, height: size, borderRadius: radius(size) }}
    >
      {children}
    </span>
  );
}

/**
 * The icon of a result, `size` px square. App, file and folder icons sit on a
 * plate of that size; symbols are coloured tiles of it.
 */
export function ResultIcon({ icon, size = 40 }) {
  const [failed, setFailed] = useState(false);
  const inner = Math.round(size * 0.75);

  // apps and files get the system's icon; the glyphs below are the fallback
  if ((icon.type === "app" || icon.type === "file") && icon.key && !failed) {
    return (
      <Plate size={size}>
        <SystemIcon src={iconUrl(icon.type, icon.key)} size={inner} onError={() => setFailed(true)} />
      </Plate>
    );
  }
  // Lummo's logo is a full rounded square, like a plate
  if (icon.type === "lummo") return <SystemIcon src="/app-icon.png" size={size} />;
  if (icon.type === "symbol" && icon.name === "folder") {
    return (
      <Plate size={size}>
        <Folder
          size={Math.round(size * 0.65)}
          strokeWidth={1.4}
          fill="color-mix(in srgb, #5AC8FA 70%, transparent)"
          color="#2E9BE6"
        />
      </Plate>
    );
  }
  if (icon.type === "file") {
    const { Icon, tint } = FILES[icon.category] || FILES.generic;
    return (
      <Plate size={size}>
        <Icon size={Math.round(size * 0.6)} strokeWidth={1.5} color={tint} />
      </Plate>
    );
  }
  const symbol = SYMBOLS[icon.name] || SYMBOLS["app-window"];
  return <Tile tint={symbol.tint} Icon={symbol.Icon} size={size} />;
}
