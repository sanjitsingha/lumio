# Lummo: rules for working in this repo

These rules are in the repo on purpose, so they hold on every machine (home, office) and in every session.

## Publishing is never automatic

Lummo is published to the Microsoft Store and to GitHub releases, and people use it. A release reaches all of them and cannot be taken back.

- **Never publish on your own, however the request is worded.** That covers `npm run release`, creating or pushing a version tag (`v0.1.x`), editing `latest.json`, changing the version in `package.json` / `Cargo.toml` / `Cargo.lock`, and anything that starts `.github/workflows/release.yml`.
- **When the user says "publish", "release", "ship", "push and release" or anything like it, do not run anything yet.** Ask one clear question first and wait for the answer. Ask what they mean, for example:
  1. Only push the code to GitHub, with no release (safe, any time)?
  2. Publish a new version? If so: which version number, which release notes, and is Microsoft Store certification finished or still running?
- Even after "yes, publish", the release script itself asks for the exact phrase `publish <version>` at a terminal, which only the user can type. Do not try to get around it (no faking a terminal, no pushing the tag by hand, no `--no-verify`, no editing the hook or the script to skip it). Tell the user to run `npm run release` themselves.
- **Pushing commits to `main` is not publishing** and is fine when the user asks to push. It only runs the check build.
- Never push a tag, delete a remote tag, or rewrite published history.

## Other standing rules

- Work for Linux first when the user is on Linux; touch Windows-only code only when the same change applies there.
- The Weather desktop widget is unreleased and stays uncommitted: never commit or push it.
- Fetch (`git fetch`) before pushing; releases and Store work also come from the Windows machine.
- Never commit signing keys, and never delete the user's own files.
