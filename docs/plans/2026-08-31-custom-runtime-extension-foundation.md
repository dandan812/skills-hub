# Custom Runtime Extension Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create an upgrade-friendly local Skills Hub fork whose application identity, data, and updater cannot be overwritten by the official build, with isolated extension points for future runtime evidence.

**Architecture:** Keep the upstream installer, store, and sync engine unchanged. Isolate custom runtime work behind a Rust feature module, a narrow Tauri command, and a standalone React page; only the backend handler list and frontend navigation/view switch may reference the extension.

**Tech Stack:** Tauri 2, Rust 2021, React 19, TypeScript 5.9, Vitest, i18next.

---

### Task 1: Establish the fork boundary

**Files:**
- Create: `CUSTOMIZATION.md`

**Step 1: Record the remote and branch contract**

Document that `upstream` is read-only, `custom/main` owns local changes, and a future writable fork must be added as `origin`.

**Step 2: Record the upgrade procedure**

Document tag-based merge commands, conflict checks, required verification, and the files expected to conflict.

### Task 2: Isolate the custom application

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/core/central_repo.rs`
- Modify: `src-tauri/src/core/tests/central_repo.rs`

**Step 1: Add a failing customization guard**

Assert that the default central directory is `.skillshub-custom`.

**Step 2: Apply the isolated identity and storage defaults**

Use `Skills Hub Custom`, identifier `com.dandan812.skillshubcustom`, and central directory `.skillshub-custom`. Disable official updater checks and updater artifact creation.

**Step 3: Run the focused Rust test**

Run: `cargo test core::central_repo::tests`

Expected: all central repository tests pass.

### Task 3: Add an isolated runtime evidence backend boundary

**Files:**
- Create: `src-tauri/src/runtime_evidence/mod.rs`
- Create: `src-tauri/src/runtime_evidence/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Define the versioned event contract**

Add a serializable `RuntimeEvidenceEventV1` contract for session, skill-loaded, and skill-called events. Do not connect it to the Skills Hub database or sync engine.

**Step 2: Expose a narrow status command**

Add `get_runtime_evidence_status`, initially returning schema version 1, state `not_configured`, no last event, and the supported event kinds.

**Step 3: Register only the module and command**

Add one Rust module declaration and one Tauri handler entry. Do not modify `installer.rs`, `sync_engine.rs`, or `skill_store.rs`.

**Step 4: Test truthful default status and event serialization**

Run: `cargo test runtime_evidence`

Expected: the status remains explicitly unconfigured and the v1 event JSON contract is stable.

### Task 4: Add an independent runtime page

**Files:**
- Create: `src/features/runtime-evidence/types.ts`
- Create: `src/features/runtime-evidence/RuntimeEvidencePage.tsx`
- Modify: `src/components/skills/Header.tsx`
- Modify: `src/App.tsx`
- Modify: `src/i18n/resources.ts`
- Modify: `src/App.css`

**Step 1: Define the frontend DTO separately**

Keep runtime evidence types out of the existing Skills DTO module.

**Step 2: Build a standalone status page**

Invoke only `get_runtime_evidence_status`. Render loading, unavailable, unconfigured, and ready states without claiming runtime evidence exists.

**Step 3: Add the smallest navigation integration**

Add `runtime` to `ActiveView`, one sidebar button, and one view branch. Keep the existing Skills state and sync flow untouched.

**Step 4: Add English and Chinese copy**

Add all runtime page and navigation text to the existing i18n resources.

### Task 5: Verify and commit the baseline

**Files:**
- Verify all changed files

**Step 1: Install locked dependencies**

Run: `npm install`

Expected: dependencies install without changing declared versions.

**Step 2: Run the complete repository check**

Run: `npm run check`

Expected: lint, Vitest, frontend build, rustfmt, Clippy, and Rust tests pass.

**Step 3: Validate Git and isolation configuration**

Run: `git diff --check`, `git remote -v`, and `git branch --show-current`.

Expected: no whitespace errors, only `upstream` exists, and the active branch is `custom/main`.

**Step 4: Commit the custom baseline**

Commit only the planned files with a Conventional Commit message describing the isolated custom foundation.
