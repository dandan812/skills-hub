# Skills Hub Custom

This repository is a local customization of
[`qufei1993/skills-hub`](https://github.com/qufei1993/skills-hub).
The upstream MIT license and copyright notices remain unchanged.

## Repository contract

- `upstream` fetches from the official repository; its push URL is deliberately
  set to `DISABLED` so accidental pushes fail locally.
- `custom/main` contains every customization shipped by this build.
- `origin` is intentionally absent until a writable GitHub fork exists.
- Never commit runtime Skill data or credentials to this repository.

After creating `https://github.com/dandan812/skills-hub`, add it with:

```powershell
git remote add origin https://github.com/dandan812/skills-hub.git
git push -u origin custom/main
```

## Isolation guarantees

The custom build uses a distinct application identifier and the default central
repository `~/.skillshub-custom`. Its official application updater is disabled.
Do not run official and custom builds against the same central repository.

Custom releases use `<upstream-version>-custom.<revision>`, starting with
`0.9.1-custom.1`. Keep `package.json`, `tauri.conf.json`, and `Cargo.toml` in
sync with `npm run version:set -- <version>`.

## Runtime extension boundary

Runtime evidence belongs in these custom-owned paths:

- `src-tauri/src/runtime_evidence/`
- `src/features/runtime-evidence/`

Only command registration in `src-tauri/src/lib.rs` and navigation/view wiring
in `src/App.tsx` and `src/components/skills/Header.tsx` should cross into
upstream-owned code. Do not add runtime evidence logic to `installer.rs`,
`sync_engine.rs`, or `skill_store.rs`.

## Upstream upgrades

`scripts/prepare-upstream-update.ps1` is the local, conservative path. It
refuses to run outside `custom/main` or with uncommitted files, fetches a
stable upstream tag, creates `upgrade/vX.Y.Z`, merges it, restores a custom
version such as `0.9.2-custom.1`, and runs the boundary, lint, and web-build
checks. It never resets, force-pushes, or resolves conflicts.

```powershell
npm run upstream:prepare
npm run upstream:prepare -- -Tag v0.9.2
```

Review the resulting branch. `-Apply` merges that reviewed branch into
`custom/main`; pushing remains an explicit separate step.

```powershell
npm run upstream:prepare -- -Tag v0.9.2 -Apply
git push origin custom/main
```

The GitHub Actions workflow **Prepare Upstream Update** uses the same model.
It creates `automation/upstream-vX.Y.Z` and opens a PR into `custom/main`.
It never writes `custom/main` directly. If a candidate branch already exists,
the workflow leaves it unchanged and only creates a missing PR; it will not
force-push over a human review. Conflicts fail before a branch is pushed.

For scheduled runs, set `custom/main` as the fork's GitHub default branch and
allow Actions to create pull requests with its `GITHUB_TOKEN` in repository
settings. Manual runs can optionally specify an upstream stable tag.

## Self-owned app updates

The custom application is intentionally shipped with the app updater disabled
until it has a unique signing key and a writable GitHub fork. This protects a
custom installation from being replaced by official Skills Hub binaries.

After creating the fork, generate a Tauri signing key locally. Keep the private
key outside this repository and use only the `.pub` file here:

```powershell
npx tauri signer generate -w "$env:USERPROFILE\.tauri\skills-hub-custom.key"
npm run updater:configure -- --repo dandan812/skills-hub --pubkey-file "$env:USERPROFILE\.tauri\skills-hub-custom.key.pub"
npm run custom:verify-boundaries -- --require-updater
```

Use the same `owner/repo` as the fork that will run the release workflow. The
configure command stores the public key and exactly one endpoint in
`src-tauri/tauri.conf.json`:

```text
https://github.com/<owner>/<repo>/releases/latest/download/updater.json
```

Commit that public configuration. In the fork's GitHub Actions secrets, set
`TAURI_SIGNING_PRIVATE_KEY` to the private key (or its base64 form), and set
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when the key has a password. Never commit
either value.

Push a matching custom tag, for example `v0.9.1-custom.1`, to publish a signed
GitHub Release and its `updater.json`. The release workflow rejects ordinary
upstream tags, unsigned builds, mismatched versions, and disabled updater
configuration before it starts platform builds. Custom releases are published
as GitHub stable releases, even though their version includes `-custom.N`, so
the `releases/latest/download/updater.json` endpoint resolves correctly.

Expected recurring conflict points are deliberately limited to:

- `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/components/skills/Header.tsx`
- `src/i18n/resources.ts`
- `src/App.css`
