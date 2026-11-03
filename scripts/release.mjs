#!/usr/bin/env node
// Bumps the version, commits, and creates the tag that triggers the GitHub
// release workflow (.github/workflows/release.yml).
//
//   npm run release -- 0.2.0 --notes "What changed"      # or: patch | minor | major
//   npm run release -- patch --notes "Fix…" --push       # also pushes commit + tag
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const files = {
  pkg: join(root, "package.json"),
  cargo: join(root, "src-tauri", "Cargo.toml"),
  lock: join(root, "src-tauri", "Cargo.lock"),
};

function fail(message) {
  console.error(`\n✖ ${message}\n`);
  process.exit(1);
}

function git(...args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

function bump(current, kind) {
  const [, major, minor, patch] = SEMVER.exec(current).map(Number);
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  if (kind === "patch") return `${major}.${minor}.${patch + 1}`;
  return kind;
}

function newer(a, b) {
  const x = SEMVER.exec(a).slice(1).map(Number);
  const y = SEMVER.exec(b).slice(1).map(Number);
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i];
  return false;
}

const args = process.argv.slice(2);
const target = args.find((a) => !a.startsWith("--"));
const notesIndex = args.indexOf("--notes");
const notes = notesIndex >= 0 ? args[notesIndex + 1] : null;
const push = args.includes("--push");
if (!target) fail("Usage: npm run release -- <version|patch|minor|major> --notes \"what changed\" [--push]");

const pkg = JSON.parse(readFileSync(files.pkg, "utf8"));
const cargo = readFileSync(files.cargo, "utf8");
const cargoVersion = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (cargoVersion !== pkg.version) fail(`package.json (${pkg.version}) and Cargo.toml (${cargoVersion}) disagree; fix that first.`);

const next = bump(pkg.version, target);
if (!SEMVER.test(next)) fail(`"${next}" is not a version like 1.2.3.`);
if (!newer(next, pkg.version)) fail(`${next} must be newer than the current ${pkg.version}.`);

try {
  git("rev-parse", "--is-inside-work-tree");
} catch {
  fail("This folder is not a git repository yet. See docs/RELEASING.md → One-time setup.");
}
if (git("status", "--porcelain")) fail("Commit or stash your changes before releasing (git status is not clean).");
if (git("tag", "--list", `v${next}`)) fail(`Tag v${next} already exists.`);

pkg.version = next;
writeFileSync(files.pkg, `${JSON.stringify(pkg, null, 2)}\n`);
writeFileSync(files.cargo, cargo.replace(/^(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m, `$1${next}$2`));
try {
  const lock = readFileSync(files.lock, "utf8");
  writeFileSync(files.lock, lock.replace(/(name = "lummo"\r?\nversion = ")[^"]+(")/, `$1${next}$2`));
} catch {
  // no lockfile yet; cargo creates it on the next build
}

git("add", files.pkg, files.cargo, files.lock);
git("commit", "-m", `Release v${next}`);
git("tag", "-a", `v${next}`, "-m", notes || `Lummo ${next}`);
console.log(`\n✔ Version ${next} committed and tagged v${next}.`);

if (push) {
  git("push");
  git("push", "origin", `v${next}`);
  console.log("✔ Pushed. GitHub Actions is now building the installers (Actions tab → release).");
  console.log("  When it finishes, installed copies of Lummo will offer the update.\n");
} else {
  console.log(`\nPublish it with:\n  git push && git push origin v${next}\n`);
}
