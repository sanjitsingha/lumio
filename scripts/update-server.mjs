#!/usr/bin/env node
// Local update feed for trying the updater without GitHub.
//
//   npm run update:local -- --version 0.1.1 --build   # build a signed 0.1.1 installer, then serve it
//   npm run update:local -- --version 0.1.1           # serve an installer built earlier
//
// Then start Lummo pointed at it (dev builds accept plain http):
//   LUMMO_UPDATE_ENDPOINT=http://127.0.0.1:8765/latest.json npm run tauri dev
//
// The feed has the same format GitHub releases get from tauri-action.
import { execFileSync } from "node:child_process";
import { createReadStream, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : fallback;
};
const version = option("--version", null);
const port = Number(option("--port", "8765"));
if (!version) {
  console.error("Usage: npm run update:local -- --version <newer version> [--build] [--port 8765]");
  process.exit(1);
}

const windows = process.platform === "win32";
const bundle = windows ? "nsis" : "deb";
const bundleDir = join(root, "src-tauri", "target", "release", "bundle", bundle);

if (args.includes("--build")) {
  const env = { ...process.env };
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    const keyFile = join(homedir(), ".tauri", "lumio.key");
    if (!existsSync(keyFile)) {
      console.error(`No signing key: set TAURI_SIGNING_PRIVATE_KEY or create ${keyFile} (docs/RELEASING.md).`);
      process.exit(1);
    }
    env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, "utf8");
    env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= "";
  }
  console.log(`Building a signed ${bundle} installer for version ${version} (this takes a few minutes)…`);
  execFileSync("npx", ["tauri", "build", "--bundles", bundle, "--config", JSON.stringify({ version })], {
    cwd: root,
    env,
    stdio: "inherit",
    shell: windows,
  });
}

const artifact = existsSync(bundleDir)
  ? readdirSync(bundleDir).find((f) => f.includes(`_${version}_`) && (f.endsWith(".deb") || f.endsWith("-setup.exe")))
  : null;
if (!artifact || !existsSync(join(bundleDir, `${artifact}.sig`))) {
  console.error(`No signed ${bundle} installer for ${version} in ${bundleDir}. Run again with --build.`);
  process.exit(1);
}

const base = `http://127.0.0.1:${port}`;
const entry = { signature: readFileSync(join(bundleDir, `${artifact}.sig`), "utf8"), url: `${base}/${encodeURIComponent(artifact)}` };
// Installed copies look up `<os>-<arch>-<bundle>`; development builds have no
// bundle type and look up the generic `<os>-<arch>` key, so offer both.
const platforms = windows
  ? { "windows-x86_64": entry, "windows-x86_64-nsis": entry }
  : { "linux-x86_64": entry, "linux-x86_64-deb": entry };
const manifest = {
  version,
  notes: `Local test build ${version}, served by scripts/update-server.mjs.`,
  pub_date: new Date().toISOString(),
  platforms,
};
const manifestPath = join(bundleDir, "latest.json");
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));

createServer((req, res) => {
  const name = decodeURIComponent((req.url || "/").split("?")[0].slice(1));
  const file = name === "latest.json" ? manifestPath : join(bundleDir, basename(name));
  if (!name || !existsSync(file) || !statSync(file).isFile()) {
    res.writeHead(404).end();
    return;
  }
  console.log(`${req.method} /${name}`);
  res.writeHead(200, { "Content-Length": statSync(file).size });
  createReadStream(file).pipe(res);
}).listen(port, "127.0.0.1", () => {
  console.log(`\nServing Lummo ${version} (${artifact}) at ${base}/latest.json`);
  console.log(`Start Lummo with:  LUMMO_UPDATE_ENDPOINT=${base}/latest.json npm run tauri dev`);
  console.log("Then open Settings → About → Check Now.  (Ctrl+C stops the server.)\n");
});
