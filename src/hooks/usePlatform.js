import { useEffect, useState } from "react";
import { api } from "../services/tauri";

let cached = null;
let pending = null;

/** Platform + app metadata (cached for the window's lifetime). */
export function usePlatform() {
  const [info, setInfo] = useState(cached);
  useEffect(() => {
    if (cached) return undefined;
    let alive = true;
    pending = pending || api.getPlatformInfo().then((i) => (cached = i));
    pending.then((i) => alive && setInfo(i)).catch(() => {});
    return () => {
      alive = false;
    };
  }, []);
  return info;
}
