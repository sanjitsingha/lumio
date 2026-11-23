import { useState } from "react";
import {
  Activity,
  AlarmClock,
  AppWindow,
  Archive,
  ArchiveRestore,
  ArrowLeft,
  ArrowRight,
  AudioLines,
  BadgeCheck,
  BatteryCharging,
  Bell,
  Bluetooth,
  Boxes,
  Braces,
  BrickWall,
  BrushCleaning,
  Calculator,
  CalendarClock,
  Camera,
  Cast,
  ChartLine,
  CircleHelp,
  Clipboard,
  ClipboardX,
  Clock,
  Cloud,
  CloudSun,
  Cog,
  Copy,
  Cpu,
  Database,
  Download,
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
  FolderClock,
  FolderCog,
  FolderOpen,
  Gamepad2,
  Gauge,
  Globe,
  HardDrive,
  Headphones,
  History,
  House,
  Image,
  Images,
  Info,
  KeyRound,
  Keyboard,
  Languages,
  Laptop,
  LayoutGrid,
  LifeBuoy,
  Lock,
  LogOut,
  MemoryStick,
  MessageSquare,
  Mic,
  Minimize2,
  Monitor,
  MonitorCheck,
  MonitorOff,
  MonitorUp,
  Moon,
  MoonStar,
  Mouse,
  Music,
  Network,
  NotebookPen,
  Package,
  PackageX,
  Paintbrush,
  Palette,
  Pencil,
  Play,
  Power,
  Presentation,
  Printer,
  Projector,
  Puzzle,
  RefreshCw,
  Rocket,
  RotateCw,
  Scissors,
  ScreenShare,
  ScrollText,
  Search,
  Server,
  Settings,
  Settings2,
  Shield,
  ShieldCheck,
  SkipBack,
  SkipForward,
  SlidersHorizontal,
  Smartphone,
  Smile,
  Speech,
  Square,
  SquareChevronRight,
  SquarePlus,
  SquareTerminal,
  SquareX,
  Store,
  SunMoon,
  Terminal,
  Trash,
  Trash2,
  Type,
  Usb,
  UserCog,
  Users,
  Video,
  Volume1,
  Volume2,
  VolumeX,
  Wifi,
  Wrench,
  ZoomIn,
} from "lucide-react";

/** System commands share one quiet tile. */
const SYSTEM = "#636366";
const system = (Icon) => ({ Icon, tint: SYSTEM });

/** Lucide icon names the backend sends (quick actions, fallbacks), drawn as tinted tiles. */
export const SYMBOLS = {
  settings: { Icon: Settings, tint: "#8E8E93" },
  calculator: { Icon: Calculator, tint: "#FF9500" },
  "folder-open": { Icon: FolderOpen, tint: "#1E90FF" },
  globe: { Icon: Globe, tint: "#30B0C7" },
  "square-terminal": { Icon: SquareTerminal, tint: "#3A3A3C" },
  "app-window": { Icon: AppWindow, tint: "#8E8E93" },
  activity: system(Activity),
  "alarm-clock": system(AlarmClock),
  archive: system(Archive),
  "archive-restore": system(ArchiveRestore),
  "arrow-left": system(ArrowLeft),
  "arrow-right": system(ArrowRight),
  "audio-lines": system(AudioLines),
  "badge-check": system(BadgeCheck),
  "battery-charging": system(BatteryCharging),
  bell: system(Bell),
  bluetooth: system(Bluetooth),
  boxes: system(Boxes),
  braces: system(Braces),
  "brick-wall": system(BrickWall),
  "brush-cleaning": system(BrushCleaning),
  "calendar-clock": system(CalendarClock),
  camera: system(Camera),
  cast: system(Cast),
  "chart-line": system(ChartLine),
  "circle-help": system(CircleHelp),
  clipboard: system(Clipboard),
  "clipboard-x": system(ClipboardX),
  clock: system(Clock),
  cloud: system(Cloud),
  "cloud-sun": system(CloudSun),
  cog: system(Cog),
  copy: system(Copy),
  cpu: system(Cpu),
  database: system(Database),
  download: system(Download),
  "file-cog": system(FileCog),
  "file-text": system(FileText),
  "folder-clock": system(FolderClock),
  "folder-cog": system(FolderCog),
  "gamepad-2": system(Gamepad2),
  gauge: system(Gauge),
  "hard-drive": system(HardDrive),
  headphones: system(Headphones),
  history: system(History),
  house: system(House),
  image: system(Image),
  images: system(Images),
  info: system(Info),
  "key-round": system(KeyRound),
  keyboard: system(Keyboard),
  languages: system(Languages),
  laptop: system(Laptop),
  "layout-grid": system(LayoutGrid),
  "life-buoy": system(LifeBuoy),
  lock: system(Lock),
  "log-out": system(LogOut),
  "memory-stick": system(MemoryStick),
  "message-square": system(MessageSquare),
  mic: system(Mic),
  "minimize-2": system(Minimize2),
  monitor: system(Monitor),
  "monitor-check": system(MonitorCheck),
  "monitor-off": system(MonitorOff),
  "monitor-up": system(MonitorUp),
  moon: system(Moon),
  "moon-star": system(MoonStar),
  mouse: system(Mouse),
  music: system(Music),
  network: system(Network),
  "notebook-pen": system(NotebookPen),
  package: system(Package),
  "package-x": system(PackageX),
  paintbrush: system(Paintbrush),
  palette: system(Palette),
  pencil: system(Pencil),
  play: system(Play),
  power: system(Power),
  printer: system(Printer),
  projector: system(Projector),
  puzzle: system(Puzzle),
  "refresh-cw": system(RefreshCw),
  rocket: system(Rocket),
  "rotate-cw": system(RotateCw),
  scissors: system(Scissors),
  "screen-share": system(ScreenShare),
  "scroll-text": system(ScrollText),
  search: system(Search),
  server: system(Server),
  "settings-2": system(Settings2),
  shield: system(Shield),
  "shield-check": system(ShieldCheck),
  "skip-back": system(SkipBack),
  "skip-forward": system(SkipForward),
  "sliders-horizontal": system(SlidersHorizontal),
  smartphone: system(Smartphone),
  smile: system(Smile),
  speech: system(Speech),
  square: system(Square),
  "square-chevron-right": system(SquareChevronRight),
  "square-plus": system(SquarePlus),
  "square-x": system(SquareX),
  store: system(Store),
  "sun-moon": system(SunMoon),
  terminal: system(Terminal),
  trash: system(Trash),
  "trash-2": system(Trash2),
  type: system(Type),
  usb: system(Usb),
  "user-cog": system(UserCog),
  users: system(Users),
  video: system(Video),
  "volume-1": system(Volume1),
  "volume-2": system(Volume2),
  "volume-x": system(VolumeX),
  wifi: system(Wifi),
  wrench: system(Wrench),
  "zoom-in": system(ZoomIn),
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

const radius = (size) => Math.round(size * 0.125);

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

/**
 * The icon of a result, `size` px square. App, file and folder icons are drawn
 * bare; symbols are coloured tiles of that size.
 */
export function ResultIcon({ icon, size = 40 }) {
  const [failed, setFailed] = useState(false);
  const glyph = Math.round(size * 0.85);

  // apps and files get the system's icon; the glyphs below are the fallback
  if ((icon.type === "app" || icon.type === "file") && icon.key && !failed) {
    return <SystemIcon src={iconUrl(icon.type, icon.key)} size={size} onError={() => setFailed(true)} />;
  }
  if (icon.type === "lummo") return <SystemIcon src="/app-icon.png" size={size} />;
  if (icon.type === "symbol" && icon.name === "folder") {
    return (
      <Folder size={glyph} strokeWidth={1.2} fill="color-mix(in srgb, #5AC8FA 70%, transparent)" color="#2E9BE6" />
    );
  }
  if (icon.type === "file") {
    const { Icon, tint } = FILES[icon.category] || FILES.generic;
    return <Icon size={glyph} strokeWidth={1.25} color={tint} />;
  }
  const symbol = SYMBOLS[icon.name] || SYMBOLS["app-window"];
  return <Tile tint={symbol.tint} Icon={symbol.Icon} size={size} />;
}
