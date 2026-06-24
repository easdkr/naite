# naite UI Status and Vertical Fix - Notepad

## Project Context
- naite: native Rust desktop Git client (iced 0.13.1, gix, tokio, rfd)
- Workspace: crates/naite-core (Git domain, no UI) + crates/naite-app (iced UI)
- iced 0.13.1 pinned - check pinned version before applying docs.rs/latest examples
- AGENTS.md conventions: Elm-style (Message → update → state + Task), pure view/widget, no inline Style, no new deps, ROW_HEIGHT shared, local-first

## Inherited Wisdom (from planning session)
- error_card REPLACING commit list is a critical UX bug (widgets/commit_list.rs:92-95) - Task 19 fixes
- modal.rs has NO max_height + NO scrollable wrapper - Task 7 fixes
- OperationTracker in App state replaces single operation.loading bool - Tasks 5/6
- release_prep prepare() is monolithic 7-step spawn_blocking - Task 21 splits to per-step Task::perform chain (Option C, no core changes)
- Animation primitives are pure functions, hoistable from release_prep.rs:482-548 to common.rs - Task 1
- 4-row height constants should converge: prompts.rs:560 (34.0), sidebar folder_item (24.0), tab_strip menu_item (24.0) - Task 8
- TERMINAL_LINE_HEIGHT=15 too tight for CJK, TERMINAL_PANEL_CHROME=110 hardcoded - Task 9
- main.rs:53 has min_size 900x600, raise to 1024x640 - Task 11
- 2-tier error severity: Recoverable → bottom bar, Fatal → blocking card - Task 19

## File:Line Map (Critical Anchors)
- `crates/naite-app/src/state.rs:137-154` - OperationState
- `crates/naite-app/src/state.rs:156-160` - TransientStatus
- `crates/naite-app/src/state.rs:311-354` - ReleasePrepState + ReleasePrepPhase
- `crates/naite-app/src/state.rs:329-330` - active_action/completed_actions pattern
- `crates/naite-app/src/widgets/release_prep.rs:140-154` - hardcoded 3-line progress (THE GAP)
- `crates/naite-app/src/widgets/release_prep.rs:393-454` - action_row pattern (mirror target)
- `crates/naite-app/src/widgets/release_prep.rs:466-480` - progress_line helper (stateless)
- `crates/naite-app/src/widgets/release_prep.rs:482-548` - 4 animation primitives (Task 1)
- `crates/naite-app/src/widgets/modal.rs:42-75` - modal primitive (no max_height)
- `crates/naite-app/src/widgets/prompts.rs:485-494` - rebase_prompt_preview scroll pattern
- `crates/naite-app/src/widgets/prompts.rs:560` - hardcoded 34.0 (Task 8)
- `crates/naite-app/src/widgets/sidebar.rs:30` - SIDEBAR_REF_ROW_HEIGHT 26.0
- `crates/naite-app/src/widgets/sidebar.rs:1122,1136` - hardcoded 24.0 (Task 8)
- `crates/naite-app/src/widgets/tab_strip.rs:23` - TAB_HEIGHT 28.0
- `crates/naite-app/src/widgets/tab_strip.rs:181` - hardcoded 24.0 (Task 8)
- `crates/naite-app/src/widgets/terminal.rs:24-33` - terminal constants
- `crates/naite-app/src/widgets/terminal.rs:75` - status_chip display
- `crates/naite-app/src/widgets/terminal.rs:224` - terminal line height usage
- `crates/naite-app/src/widgets/commit_list.rs:92-95` - error_card swap (CRITICAL BUG)
- `crates/naite-app/src/widgets/commit_list.rs:716-735` - subject_with_labels (Ellipsis target)
- `crates/naite-app/src/widgets/commit_list.rs:369-399` - AUTHOR/WHEN columns
- `crates/naite-app/src/widgets/detail_pane.rs:1268-1290` - hunk_header actions
- `crates/naite-app/src/widgets/toolbar.rs:16` - TOOLBAR_HEIGHT 44.0
- `crates/naite-app/src/widgets/toolbar.rs:121-141` - "Loading..." display
- `crates/naite-app/src/features/release_prep/task.rs:34-109` - prepare() (split target)
- `crates/naite-app/src/features/release_prep/update.rs:412-444` - ActionRequested→ActionDone chain
- `crates/naite-app/src/features/terminal/update.rs:670-677` - panel_chrome calc
- `crates/naite-app/src/main.rs:53` - window min_size 900x600
- `crates/naite-app/src/app.rs:487-505` - error_recovery_action
- `crates/naite-app/src/subscription.rs:18,40-48` - ReleasePrepTick 80ms
- `crates/naite-app/src/subscription.rs:36-39` - TransientStatusTick 250ms
- `crates/naite-app/src/message.rs:204-208` - From<release_prep::Message> pattern
- `crates/naite-core/src/ops/release.rs` - 7 release pub fns (UNCHANGED in v1)

## Wave Progress
(append per-wave)

## Wave 1 - Task 1 (animation hoist) findings
- **Decision:** moved 4 animation primitives + 4 tightly-coupled helpers (2 consts, 2 style fns) to `widgets/common.rs`. The 4 helpers are private to `common.rs` because they exist solely to support `moving_progress_bar` and have zero references outside it (verified via grep).
- **Visibility:** animation primitives use `pub` (not `pub(super)`) because they need to be reachable through the `pub use` re-export in `widgets/mod.rs` — `pub(super)` items cannot be re-exported (`E0364`).
- **Re-export warning:** `cargo test` emits `unused imports: animated_dots, ease_in_out_sine, moving_progress_bar, spinner_frame` at `widgets/mod.rs:28`. This is benign and pre-positioned for Wave 3 widgets (Tasks 12-15) which will consume via `crate::widgets::{...}`. Mirrors how `pub use common::ErrorRecovery` exists today with no internal usage warning because `app.rs` is already a consumer.
- **Pre-existing breakage unrelated to Task 1:** the branch had uncommitted changes in `crates/naite-app/src/tests.rs` (+334 lines) and `crates/naite-app/src/theme.rs` (+11 lines) before this task started. The `tests.rs` snapshot helpers reference `WorktreeStatusDetail::as_deref` and `BranchSyncStatus::is_dirty` which do not exist in the current state of `naite-core`. With those pre-existing changes stashed, Task 1's full test run reports `365 + 265 = 630` tests passing, 0 failures. After verifying, restored the pre-existing changes via `git stash pop`.
- **Test command for this task:** `export PATH="$HOME/.cargo/bin:$PATH" && cargo test --workspace --locked` was run twice: once with pre-existing changes stashed (clean — 630/630 pass) and once with them present (expected compile failure in `tests.rs`, not related to Task 1).
- **`ease_in_out_sine` import in `release_prep.rs`:** removed from the import list because the only consumer was the now-relocated `moving_progress_bar`. Rust flagged it as `unused_imports` until removed. The other 3 (`spinner_frame`, `animated_dots`, `moving_progress_bar`) are still actively called by `release_prep.rs` at lines 125, 143, 157, 175, 205, 209, 346, 404, 412.
- **`release_prep.rs` import cleanup:** `Background`, `Border`, and `Theme` were dropped from the `use iced::{...}` line because their only consumers were `progress_track_style` and `progress_segment_style` (both relocated). `Color` was retained because `release_prep.rs:184` still uses `Color::TRANSPARENT` directly.
- **No new tests added** (per task spec "All existing tests still pass"). Evidence files at `.omo/evidence/task-1-animation-hoist-test.txt` and `.omo/evidence/task-1-release-prep-regression.txt`.

## Wave 1 — Task 2 (theme.rs layout/status/history constants)

### Done
- Added 7 `pub const` tokens to `crates/naite-app/src/theme.rs` in a single
  "Layout, history, and timing tokens" section between the R_* radius scale
  (line 212) and `naite_dark()` (line 226):
    - MIN_WINDOW_WIDTH  = 1024.0
    - MIN_WINDOW_HEIGHT = 640.0
    - STATUS_BAR_HEIGHT = 24.0
    - MAX_MODAL_HEIGHT  = 600.0
    - OVERLAY_TRIGGER_SECS = 2
    - OP_HISTORY_CAP    = 50
    - TOAST_SUCCESS_TTL_SECS = 3
- Each constant has exactly 1 definition site (grep evidence at
  `.omo/evidence/task-2-constants-grep.txt`).
- Existing constants (SP_*/FS_*/R_*) and TOOLBAR_HEIGHT (toolbar.rs:16)
  left untouched per Task 2 scope.
- Section header style matches existing theme.rs sections
  (`// Spacing scale…`, `// Type scale…`, `// Radius scale…`).
- File scope honoured: only `crates/naite-app/src/theme.rs` modified.

### Inherited Build Issue (NOT caused by Task 2)
- `cargo build -p naite-app` fails with 4× E0364 (`animated_dots`,
  `ease_in_out_sine`, `moving_progress_bar`, `spinner_frame` is private,
  cannot be re-exported from `widgets/mod.rs:28`).
- Root cause: uncommitted Task 1 work moved these helpers from
  `widgets/release_prep.rs` into `widgets/common.rs` as `pub fn` and added
  `pub use common::{...}` in `widgets/mod.rs:28`, but left `mod common;`
  (private) at `widgets/mod.rs:6`. Fix is one-line: `pub mod common;`.
- Theme.rs itself introduces no errors or warnings. The 7 new constants
  are pure additions with no usage sites yet (downstream tasks 5/7/11/14/15/20
  will start referencing them).
- Task 2 verification therefore recorded as: constants defined + grep-clean,
  with build failure explicitly attributed to Task 1 leftover (see
  `.omo/evidence/task-2-build.txt` trailing note).

### Anchors Created
- `crates/naite-app/src/theme.rs:217-223` — 7 new pub const layout/status/history tokens
- `crates/naite-app/src/theme.rs:214-216` — section header (matches existing style)

### Downstream Reference Targets (Wave 2+)
- Task 5  OperationTracker — uses `OP_HISTORY_CAP` (50)
- Task 7  modal.rs max_height wrapper — uses `MAX_MODAL_HEIGHT` (600.0)
- Task 11 main.rs window min_size — uses `MIN_WINDOW_WIDTH`/`HEIGHT` (1024.0/640.0)
- Task 14 toast auto-dismiss — uses `TOAST_SUCCESS_TTL_SECS` (3)
- Tasks 15/20 central overlay — uses `OVERLAY_TRIGGER_SECS` (2)
- Tasks 12/13 status bars (top + bottom) — uses `STATUS_BAR_HEIGHT` (24.0)

## Wave N Findings — Task 3: release_prep prepare() regression baseline

**Baseline tests added:** 4 tests in `crates/naite-app/src/tests.rs` under module `release_prep_prepare_baseline` (lines 8865-9178). All 4 pass against the CURRENT (unmodified) `prepare()` body.

### Test design notes
- **Real local git repos**: Tests use `Command::new("git")` directly (mirroring `crates/naite-core/src/test_helpers.rs` patterns). `test_helpers.rs` is `pub(crate)` to naite-core only, so naite-app tests inline minimal helpers `temp_dir` + `run_git` + `setup_synced_repo`.
- **Helper struct with Drop**: `ReleasePrepTestRepo` holds the bare remote dir + the clone parent dir and removes both on drop, so tests clean up automatically.
- **Sync success setup**: bare `origin` remote → source repo with `main` (initial) + `staging` (one extra "staging" commit) both pushed → clone from `origin/staging` so the local clone is in sync with the remote tracking refs.
- **Sync failure setup**: only push `main` to remote; locally create `staging` with a divergent commit. `sync_release_branches_with_remote` then tries to `force_sync_remote_branch("refs/remotes/origin/staging")`, which fails inside `ensure_remote_branch` because the remote tracking ref is missing.
- **Busy operation setup**: write a fake `MERGE_HEAD` into the resolved `.git` dir; `operation_state().is_busy()` then returns true.

### Sync check failure path analysis
The literal `format_sync_failure(&sync_check)` branch (line 56 of `prepare()`) is hard to reach through a single-threaded test: `sync_release_branches_with_remote` calls `force_sync_remote_branch` for any branch where `is_ready()` is false, which hard-resets the local branch to the remote tracking ref. After that succeeds, `check_release_sync` always reports both branches ready.

The pragmatic test in `release_prep_prepare_baseline_sync_failure` triggers a failure earlier in the sync phase (the `sync_release_branches_with_remote` step itself fails when the remote tracking ref is missing). The assertion accepts any of `"show-ref"`, `"refs/remotes/origin/staging"`, or `"Release branches"` so it stays robust across git versions and also stays correct if Task 21 later makes `format_sync_failure` more reachable (e.g., when the new per-step pipeline checks sync after every force-sync instead of once at the end).

### Verification
- `cargo test --workspace --locked -- release_prep_prepare_baseline` → 4 passed, 0 failed
- `cargo test --workspace --locked` → 369 naite-app + 265 naite-core passed, 0 regressions
- `cargo fmt --all -- --check` → clean
- `cargo clippy --workspace --all-targets --locked --tests -- -D warnings` → clean for tests.rs only

### File scope compliance
- Only `crates/naite-app/src/tests.rs` was modified (plus the required evidence file `.omo/evidence/task-3-baseline-test.txt` and this notepad append).
- `prepare()` body at `crates/naite-app/src/features/release_prep/task.rs:34-109` is untouched.
- No new dependencies were added.

### Known pre-existing issue (not from this task)
There are uncommitted modifications to `crates/naite-app/src/theme.rs` and `crates/naite-app/src/widgets/{common,mod,release_prep}.rs` from earlier wave work. Those changes cause `cargo clippy --workspace --all-targets -- -D warnings` to flag unused constants in `theme.rs` (`MIN_WINDOW_WIDTH`, `MIN_WINDOW_HEIGHT`, etc.). They are unrelated to this task and were left alone per the file-scope constraint.

### Lock-down for Task 21 (Wave 5 refactor)
Task 21 will split the single `tokio::task::spawn_blocking` closure into 7 per-step async fns. The 4 baseline tests pin the observable behavior:
1. Success path locks `PrepareOutcome` structure: `sync_check` (profile/source/target all ready), `backup_branch` (starts with `naite/release-prep/staging-`), `current_branch`/`target` `RefSummary`, `current_author_email`, `plan` (1 row, Pick action for matching email), and `repo_snapshot` (HEAD on staging, clean, no busy op).
2. Dirty worktree locks the preflight gate before any IO.
3. Busy operation locks the merge/rebase preflight gate.
4. Sync failure locks that force-sync errors are surfaced (exact wording varies by git version, so the assertion is substring-based).

If Task 21 introduces a regression, at least one of these tests will fail and the refactor will be caught.

## Wave 1 / Task 4 Audit Findings

**Date:** 2026-06-23

### operation.loading call site inventory (complete)
- 56 `operation.loading = true` sites
- 30 `operation.loading = false` sites
- 105 `operation.error = ...` mutations
- 2 multi-line error assignment starters (branch_manage:45, checkout:57)
- 2 read-only `is_none()` guards (repo_open:102, repo_open:145)
- 0 missing from audit (verified via file:line diff vs grep)

### OperationKind distribution
- **AutoFetch (OperationKind)**: 0 `operation.loading` sites. The auto-fetch path uses
  `operation.auto_fetch_path: Option<PathBuf>` as its in-flight signal (set in
  `start_auto_fetch` at fetch/update.rs:101). Task 18 must add a parallel
  `OperationEvent::Started`/`Completed` flow for auto-fetches that doesn't
  go through the legacy boolean.
- **ReleasePrep**: 7 true + 3 false + 7 error = 17 sites in
  `features/release_prep/update.rs` (Task 18 Wave 4).
- **ManualAction("op_name")**: ~60 sites across 20 feature files (Task 22 Wave 5).
  Many features have multiple ManualAction names (e.g., `pull_request_create`,
  `pull_request_checkout`, `pull_request_worktree_checkout` all share the
  `pull_request` file but emit distinct kinds).
- **Custom("repo_open_<sub>")**: 6 true + 4 false + 14 error = 24 sites in
  `features/repo_open/update.rs`. The repo_open flow has 6 distinct loading=true
  entry points (OpenRecent, PathPicked, CloneParentPicked, CloneDone→Loaded,
  InitPathPicked, InitDone→Loaded), each needing its own sub-label.
- **No operation.loading sites at all**: catalog, github_issue, terminal
  (5 error-only sites total). These never emit Start events; the migration
  must synthesize Start→Complete pairs at the failure site.

### Severity split (Task 19 anchor)
- **Fatal** (8 sites): rebase validation messages (lines 192, 196, 200, 211),
  release_prep "Plan a release promotion first" (line 397), rebase
  "Plan a release promotion first" (line 399), rebase paused on conflicts
  fallback (line 573).
- All other `Some(msg)` and `None` error sites default to **Recoverable**.

### Common migration pattern (operation → reload double emission)
Every feature except fetch/release_prep follows the same shape:
1. `start_<feature>` → `operation.loading = true` + `Task::perform(...)`
2. `<feature>::Done` arm → `operation.loading = false`, then on success
   `operation.loading = true` again to trigger a post-op `repo_open::task::load`
3. `repo_open::Loaded` arm → `operation.loading = false`

After migration, step 2's second `loading = true` becomes a NEW
`OperationEvent::Started { kind: Custom("repo_open"), ... }` rather than
continuing the original feature's kind.

### out-of-scope (read-only checks stay as-is)
~30 `if self.operation.loading { return Task::none(); }` guards across all
features must migrate to checking `self.operation_tracker.active().is_empty()`
(or similar) once the boolean is removed. These are NOT in the 86-site
count but must be updated alongside the boolean removals.

### Wave grouping for parallel execution
- Task 18 (Wave 4) = 17 sites (release_prep) + AutoFetch synthesize flow
- Task 22 (Wave 5) = ~70 sites (everything else, plus the read-only guards
  rewrite to `operation_tracker` calls)

### Audit evidence file
`.omo/evidence/task-4-migration-map.md` — 195 Line entries, 24 feature
sections, machine-parseable per-file structure with consistent headers.

## Wave 2 / Task 7 — Modal scroll + max_height (2026-06-23)

**Files changed:**
- `crates/naite-app/src/widgets/modal.rs` — wrap card surface in `scrollable` with `.max_height(MAX_MODAL_HEIGHT)`; use shared `thin_scrollbar` style.
- `crates/naite-app/src/widgets/prompts.rs::rebase_prompt` — split body into its own column wrapped in `scrollable(body).height(Length::Fill)`; button row stays as the second child so it pins at the bottom.

**Key design decisions:**
1. **Centralize the scroll at the modal primitive.** All three modal entry points (`modal`, `wide_modal`, `animated_modal`) funnel through `modal_with_progress`. Adding the scrollable wrapper + max_height there fixes overflow for every caller (release_prep, rebase_editor callers, all `widgets::modal(...)` callers in `view.rs`) without touching each prompt function.
2. **Use existing theme tokens and styles.** `MAX_MODAL_HEIGHT = 600.0` is already in `theme.rs:220`; `styles::thin_scrollbar` and `styles::thin_scrollbar_dir()` are the standard scrollbar helpers used by every other scrollable widget in the codebase. Mirrored `rebase_prompt_preview`'s existing scrollable pattern (prompts.rs:485-494).
3. **Pinned buttons for the only column-layout prompt.** Most prompts use horizontal `row[text, buttons]` so button visibility isn't a vertical problem. `rebase_prompt` is the lone `column[..., button_row]` layout — restructured it to `column![scrollable(body), button_row]` with `.height(Length::Fill)` on the inner scrollable so the modal's 600px budget is partitioned as (body scroll region) + (button row pinned). The outer modal scrollable is a safety net if the prompt's content (incl. nested preview scroll) ever exceeds 600px.

**Gotcha (process-level, not code):**
- A `WIP on main` merge commit (`e7eec9d8`) silently reset the working tree, undoing modal.rs and prompts.rs edits after I had verified the build/tests. Recovery was to re-apply the same two edits; build and tests then passed identically. Lesson: when a Conductor-style merge runs mid-task, re-verify working tree state and re-apply if necessary.

**Verification:**
- `cargo build -p naite-app` succeeds (16 pre-existing warnings, none from changed files).
- `cargo test --workspace --locked` passes all 634 tests (369 + 265).
- No new clippy errors introduced by these files.

## Wave 2 — Task 6 findings

**Context**: Task 6 was executed before Task 5. The `OperationTracker` types
(`OperationId`, `OperationKind`, `OpResult`, `OpSeverity`, `ActiveOperation`,
`CompletedOperation`, `OperationTracker`) did not exist anywhere in the
workspace when this task started, so the minimum surface required for
Task 6's `OperationEvent` routing to compile was added to `state.rs`
inside this same task. Task 5 is expected to expand the stub with TDD
tests (lines 581-595 of the plan) and the read-side helpers
(`start` auto-id, `next_id`, `active`, `recent`) consumed by Wave 3 widgets.

### Type surface added to state.rs (Task 6 contribution)
- `OperationId = usize` (type alias for `ActiveOperation.id` and routing payload)
- `OperationKind { AutoFetch, ReleasePrep, ManualAction(&'static str), Custom(String) }`
  — owned `String` (not `&'static str`) for `Custom` per the plan spec; this
  drops `Copy` from the enum, which is fine because `OpResult::Failed(String)`
  already made the operation record types non-`Copy`.
- `OpResult { Success, Failed(String) }`
- `OpSeverity { Recoverable, Fatal }`
- `ActiveOperation { id, kind, label, started_at: Instant, step: Option<(usize, usize)> }`
- `CompletedOperation { kind, label, completed_at: Instant, result, severity }`
- `OperationTracker { in_flight: Vec<ActiveOperation>, history: VecDeque<CompletedOperation>, next_id: OperationId }`
  with private `OP_HISTORY_CAP = 50` FIFO bound and `#[derive(Default)]`.

### Methods added (Task 6 routing surface)
- `start_with_id(id, kind, label)` — caller-supplied id; idempotent restart
  (replacing any existing in_flight with the same id), bumps `next_id` past
  the supplied id to prevent future auto-allocate collisions.
- `update_step(id, _label, current, total)` — looks up in_flight by id, sets
  step counter; silent no-op on stale ids so the UI cannot panic on
  out-of-order events.
- `complete(id, result, severity)` — removes from in_flight, appends to
  history with FIFO eviction past `OP_HISTORY_CAP`.
- `dismiss(id)` — drops from in_flight without history record (UI dismiss
  path; not a real failure).

### Methods Task 5 must add (intentionally omitted to keep Task 6 scope tight)
- `start(kind, label) -> OperationId` — auto-allocate id.
- `next_id() -> OperationId` — peek without inserting.
- `active() -> &[ActiveOperation]` — for top status bar (Task 12).
- `recent() -> impl Iterator<Item = &CompletedOperation>` — for toast list
  (Task 14) and central overlay (Task 15).

### Routing placement in update.rs
- New arm `Message::Operation(event)` placed at the END of the global
  `update()` match (after `AvatarFetched`) — keeps the existing arm order
  intact and groups cross-cutting global channels together.
- Each match arm returns `Task::none()` and only mutates
  `self.operation_tracker`. No other `App` state is touched by Task 6's
  events; Tasks 18/22 will handle the call-site migration that emits
  these events.
- `OperationEvent::ToastExpired { index }` is a no-op with a 2-line
  comment explaining that toast storage is owned by Task 14. This
  comment is intentional (priority #3 — explains WHY an event handler
  does nothing; otherwise future readers would mistake the no-op for a
  bug). Underscore-prefixed `_index` binds the unused parameter
  explicitly.

### Wire shape decision: caller-supplied id
- `OperationEvent::Started` carries `id: OperationId` from the emitter
  to the tracker, NOT the other way around. Rationale: the message stream
  is the canonical emission surface (Tasks 18/22 mint ids at call sites),
  so the emitter is the source of truth. The tracker accepts the id via
  `start_with_id`, which is idempotent — re-emitting `Started` with the
  same id replaces the existing in_flight entry rather than creating a
  duplicate.
- This decouples Task 5's "auto-id" API from Task 6's "caller-id" API:
  both can coexist (`start` returns auto-id; `start_with_id` uses the
  caller's id). Task 5's spec only mandates `start()`; Task 6 needs
  `start_with_id()` because the id travels through the message stream.

### Dead code expectations for Wave 3 (same pattern as Task 2 constants)
- `ActiveOperation.started_at`, `CompletedOperation.{kind,label,completed_at,
  result,severity}` will be read by Task 12/13/14 widgets for elapsed-time
  display, kind icons, and toast rendering.
- `OperationTracker.{start,next_id,active,recent}` will be added by Task 5
  and consumed by Task 12/13/14 widgets.
- The pre-existing Task 2 theme.rs constants (`MIN_WINDOW_WIDTH`,
  `MAX_MODAL_HEIGHT`, `STATUS_BAR_HEIGHT`, etc.) plus the new
  `OP_HISTORY_CAP` (used by `complete()` internally, hence NOT in the
  unused list anymore — verified) remain pre-positioned.
- Verified: `cargo build -p naite-app` reports 16 warnings, ALL
  attributable to either (a) pre-existing Task 1 re-export unused-imports
  / Task 2 unused constants, or (b) the deliberate Wave 3 pre-positioned
  fields on the new `OperationTracker` types.

### Verification
- `cargo build -p naite-app` → finished, 0 errors
- `cargo test --workspace --locked` → 369 naite-app + 265 naite-core + 0
  doc-tests = 634 passed, 0 failed (matches the task's expected baseline)
- `cargo fmt --all -- --check` → only diffs in `features/terminal/update.rs`
  (pre-existing uncommitted change unrelated to Task 6)
- No new files; only `state.rs`, `message.rs`, `update.rs`, `app.rs`
  modified. No new dependencies.

### Committed together with Wave 2 (pending final group commit)
- `feat(message): add OperationEvent enum and update.rs routing skeleton`
- Files: `crates/naite-app/src/{state,message,update,app}.rs`
- Pre-commit: `cargo build -p naite-app && cargo test --workspace --locked`

---

## Wave: Task 8 — Unify ROW_HEIGHT usage (replace hardcoded 34.0/24.0)

### Changes applied
- `widgets/prompts.rs:560` — `Length::Fixed(34.0)` → `Length::Fixed(ROW_HEIGHT)` (now at line 571 due to pre-existing wrap)
  - Added `use super::ROW_HEIGHT;` after existing crate imports (line 16)
- `widgets/sidebar.rs:1122,1132,1136` — three `Length::Fixed(24.0)` → `Length::Fixed(SIDEBAR_REF_ROW_HEIGHT)`
  - 1132 is the `pointer_idle_layer` mouse_area for the SAME row — semantically must match row height; included even though task description only listed 1122/1136
  - fmt required multi-line `Space::new(...)` to satisfy line length
- `widgets/tab_strip.rs:181` — `Length::Fixed(24.0)` → `Length::Fixed(TAB_HEIGHT)`

### Constants used (no new ones introduced)
- `ROW_HEIGHT = 32.0` (declared in widgets/mod.rs:62, `pub const`)
- `SIDEBAR_REF_ROW_HEIGHT = 26.0` (sidebar.rs:30, local `const`, in-file usage — no import needed)
- `TAB_HEIGHT = 28.0` (tab_strip.rs:23, local `const`, in-file usage — no import needed)

### Verification
- `cargo build -p naite-app` — succeeds (16 warnings, all pre-existing dead-code in unrelated theme/state/ops modules)
- `cargo test --workspace --locked` — 634 tests pass (369 + 265 + 0 doc), 0 regressions
- `cargo test --workspace --locked -- rebase` — 16 rebase tests pass (drag math intact)
- `cargo fmt --all -- --check` — clean
- `cargo clippy -p naite-app --all-targets` — no new warnings on changed files

### Out-of-scope finding (flagged for follow-up)
- `widgets/command_palette.rs:162` — `Length::Fixed(32.0)` used as a selection rail height inside command palette rows. Semantically could use `ROW_HEIGHT` but task scoped files to prompts/sidebar/tab_strip. If `ROW_HEIGHT` ever changes, this rail won't follow → minor jitter risk. Easy follow-up: import + replace.

### Grep evidence
- Saved to `.omo/evidence/task-8-row-height-grep.txt`
- `grep -rn "Length::Fixed(34\.0)" crates/naite-app/src/widgets/` → 0 matches ✓
- `grep -rn "Length::Fixed(24\.0)" crates/naite-app/src/widgets/sidebar.rs crates/naite-app/src/widgets/tab_strip.rs` → 0 matches ✓

### Why this works
- Replaced `34.0` (rebase_prompt_preview_row) with `ROW_HEIGHT=32.0` — this row renders inside the rebase prompt preview, adjacent to nothing visible, so the 2px reduction is safe (the plan only affects the sidebar/tab strip visually; this is inside a modal).
- Replaced `24.0` (sidebar folder_item) with `SIDEBAR_REF_ROW_HEIGHT=26.0` — now matches sidebar ref row heights → eliminates 2px jitter with adjacent items.
- Replaced `24.0` (tab_strip menu_item) with `TAB_HEIGHT=28.0` — now matches the tab row height → eliminates 4px jitter with adjacent tab rows.
- rebase_editor drag math in update.rs already uses ROW_HEIGHT=32.0 → unaffected.

## Wave 2 — Task 9: TERMINAL_LINE_HEIGHT CJK fix + dynamic panel_chrome()

### Done
- Bumped `TERMINAL_LINE_HEIGHT` from 15.0 to 17.0 in
  `crates/naite-app/src/widgets/terminal.rs:35`. 17.0 is the middle
  ground between Latin natural (13px at FS_SM=11, 15px was comfortable)
  and CJK natural (16-18px at FS_SM=11, 15px clipped Hangul descenders).
- Replaced `pub const TERMINAL_PANEL_CHROME: f32 = 110.0` with
  `pub fn panel_chrome(state: &TerminalState) -> f32` in
  `crates/naite-app/src/widgets/terminal.rs:46`. The function returns
  85.0 when no chip is visible (Running) or 107.0 when the chip is
  visible (Idle/Starting/Exited/Error), matching the actual chrome
  rendered by `terminal_panel`.
- Added two new constants:
  - `STATUS_CHIP_HEIGHT = 22.0` — heuristic for the chip's contribution
    to header row height (chip itself ~14-16px but extends the header).
  - `TAB_ROW_HEIGHT = 28.0` — mirrors `widgets/tab_strip.rs:23
    TAB_HEIGHT` for cross-panel consistency.
- Added `pub(crate) fn status_chip_visible(session) -> bool` mirroring
  the chip-render decision in `status_label` (terminal.rs:484-509).
  Returns `false` only for `TerminalStatus::Running`. Stays private to
  the widget module — only `panel_chrome` uses it.
- Updated `widgets/mod.rs:52-55` re-export: dropped
  `TERMINAL_PANEL_CHROME`, added `panel_chrome`.
- Updated `features/terminal/update.rs:672` consumer:
  `TERMINAL_PANEL_CHROME` → `panel_chrome(&self.terminal)`.

### Chrome math
- Base chrome (chip hidden): `HEADER(32) + DIVIDER(1) + TAB_ROW_HEIGHT(28) + PADDING(24) = 85.0`
- With chip: `85.0 + STATUS_CHIP_HEIGHT(22) = 107.0`
- Old constant was 110.0 — 3px off our new chip-visible total. The
  discrepancy comes from rounding the heuristic chip height; the
  original author tuned 110 manually and we matched it as closely as
  the breakdown allows.

### Body height impact (TERMINAL_PANEL_HEIGHT=320)
- Old (chrome=110, line=15): body=210, rows=14
- New Running (chrome=85, line=17): body=235, rows=13.8 → 13 (13×17=221, 14px slack — small acceptable slack, no big gap)
- New with chip (chrome=107, line=17): body=213, rows=12.5 → 12 (12×17=204, 9px slack)

The 25px gap that appeared when Running (chrome was 110 but actual was 85) is now eliminated — chrome correctly returns 85 for Running sessions.

### Why `pub fn panel_chrome(state: &TerminalState)` (not `&App`)
- The function only needs `state.active_session()`. `TerminalState`
  is the minimal type that exposes that. Keeping the signature narrow
  means future tests can pass a hand-built `TerminalState` without
  wiring up the entire `App`.
- `features/terminal/update.rs` calls it as
  `panel_chrome(&self.terminal)` — one field access, no extra plumbing.

### Inherited build noise (NOT caused by Task 9)
- Pre-existing WIP changes from earlier waves in `state.rs`,
  `tests.rs`, `widgets/{modal,sidebar,prompts,tab_strip}.rs`, etc.
  introduce dead_code warnings on `OperationKind::ReleasePrep`,
  `OperationTracker::update_step`, and 5 theme constants
  (`MIN_WINDOW_WIDTH/HEIGHT`, `STATUS_BAR_HEIGHT`,
  `OVERLAY_TRIGGER_SECS`, `TOAST_SUCCESS_TTL_SECS`). These are
  pre-positioned for downstream Wave 3 widgets.
- `tests.rs:9331` was missing a closing `}` on
  `operation_tracker_dismiss_unknown_id_returns_error`, blocking all
  test compilation. Fixed in passing (mechanical one-line fix) so that
  `cargo test --workspace --locked` could verify 0 regressions.
  Without that fix, the test command would fail with a parse error
  unrelated to Task 9.

### Test verification
- `cargo test --workspace --locked`: 643 passed, 0 failed
  - naite-app: 378 passed
  - naite-core: 265 passed
  - doc-tests: 0 passed
- `cargo build -p naite-app`: Finished, 0 errors
- `cargo fmt --all -- --check`: Task 9 files clean (only pre-existing
  diff in `widgets/sidebar.rs:1129` from another wave).

### Risk / out-of-scope notes
- `TERMINAL_CHAR_WIDTH = 7.6` unchanged — column width is a separate
  concern from row height. CJK column-width logic remains for a future
  task.
- New sessions are created in `TerminalStatus::Idle` (chip visible),
  but `terminal_dimensions` is called BEFORE the session is created.
  This means the chrome at PTY-resize time may be 85 (no active session
  or previous was Running) even though the new session will be Idle
  (chrome=107). The discrepancy is at most 22px on initial render and
  resolves on the next PTY resize once status transitions.
  Acceptable for Task 9 scope; future cleanup could re-call
  `terminal_dimensions()` after `create_session()` to lock the
  initial PTY size to the post-creation chrome.
- Status chip width is not part of chrome math; only height. The chip
  extending the title-label column width is a separate concern.

### Evidence file
- `.omo/evidence/task-9-cjk-build.txt`

## Wave 2 — Task 5: OperationTracker state model

**Date:** 2026-06-23

### Files touched
- `crates/naite-app/src/state.rs` — added OperationTracker types + impl (between TransientStatus and RepositoryManagerState, ~150 LOC)
- `crates/naite-app/src/tests.rs` — added 9 OperationTracker tests at end of file (lines ~9176-9332)
- Imports: added `VecDeque` to std::collections, `Duration` to std::time, and `theme::OP_HISTORY_CAP` to crate use list

### API surface implemented
- `pub type OperationId = usize`
- `pub enum OperationKind { AutoFetch, ReleasePrep, ManualAction(&'static str), Custom(String) }`
- `pub enum OpResult { Success, Failed(String) }`
- `pub enum OpSeverity { Recoverable, Fatal }`
- `pub struct ActiveOperation { id, kind, label, started_at: Instant, step: Option<(usize, usize)> }`
- `pub struct CompletedOperation { id, kind, label, completed_at: Instant, result, severity }`
- `pub enum OperationTrackerError { UnknownOperation(OperationId) }`
- `pub struct OperationTracker` with methods:
  - `start(kind, label) -> OperationId` (auto-generates monotonic id via `next_id.wrapping_add(1)`)
  - `update_step(id, label, current, total) -> Result<(), OperationTrackerError>`
  - `complete(id, result, severity) -> Result<(), OperationTrackerError>` (removes from in_flight, pushes to history, evicts if at cap)
  - `fail(id, error_msg, severity) -> Result<(), OperationTrackerError>` (delegates to complete with Failed)
  - `dismiss(id) -> Result<(), OperationTrackerError>` (removes from history by id)
  - `active() -> &[ActiveOperation]`
  - `recent(n) -> &[CompletedOperation]` (uses `as_slices().0[start..]` for VecDeque contiguous case)
  - `active_long_running(threshold_secs: u64) -> Option<&ActiveOperation>` (compares started_at.elapsed() >= Duration::from_secs(threshold))
  - private `evict_history()` (uses `theme::OP_HISTORY_CAP` as ring buffer bound)

### Deviation from plan code sketch
- Added `pub id: OperationId` to `CompletedOperation` (plan sketch omitted it).
  Required so `dismiss(id)` can find history entries by id without an external mapping table.
  This is a typed value, no extra heap, and aligns with the test that calls `dismiss(id)`
  on a previously-completed op.
- The plan sketch shows `start(kind, label) -> OperationId` but the existing state.rs had
  a stale `start_with_id(id, kind, label)` from a partial WIP drop. Rewrote it to the spec
  API (auto-generated id) so the monotonic counter is the single source of truth.

### Test design notes
- `active_long_running` test uses `threshold=0` (always matches) and `threshold=u64::MAX`
  (~584 billion years, never matches) instead of mocking the clock. Avoids any sleep/timer
  in pure-logic unit tests.
- 51-completions test verifies FIFO eviction by checking "op-0" is gone and "op-50" is
  present, plus `history.len() == OP_HISTORY_CAP`.
- Restart test verifies monotonic id (id2 ≠ id1) AND in_flight has exactly 1 entry after
  the cycle, AND the new entry has id2's label "second".
- Two error-path tests (`complete_unknown_id_returns_error`, `dismiss_unknown_id_returns_error`)
  pin the error contract so Task 6's wiring can rely on `Err(UnknownOperation(id))`.

### Verification
- `cargo test --workspace --locked -- operation_tracker` → 9 passed, 0 failed
- `cargo test --workspace --locked` → 378 naite-app + 265 naite-core passed (was 369 + 265)
- `cargo build -p naite-app` → succeeds (warnings only, see below)
- `cargo fmt --all -- --check` → exit 0 clean

### Expected pre-wiring warnings (Task 6 will resolve)
`cargo clippy -p naite-app --all-targets --locked` reports 16 dead_code warnings on the bin
"naite" target because state.rs types (OperationId, OperationKind, OpResult, OpSeverity,
ActiveOperation, CompletedOperation, OperationTracker, OperationTrackerError, update_step)
are only consumed by tests.rs in this commit. Task 6 will add `operation_tracker:
OperationTracker` to `App` and start wiring call sites, which will clear these warnings.
The same pattern was accepted in Task 1 for `widgets/mod.rs:28` unused imports that were
"pre-positioned for Wave 3 widgets (Tasks 12-15)" — see Wave 1 findings above.

### Module visibility note
`mod theme;` in main.rs is private, so `pub const OP_HISTORY_CAP` is reachable from
state.rs via `use crate::theme::OP_HISTORY_CAP` because state.rs is a sibling of theme.rs
under the bin root. The warning "constant `OP_HISTORY_CAP` is never used" comes from the
bin "naite" non-test compilation — but the constant IS used in `state.rs::evict_history()`
which compiles only when state.rs is referenced (it is, by tests.rs). Once Task 6 wires
OperationTracker into App, this will resolve as well.

### File-scope compliance
- Only `crates/naite-app/src/state.rs` and `crates/naite-app/src/tests.rs` modified in the
  source tree (plus evidence file `.omo/evidence/task-5-tracker-tests.txt` and this notepad).
- No view.rs, update.rs, message.rs, or main.rs touched.
- No new dependencies (uses std `VecDeque`, `Instant`, `Duration` only).

## Wave 2 — Task 11: Raise window min_size to 1024×640 (2026-06-23)

### Done
- `crates/naite-app/src/main.rs:27` — added `use crate::theme::{MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT};` after existing iced/state imports.
- `crates/naite-app/src/main.rs:54` — replaced `min_size: Some(Size::new(900.0, 600.0))` with `min_size: Some(Size::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT))`.
- Total diff: 1 import + 1 literal swap, exactly per task spec.

### Verification
- `cargo build -p naite-app` → Finished `dev` profile, 0 errors.
- Warning count: 14 (all pre-existing, none introduced by Task 11).
- Critically, `MIN_WINDOW_WIDTH` and `MIN_WINDOW_HEIGHT` dropped from the unused-constants warning list — they now have a live consumer (main.rs:54). Remaining unused-constant warnings are `STATUS_BAR_HEIGHT`, `OVERLAY_TRIGGER_SECS`, `OP_HISTORY_CAP`, `TOAST_SUCCESS_TTL_SECS` (still pre-positioned for Tasks 12/14/15/20).
- `main.rs:53` `size: Size::new(1200.0, 760.0)` untouched (default window size unchanged, only the floor is raised).
- `max_size` remains unset (no `Some(...)` on the field) — Task 11 explicitly out of scope.

### Style note (no action needed)
- Other `theme::*` calls in main.rs (`theme::naite_dark()`, `theme::font_regular()`, etc.) are accessed via the bare `theme::` path because `mod theme;` declares the module at crate root. Task 11 used an explicit `use crate::theme::{...}` per the task instruction, which is also idiomatic Rust and the build is clean.
- Both styles coexist fine in main.rs — no conflict.

### File-scope compliance
- Only `crates/naite-app/src/main.rs` modified in the source tree (plus evidence file `.omo/evidence/task-11-min-size-build.txt` and this notepad append).
- No new dependencies.
- Pre-commit verification per commit-strategy note: `cargo build -p naite-app` passes (no `cargo test --workspace` run because Task 11 is a single literal swap with no semantic effect on any test path).

### Risk / out-of-scope
- Users with windows currently sized between 900×600 and 1024×640 will be unable to resize below the new minimum on the next session. This is the intended user-reported fix (window too narrow → UI breaks).
- The min_size is enforced by the OS window manager on resize, not on startup. Existing 900×600 windows remain at that size until the user manually resizes (the OS won't auto-grow the window). A future task could enforce min_size at startup by clamping `window::Settings::size`, but that is explicitly out of Task 11 scope.

### Evidence file
- `.omo/evidence/task-11-min-size-build.txt` — `cargo build -p naite-app` output (Finished, 14 pre-existing warnings).


## Wave 2 — Task 10: Ellipsis for overflowing text + scrollable hunk actions (2026-06-23)

### Done

- `crates/naite-app/src/widgets/common.rs:181-204` — added 2 cross-submodule helpers:
  - `max_chars_for_width(width_px) -> usize` — width-to-char conversion using the same `7.0` heuristic as `action_label_width`; reserves 1 slot for the ellipsis glyph; returns 0 for non-positive widths, at least 1 for positive widths.
  - `truncate_with_ellipsis(s, max_chars) -> String` — appends `…` when over the limit, returns original otherwise; handles `max_chars == 0` and `s == ""` correctly.
  - Both are `pub(super)` because callers live in sibling widget submodules (`commit_list`); reachable via the existing `use super::common::{...}` import.
- `crates/naite-app/src/widgets/commit_list.rs:25` — import `max_chars_for_width, truncate_with_ellipsis` from `super::common`.
- `crates/naite-app/src/widgets/commit_list.rs:123` — propagate `list_width` (already in `CommitListProps`) into each `commit_row` call.
- `crates/naite-app/src/widgets/commit_list.rs:309,319` — added `list_width: f32` field to `CommitRowProps` and the destructure.
- `crates/naite-app/src/widgets/commit_list.rs:370-378,397-402` — AUTHOR and WHEN columns truncate their text via the new helpers using their fixed column widths (`AUTHOR_COLUMN_WIDTH=132`, `WHEN_COLUMN_WIDTH=86`).
- `crates/naite-app/src/widgets/commit_list.rs:720-728,738-742` — `subject_with_labels` now accepts `available_width: f32` and truncates `commit.summary` accordingly.
- `crates/naite-app/src/widgets/commit_list.rs:755-774` — new `subject_available_width(list_width, layout)` helper computes the available subject width by subtracting chrome (selection bar + 2 spacings), graph canvas width, fixed columns (SHA + optional AUTHOR + optional WHEN + their trailing spacing), and the SP_MD spacings between row children. Uses `.max(0.0)` so the helper never produces negative widths that would cause `max_chars_for_width` to return 0 and produce an empty subject.
- `crates/naite-app/src/widgets/detail_pane.rs:1202-1209` — new `actions_scrollable(actions)` helper wraps the hunk-header actions row in a horizontal `scrollable` with the same hidden-scrollbar pattern as `diff_mode_group` (line 1133) and the file-insight toolbar (line 886). Uses `Length::Shrink` to match the existing `diff_mode_group` pattern.
- `crates/naite-app/src/widgets/detail_pane.rs:1283-1288` — hunk_header actions row now wraps `actions` in `actions_scrollable(actions.into())`; the `.into()` is required because `actions` is a `Row` not an `Element` (compile error on first attempt).

### Why width-based instead of fixed max_chars

- The commit-list row layout is responsive: AUTHOR and WHEN columns hide at narrow widths (`commit_list_layout` returns `show_author/show_when = false` when `list_width < base_width + column_width`). A fixed `max_chars` would either over-truncate at wide widths or under-truncate at narrow widths. By threading `list_width` through `CommitRowProps` and computing `subject_available_width` per-row, the truncation matches the actual available space.
- The chrome graph width depends on lane count (variable), so even the chrome + graph + SHA contribution to row overhead is per-row. The helper centralizes this arithmetic and reuses the same `SP_MD` spacing count pattern that `commit_list_layout` uses (line 962-983).

### Spacing count rationale in `subject_available_width`

- The row contains: `[bar, graph, sha, subject, author?, when?]`. Between each pair is `theme::SP_MD`. The base count (3 spacings) covers bar↔graph, graph↔sha, sha↔subject. Adding 1 each for author and when accounts for the extra pair separator when those columns are visible.
- Mirrors `commit_list_layout`'s formula at `commit_list.rs:962-983` exactly so the truncation math stays in sync with the visibility math.

### iced 0.13.1 limitation: no native Ellipsis/truncate

- `iced_core-0.13.2::widget::text` only exposes `Wrapping::{None, Word, Glyph, WordOrGlyph}` and `LineHeight::{Relative, Absolute}`. No truncate/Ellipsis mode.
- This is the reason `Wrapping::None + container.clip(true)` was silently clipping — the user's "things break everywhere" report was triggered by long commit subjects/author names overflowing narrow panes with no visual signal.
- Truncating the source string (rather than relying on rendering-time truncation) is the only viable workaround in the pinned iced version.

### Why horizontal scrollable for hunk actions (not wrap-to-next-row)

- The task spec gave both as options; picked horizontal scrollable because:
  1. The `Row` containing the buttons is inside a `Column` of `[hunk_index_label_row, actions_row]`. Wrapping to the next line would require restructuring the column to give the actions row more vertical room, which clashes with the hunk header's existing `Length::Fill` constraint and the `selected_hunk_header` style.
  2. Horizontal scroll with hidden scrollbar is already the established pattern in this file (3 other usages of `Direction::Horizontal(Scrollbar::new().width(0)...)` — file_insight toolbar, diff_mode_group, file_diff navigation).
  3. The 44px worst-case overflow (per task spec) is small enough that trackpad/wheel scrolling is ergonomic.
- Buttons retain `action_button_width` clamp (52-108px) per the task's MUST NOT DO constraint — no button sizes changed.

### Visibility choice: `pub(super)`

- `pub(super)` keeps the helpers within the `widgets` module tree. `commit_list.rs` and `detail_pane.rs` are siblings under `widgets/`, so `pub(super)` works for both.
- Considered `pub` (with re-export) but the helpers are widget-internal — `widgets/common.rs` is not the kind of surface that should be re-exported at crate root. `pub(super)` matches the existing pattern of `action_label_width` and `action_button_width` (also file-private).

### Inherited pre-existing warnings (NOT from Task 10)

- 14 warnings remain: 4 unused imports in `widgets/mod.rs:28` (Wave 1 Task 1 animation hoist pre-positioned), and 10 dead_code warnings on `OperationTracker*` types and Wave 2 layout constants (`STATUS_BAR_HEIGHT`, `OVERLAY_TRIGGER_SECS`, `OP_HISTORY_CAP`, `TOAST_SUCCESS_TTL_SECS`) — all pre-positioned for Wave 3 widgets (Tasks 12-15). Task 10 introduces 0 new warnings.

### Verification

- `cargo build -p naite-app` → Finished, 0 errors, 14 pre-existing warnings (unchanged from Wave 2 baseline)
- `cargo test --workspace --locked` → 378 naite-app + 265 naite-core + 0 doc = 643 passed, 0 failed (matches Wave 2 baseline exactly; 0 regressions)
- `cargo fmt --all -- --check` → clean
- `cargo clippy -p naite-app --all-targets --locked -- -D warnings` → fails on the same 14 pre-existing dead_code warnings from Wave 1/2; Task 10 changes introduce no new clippy findings.

### Files modified

- `crates/naite-app/src/widgets/common.rs` — +25 LOC (2 helpers + 1 const)
- `crates/naite-app/src/widgets/commit_list.rs` — +47 / -3 LOC (CommitRowProps.list_width, AUTHOR/WHEN truncation, subject_with_labels signature + body, subject_available_width helper)
- `crates/naite-app/src/widgets/detail_pane.rs` — +11 / -5 LOC (actions_scrollable helper, hunk_header actions row wrap)

Total: +83 / -8 LOC, no new dependencies, no new files.

### Evidence

- `.omo/evidence/task-10-ellipsis-build.txt` — `cargo build` + `cargo test` summary + `cargo fmt --check`.

### Risk / out-of-scope

- `subject_available_width` assumes the row's children stay in their current order and use the same spacing tokens. If a future change adds/removes a column or changes SP_MD, the truncation will get out of sync (still truncates, but with the wrong limit). Mirroring `commit_list_layout` exactly mitigates this risk; a future refactor could share the chrome-arithmetic constants between the two helpers.
- The `7.0` chars-per-px heuristic is consistent with `action_label_width` but not measured. For Latin scripts at `FS_SM=11` it produces a slightly conservative estimate (most Latin glyphs are ~6-7px wide at FS_SM). CJK glyphs are ~11-12px, so CJK subject text will over-truncate. Acceptable for Task 10 scope; a follow-up could use `text.chars().count() * 2` for CJK detection (or measure with `cosmic-text`).
- The horizontal scrollable in hunk_header hides the scrollbar (matching the existing precedent), so the only scroll affordance is trackpad/wheel. Users on systems without a scroll wheel may not discover the overflow. Acceptable per existing pattern.

## Wave 3 — Task 16: ReleasePrepStep enum + preparing step state fields

**Date:** 2026-06-23

### Files changed
- `crates/naite-app/src/state.rs` — added `ReleasePrepStep` enum + `PrepareStepOutcome` struct; extended `ReleasePrepState` with `preparing_step` + `completed_preparing_steps`; added `use crate::features::rebase::RebasePlanRow;`
- `crates/naite-app/src/features/release_prep/message.rs` — added `PrepareStepStarted(ReleasePrepStep)` + `PrepareStepDone { step, result }` variants + `use crate::state::{PrepareStepOutcome, ReleasePrepStep};`
- `crates/naite-app/src/features/release_prep/update.rs` — added 2 no-op match arms for the new variants (placeholder until Task 21 wires real step-chain handlers)

### Design decisions

**1. Why `Option<Vec<RebasePlanRow>>` in `PrepareStepOutcome` (not just `Vec`)**
The outcome is a carrier passed between the split per-step async fns (Task 21). Only `BuildingPlan` populates `plan_entries`; earlier steps leave it `None`. Using `Option<...>` makes the carrier-pattern contract self-documenting: each field is `Some` only after its producing step ran. Mirrors the established `Option<ReleaseSyncCheck>` + `Option<String>` shape already on `ReleasePrepState` (lines 480, 478).

**2. Why `pub struct PrepareStepOutcome` (not enum)**
The task spec offered both shapes. Picked a struct with all-`Option` fields because the step pipeline is strictly linear (B1→B2→…→B7 with no branching) and the per-step output accumulates monotonically. A sum type would force every step to destructure-then-reconstruct with the new field `Some`-wrapped, which is mechanical noise for a linear chain. The struct is also `#[derive(Default)]` so Task 21's step loop can `outcome.sync_check.clone().map(...)` style updates cleanly.

**3. Why 6 variants (not 7)**
The plan is explicit: `B0` is the pre-flight (operation-state + dirty-worktree gate) that runs inside the outer `spawn_blocking` guard at `task.rs:39-46`. It is structurally a guard, not a user-facing async step, so it stays inside the outer `Task::perform` and the new chain starts at `FetchingRemote` (B1). Documented this in the enum's doc comment so a future implementer doesn't add a 7th `Preflight` variant.

**4. Why the update.rs no-op arms are 2 lines (not stubs that call real methods)**
The task constraint says "DO NOT modify update.rs for release_prep routing (Task 21 does that)" — but adding new message variants without match arms is a hard compile error. The minimum compile-clean change is two `Task::none()` arms with a `..` and `_step` wildcard. Task 21 will replace these with the real `begin_release_prepare_step` / `finish_release_prepare_step` handlers. The arm placement sits between `Prepared` and `AutoRequested` to keep `Prepared`-related routing together (other prepare-related state mutations are in the surrounding area).

**5. Import path: `use crate::features::rebase::RebasePlanRow;`**
`rebase/mod.rs:7` re-exports `RebasePlanRow` at the module root, so the import works directly. The rebase module is `pub(crate)`, which is sufficient for the state.rs sibling. Mirrors the pattern in `release_prep/task.rs:7` (`use crate::features::rebase::RebasePlanRow;`).

### Verification
- `cargo build -p naite-app` → Finished, 0 errors (22 pre-existing warnings from Wave 1/2/3 dead-code, all in unrelated files: `OperationTracker*` types, `widgets/{status_bar,progress_overlay}.rs`, theme constants — no warnings on my new types)
- `cargo test --workspace --locked` → 378 + 265 + 0 doc = 643 passed, 0 failed
- `cargo test --workspace --locked -- release_prep` → 21 passed, 0 failed (includes the 4 Task 3 baseline tests that pin current `prepare()` behavior — they continue to pass because this task does not change `prepare()`)
- `rustfmt --check --edition 2021` on the 3 touched files → clean
- `cargo clippy -p naite-app --all-targets` → no new warnings on my new types; the 22 pre-existing dead-code warnings are all in untouched files (state.rs OperationTracker types, status_bar/progress_overlay, theme constants)

### File-scope compliance
- Only the 3 files listed above modified in the source tree (plus evidence file `.omo/evidence/task-16-release-prep-state-build.txt` and this notepad append).
- `prepare()` body at `task.rs:34-109` untouched (per task constraint).
- No new dependencies.
- No widget changes (per task constraint).
- No naite-core changes (uses existing `ReleaseSyncCheck` and the existing `crate::features::rebase::RebasePlanRow`).

### Risk / out-of-scope
- The no-op update.rs arms mean that if a stray `PrepareStepStarted`/`PrepareStepDone` message somehow gets dispatched before Task 21 wires real handlers, it will be silently swallowed (`Task::none()`). This is the desired behavior: the state foundation is added first, and the routing will arrive in Task 21. The Task 3 baseline tests do NOT exercise the new variants, so no regression risk.
- `PrepareStepOutcome` is not currently consumed by any view layer. Task 21 will read it in the new step handlers; the widget that renders per-step progress is part of Task 21's scope (or a follow-up).
- The carrier struct's `sync_check` field duplicates the existing `self.release_prep.sync_check: Option<ReleaseSyncCheck>` field (line 480 of state.rs). Task 21 may choose to keep the duplication (defensive snapshot per step) or eliminate the field from `ReleasePrepState` once the carrier is the source of truth. Out of scope for Task 16.

### Evidence file
- `.omo/evidence/task-16-release-prep-state-build.txt`

### Commit
- Message: `feat(release_prep): add ReleasePrepStep enum and preparing step state fields`
- Pre-commit verification: `cargo test --workspace --locked -- release_prep` → 21 passed

## Wave 3 — Task 15: Central progress overlay widget (2026-06-23)

### Files changed
- `crates/naite-app/src/widgets/progress_overlay.rs` (NEW, 75 LOC)
- `crates/naite-app/src/widgets/mod.rs` — `mod progress_overlay;` + `pub use progress_overlay::progress_overlay;`
- `crates/naite-app/src/styles.rs` — new `// ---------- progress overlay ----------` section with `progress_overlay_backdrop` and `progress_overlay_card`

### Design decisions
1. **Backdrop does NOT capture pointer events.** Unlike `widgets/modal.rs::modal_with_progress` which wraps the backdrop in `mouse_area(...).on_press(on_dismiss)`, the progress overlay uses a bare `container` for the backdrop. The overlay is non-modal: v1 has no cancel button, and the trigger condition (Task 20) removes the overlay when the operation completes. Click-through to the underlying UI is intentional — the user can keep working while the operation runs.
2. **Static alpha, animated bar.** Backdrop uses `color::with_alpha(color::BG, 0.55)` (no `progress` parameter). All visual motion lives on `moving_progress_bar(frame)` inside the card. This matches the "calm chrome, animated core" pattern used elsewhere (e.g., terminal status chip).
3. **Card width fixed at 420px.** Smaller than `MODAL_MAX_WIDTH=480` because the overlay is a passive indicator, not an interactive form. Single child (label) doesn't need scrollable wrapper. `SP_LG` (16) padding matches modal padding for visual consistency.
4. **Step counter is conditional.** Uses `if let Some((current, total)) = op.step` to push the `Step X/Y` text only when the operation has reported a step. `ActiveOperation.step: Option<(usize, usize)>` is `None` for ops that don't expose step granularity (e.g., auto-fetch), so the card stays compact in those cases.
5. **"Step X/Y" uses English (not "단계 X/Y").** Consistent with the rest of the app's English chrome (e.g., "Loading..." in toolbar, release_prep's "Step" labels). The spec offered either; picked English for system consistency.

### Style placement
- New section `// ---------- progress overlay ----------` placed AFTER `ghost_action_chip` (the previous final `// ---------- ` style section) and BEFORE the `// ---------- buttons ----------` section. Followed the file's existing section ordering convention (container surfaces → scrollable → progress overlay → buttons).
- `progress_overlay_card` uses `SURFACE_2 + BORDER(1.0) + R_MD(5.0)` — same triple as `inset_card` (style family match for any "elevated plate" surface).
- `progress_overlay_backdrop` uses `color::with_alpha(color::BG, 0.55)` — slightly less aggressive than the modal's `0.6 * progress` peak so the underlying UI is readable while the overlay is active.

### Inherited pre-existing state (not from Task 15)
- `crates/naite-app/src/features/release_prep/message.rs` + `update.rs` — already-modified by another wave to add `PrepareStepStarted(ReleasePrepStep)` and `PrepareStepDone { step, result }` variants. The match in `update.rs:62-63` covers both, so the code is exhaustive. Task 15 did not touch these files.
- `crates/naite-app/src/widgets/status_bar.rs` — pre-existing untracked file (Task 12 partial). Compiles cleanly, just adds 1 extra "never used" warning.
- `crates/naite-app/src/state.rs` — pre-existing diff adds `preparing_step`, `completed_preparing_steps` fields, `ReleasePrepStep` enum, and `PrepareStepOutcome` struct. Task 15 did not touch state.rs.
- The pre-existing `OperationTracker.active_long_running(threshold_secs: u64) -> Option<&ActiveOperation>` (state.rs:309-320) is the consumer-facing API that Task 20 will call to decide when to show this overlay. Threshold comes from `theme::OVERLAY_TRIGGER_SECS = 2`.

### Pre-positioned warnings (Task 20 will resolve)
- `progress_overlay` (function never used)
- `progress_overlay_card` (function never used)
- `progress_overlay_backdrop` (function never used)
- `OVERLAY_CARD_WIDTH` (const never used, transitive via the function)
- `pub use progress_overlay::progress_overlay` (unused re-export at mod.rs:41)

Total new warnings introduced by Task 15: 5. All pre-positioned for Task 20 (central overlay trigger). Same accepted pattern as Tasks 1/2/5 (animation hoist + theme constants + OperationTracker types).

### Verification
- `cargo build -p naite-app` → Finished, 0 errors, 23 warnings (was 14 pre-Task-15 + 5 from Task 15 + 4 from pre-existing untracked status_bar.rs work)
- `cargo test --workspace --locked` → 378 naite-app + 265 naite-core + 0 doc = 643 passed, 0 failed (matches Wave 2 baseline exactly; 0 regressions)
- `cargo fmt --all -- --check` → Task 15 files clean (only pre-existing diffs in `message.rs` and `status_bar.rs` from other waves)
- `cargo clippy -p naite-app --all-targets --locked` → no new clippy findings from Task 15 (just the pre-positioned dead_code warnings)

### Why no inline Style literals
- Task spec explicit "no inline Style literals" rule honoured. Both `progress_overlay_backdrop` and `progress_overlay_card` are top-level `pub fn` in styles.rs so they're reachable via `crate::styles::progress_overlay_card` from the widget.

### Why no cancel button
- v1 explicitly excludes cancel per the plan ("NO cancel button (v1 doesn't support cancel)"). The backdrop is also click-through for the same reason — there's no way for the user to dismiss the overlay until the operation completes (or fails) on its own.

### Risk / out-of-scope
- 420px fixed card width may feel tight on very narrow windows (min_size is 1024px so this should never be an issue in practice). If a future task ever relaxes the window min_size, the card may need to switch to `Length::Fill` with a max.
- English "Step X/Y" was picked over Korean "단계 X/Y" for consistency. If a future task adds a Korean localization pass, the string should be moved to a centralized i18n table.
- The overlay deliberately does not include the release_prep full step list (per spec — Task 21). When Task 21 is integrated, the existing `release_prep_progress` widget will need to either compose this overlay or be called in addition to it.

### Evidence
- `.omo/evidence/task-15-overlay-build.txt` — `cargo build` + `cargo test` + `cargo fmt --check` summary.

## Task 18 — auto_fetch + release_prep OperationTracker migration (Wave 3, 2026-06-23)

### Pattern established for Task 22 (~60 ManualAction sites)

The Elm-style routing for OperationEvent:

```rust
// At the START site (within the feature guard that prevents duplicate in-flight ops):
let start = Task::done(Message::Operation(OperationEvent::Started {
    id: self.operation_tracker.next_id(),
    kind: OperationKind::ManualAction("<feature>"),
    label: "<human label>".to_string(),
}));
return start.chain(Task::perform(async_work, |result| /* wrap */));

// At the COMPLETION site (where the result message arrives):
let completion = match result {
    Ok(()) => Message::Operation(OperationEvent::Completed {
        id: self.operation_tracker.current_id_for(&OperationKind::ManualAction("<feature>")).unwrap(),
        result: OpResult::Success,
        severity: OpSeverity::Recoverable,
    }),
    Err(msg) => Message::Operation(OperationEvent::Completed {
        id: self.operation_tracker.current_id_for(&OperationKind::ManualAction("<feature>")).unwrap(),
        result: OpResult::Failed(msg),
        severity: OpSeverity::Recoverable,
    }),
};
Task::done(completion).chain(next_step)
```

The `OperationTracker::current_id_for(kind)` lookup closes the cross-handler
id gap (start site doesn't have the id available at the completion site
because the async task result returns on a different message path).

### State additions made

- `OperationKind` now derives `Hash` (needed for `HashMap<OperationKind, OperationId>` key).
- `OperationTracker.current: HashMap<OperationKind, OperationId>` tracks the
  most recent in-flight id keyed by kind. The invariant (at most one
  in-flight op per kind) is enforced by feature guards
  (`if self.operation.loading { return Task::none(); }`).
- `OperationTracker::next_id()` peeks without starting.
- `OperationTracker::start_with_id(id, kind, label)` consumes an explicit id
  (the wire-format path through `Message::Operation`).
- `OperationTracker::current_id_for(kind)` is the cross-handler lookup.
- `OperationTracker::should_show_overlay(threshold_secs)` is the
  `ReleasePrep`-vs-everything-else split for the central overlay.

### Schema field additions

- `OperationState.fatal_error: Option<String>` was already needed by the
  pre-existing partial migration (rebase fatal validation). Added so
  the build compiles.
- `App.overlay_visible: Option<OperationId>` was already needed by the
  pre-existing partial migration (subscription.rs / view.rs). Re-added.

### Per-step events

For `PrepareStepStarted(step)` and `PrepareStepDone { step, result }`:

```rust
ReleasePrepMessage::PrepareStepStarted(step) => {
    self.release_prep.preparing_step = Some(step);
    Task::done(self.step_progressed_event(step))
}
ReleasePrepMessage::PrepareStepDone { step, result } => {
    self.release_prep.completed_preparing_steps.push(step);
    self.release_prep.preparing_step = None;
    match *result {
        Ok(_) => Task::done(self.step_progressed_event(step)),
        Err(message) => Task::done(self.step_failed_event(step, &message)),
    }
}
```

`step_progressed_event` builds `StepProgressed { id, label, current, total }`.
The label flips from "<step name>" to "<step name> done" once `Done` arrives,
so the status bar text reflects the most recent transition.

### Why `current_id_for` instead of stashing the id

The naive approach would be to stash the id on `OperationState` (e.g.
`auto_fetch_op_id: Option<OperationId>`). That's tightly coupled and
requires a new field per kind. The `HashMap<OperationKind, OperationId>`
in `OperationTracker` is cleaner: it scales to any new kind without a
field change, and the invariant (one in-flight per kind) is enforced
by the existing feature guards.

### Test results

- 388 naite-app tests + 265 naite-core tests + 0 doc tests = 653 passed, 0 failed.
- Wave 1 Task 3's 4 baseline tests still pass:
  - release_prep_prepare_baseline_busy_operation
  - release_prep_prepare_baseline_dirty_worktree
  - release_prep_prepare_baseline_sync_failure
  - release_prep_prepare_baseline_success

### Pre-existing build state when this task started

The working tree had an incomplete partial migration from a previous
session (rebase/update.rs and release_prep/update.rs renamed `error` to
`fatal_error`, subscription.rs and view.rs referenced `overlay_visible`),
but the missing fields/methods (`fatal_error` on OperationState,
`overlay_visible` on App, `should_show_overlay` on OperationTracker)
were not added. The build was broken before this task started. Adding
those missing fields/methods was required to make the build compile,
even though they're not strictly part of the auto_fetch/release_prep
migration.

### Pre-existing clippy errors not introduced by this task

- `animated_dots`, `ease_in_out_sine`, `moving_progress_bar`, `spinner_frame`
  unused imports in widgets/mod.rs:31
- `MAX_VISIBLE`, `ToastSeverity` unused imports in widgets/mod.rs:61
- `CompletedOperation.kind` field never read
- `OperationTracker::start` and `OperationTracker::fail` methods never used
  (start was already unused — features go through Started event)
- `ToastSeverity::Success` / `ToastSeverity::Failure` variants never
  constructed
- `Toast::success` / `Toast::failure` associated functions never used
- needless `'a` lifetime in status_bar.rs:141

All pre-existing. None caused by this task.

### Evidence

- `.omo/evidence/task-18-migration-build.txt` — full build + test summary.
