# Releasing and updating Lummo

This is how a change on your machine reaches every installed copy of Lummo:

```
you: npm run release -- patch --notes "…" --push
        │  (bumps version, commits, tags v0.1.1, pushes)
        ▼
GitHub Actions (.github/workflows/release.yml)
        │  builds + signs:  Lummo_0.1.1_x64-setup.exe   (Windows)
        │                   Lummo_0.1.1_amd64.deb        (elementary OS / Ubuntu)
        │                   Lummo_0.1.1_amd64.AppImage   (any Linux)
        │  and writes latest.json (the update feed)
        ▼
GitHub release "Lummo v0.1.1"
        ▼
installed Lummo (every 6 h, or Settings → About → Check Now)
        │  downloads the matching installer, verifies its signature,
        ▼  installs it and restarts itself
```

Updates are **signed**. Installed copies accept only files signed with *your* private key, so nobody else can push an update, even with access to the download URL.

---

## One-time setup

> **Status for this project:** steps 1, 2 and the local part of step 4 are done. The key exists in `~/.tauri/`, `repository = "https://github.com/sanjitsingha/lumio"` is set, and git is initialised with `origin` configured. What remains: **step 3** (add the secret) and **`git push -u origin main`**, then step 5.

### 1. Your signing key (already created)

A key pair was generated on this machine:

| File | What it is |
|---|---|
| `~/.tauri/lumio.key` | **Private key.** Keep it secret and **back it up** (password manager). If you lose it, installed copies can never be updated again; users would have to reinstall. |
| `~/.tauri/lumio.key.pub` | Public key. Already embedded in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). |

To create a new pair (for example on another machine):

```bash
npx tauri signer generate -w ~/.tauri/lumio.key
```

Then paste the content of `lumio.key.pub` into `tauri.conf.json`. Copies installed with the old key will not accept updates signed with the new one.

### 2. Create the GitHub repository

1. On github.com → **New repository**, e.g. `lummo`. It must be **Public**: the app downloads updates anonymously, and private repositories' release files require a login.
2. Put its URL in `src-tauri/Cargo.toml`:
   ```toml
   repository = "https://github.com/<your-user>/lummo"
   ```
   This single line enables the update check, the release-page link and the "Source code" link in About.

### 3. Add the signing key as a secret

GitHub → your repository → **Settings → Secrets and variables → Actions → New repository secret**:

| Name | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | the full content of `~/.tauri/lumio.key`: one line of 348 characters |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | only if you set a password when generating the key (this project's key has none, so do not create it) |

Copy the key from an editor, not from terminal output, where line-wrapping can change characters. In VS Code use **File → Open File… → `~/.tauri/lumio.key`**, then **Ctrl+A, Ctrl+C**, and paste into the secret's value box without adding spaces or line breaks.

The release workflow tests the key first. If it fails, the run page names the problem:

| Message | Meaning |
|---|---|
| *secret is missing* | `TAURI_SIGNING_PRIVATE_KEY` is not set |
| *Invalid input for the given encoding* / *Invalid symbol* / *Invalid padding* | the pasted key was altered, wrapped or cut off; paste it again |
| *Wrong password for that key* | the `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secret does not match (delete it if the key has no password) |

After fixing a secret, open the failed run and click **Re-run all jobs**; no new version is needed.

### 4. Push the code

```bash
cd ~/Spotlight
git init -b main
git add .
git commit -m "Lummo 0.1.0"
git remote add origin https://github.com/<your-user>/lummo.git
git push -u origin main
```

`.gitignore` already excludes `node_modules`, build output and `*.key` files.

### 5. Publish the first version and install it

```bash
git tag -a v0.1.0 -m "First release"
git push origin v0.1.0
```

Open the **Actions** tab and wait for the *release* workflow (about 15 minutes). Then go to **Releases → Lummo v0.1.0** and install it on your machines:

- **elementary OS / Ubuntu:** download `Lummo_0.1.0_amd64.deb`, then `sudo apt install ./Lummo_0.1.0_amd64.deb`. Lummo appears in the Applications menu.
- **Windows:** download and run `Lummo_0.1.0_x64-setup.exe`. It installs per user, without admin rights.
- **Other Linux:** download the `.AppImage`, `chmod +x` it and run it.

From now on these copies update themselves.

---

## Every release

```bash
# 1. work, commit and push as usual
git add . && git commit -m "Better ranking" && git push

# 2. release (version can be patch | minor | major | 1.4.0)
npm run release -- patch --notes "Better ranking for apps; Caps Lock fixes" --push
```

`npm run release` does the following:

- checks that the working tree is clean;
- bumps the version in `package.json`, `src-tauri/Cargo.toml` and `Cargo.lock`;
- commits the bump as "Release v0.1.1";
- creates the tag `v0.1.1`, whose message becomes the release notes users see;
- with `--push`, pushes the commit and the tag, which starts the build.

When the workflow finishes, installed copies notice the update (at the next 6-hourly check, or immediately with **Settings → About → Check Now**). The release notes are shown there, and **Update Now** downloads, verifies, installs and restarts.

| Installed as | What "Update Now" does |
|---|---|
| Windows `.exe` | runs the new installer with a small progress window, then restarts Lummo |
| `.deb` | asks for your password (system package), installs, restarts Lummo |
| AppImage | replaces the AppImage file, restarts |

**Settings → General**:

- Lummo checks for updates shortly after it starts and every few hours, and notifies you when a release appears.
- *Install updates automatically* installs it without asking.

**Manual update** is always possible: About → **Release Page** opens the latest GitHub release, where you can download the installer yourself.

### If something goes wrong

- **The workflow failed:** open the failed job in **Actions**. A common cause is a missing `TAURI_SIGNING_PRIVATE_KEY` secret. Fix it, then delete and re-push the tag:
  `git push --delete origin v0.1.1 && git tag -d v0.1.1`, then tag and push again (or re-run the workflow).
- **"signature is invalid":** the release was signed with a different key than the one in `tauri.conf.json`.
- **"Tag does not match Cargo.toml version":** create tags with `npm run release`, which keeps them in sync.

---

## Testing updates locally (before publishing anything)

The update flow can be exercised on your development machine without GitHub:

```bash
# terminal 1: build a signed "0.1.1" installer and serve a local update feed
npm run update:local -- --version 0.1.1 --build

# terminal 2: run the development app (0.1.0) against that feed
LUMMO_UPDATE_ENDPOINT=http://127.0.0.1:8765/latest.json npm run tauri dev
```

In Lummo: **Settings → About → Check Now**. It reports *Lummo 0.1.1 is available* with the notes, and **Download** fetches the installer and verifies its signature.

Development builds never install over themselves (there is nothing installed to replace). To see the full install-and-restart, install a packaged build first (`npm run build:app`, then `sudo apt install ./src-tauri/target/release/bundle/deb/Lummo_0.1.0_amd64.deb`) and publish a newer version through GitHub as described above.

Without `--build`, `npm run update:local` serves an installer you built earlier with the same command.

---

## Building installers by hand

```bash
npm run build:app                                   # unsigned, for local installs (.deb/AppImage or .exe)
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/lumio.key)" npm run tauri build   # signed + update artifacts
```

A plain `npm run tauri build` needs the signing key in the environment, because updater artifacts are enabled. Use `build:app` when you only want installers.
