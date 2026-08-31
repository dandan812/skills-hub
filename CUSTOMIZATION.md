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

## Upgrade procedure

Merge a reviewed upstream release tag into a temporary upgrade branch:

```powershell
git fetch upstream --tags
git switch custom/main
git switch -c upgrade/vNEXT
git merge vNEXT
npm install
npm run check
```

Resolve conflicts without dropping the application identifier, data-directory,
or updater guards. When verification passes:

```powershell
git switch custom/main
git merge --no-ff upgrade/vNEXT
git push origin custom/main
```

Expected recurring conflict points are deliberately limited to:

- `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`
- `src/App.tsx`
- `src/components/skills/Header.tsx`
- `src/i18n/resources.ts`
- `src/App.css`
