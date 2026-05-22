# naite Agent Guide

naite is a native Rust desktop Git client for reading layered commit history.
Keep this file high-signal and update it when the build, test, dependency, or
architecture contracts change.

## Current Stack

- Workspace: Cargo workspace with `crates/naite-core` and `crates/naite-app`.
- Rust: stable Rust only. Do not use nightly-only features unless explicitly requested.
- Edition: workspace edition `2021`.
- UI: `iced` is declared as `0.13` and currently locked to `0.13.1`.
- Git domain: `gix` in `naite-core`; the UI crate must not import `gix` directly.
- File dialogs: `rfd`.
- Async runtime: `tokio`, with blocking Git work moved off the UI thread.

Latest documentation note: docs.rs currently exposes newer `iced` API docs than
this project is pinned to. Check the pinned crate version before applying
examples from `docs.rs/latest`, and upgrade `Cargo.toml`/`Cargo.lock` as an
explicit dependency-change task if a newer iced-only API is needed.

## Project Boundaries

- `crates/naite-core`: repository access, Git command wrappers, diff parsing,
  graph layout, and plain data structures consumed by the UI.
- `crates/naite-app`: iced application state, root messages, feature folders
  under `src/features/`, pure views/widgets under `src/widgets/`, icons,
  styles, and theme tokens.
Preserve the core/UI separation. Add structured data or domain operations to
`naite-core`, then render or trigger them from `naite-app`.

## Commands

In this Conductor environment, `cargo` may not be on `PATH`. Use:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Common checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p naite-app
cargo run -p naite-app --release
```

For small documentation-only changes, `git diff --check` is sufficient unless a
code path was touched.

## Rust Rules

- Prefer small, typed domain structures over passing raw strings between crates.
- Return `Result<T, Error>` from core operations and keep user-facing string
  conversion at the app/task boundary.
- Avoid `unwrap`, `expect`, and `panic` in production paths. Use them only in
  tests where the invariant is the point of the test.
- Keep blocking filesystem, Git, or process work out of iced update/view code.
  Use `Task::perform` plus `tokio::task::spawn_blocking`.
- Do not add dependencies without an explicit user request. First check whether
  std, `gix`, `iced`, `rfd`, `thiserror`, or `tokio` already covers the need.
- Prefer set/batch operations for repository scans. Avoid per-row Git subprocess
  calls in commit-list, graph, status, or diff rendering paths.
- Keep tests near the core logic when behavior is independent of UI rendering.

## iced Rules

- Follow the current Elm-style iced shape: `Message`, `App::update`,
  `App::view`, `Task<Message>`, and `Subscription<Message>`.
- `view` and widget functions must remain pure render descriptions. Do not do
  I/O, Git calls, sleeps, or mutation outside iced state construction there.
- Route user actions through `Message` variants, then let `update` mutate state
  and return a `Task` when side effects are needed.
- Map child messages/tasks explicitly when splitting UI modules; keep state
  ownership obvious.
- Use `Element<'_, Message>` and existing widget-builder helpers in
  `crates/naite-app/src/widgets/` before adding new rendering surfaces.
- Keep styling in `styles.rs` and design tokens in `theme.rs`. Do not inline
  one-off `button::Style` or `container::Style` literals in views.
- Preserve stable dimensions for fixed-format UI: commit rows, graph canvas,
  pane headers, buttons, and list cells should not resize because labels change.
- `ROW_HEIGHT` is shared between the commit row, graph canvas, and scroll math;
  do not change it without checking all three uses.
- Prefer compact controls, contextual menus, and keyboard-first flows over
  decorative panels or large explanatory UI text.

## Git UI Safety

- Destructive or history-rewriting operations must show the exact target,
  current branch/worktree state, and the fallback/abort path before execution.
- Existing write surface is local branch checkout. Keep new write operations
  explicit and reversible where Git allows it.
- Surface exact stderr for failed Git commands, but keep secrets/tokens out of
  UI and logs if provider integrations are added later.
- The app is local-first. Do not introduce telemetry, cloud sync, token storage,
  or source upload without an explicit product decision.

## Verification

Before reporting a code change as complete:

1. Run the narrowest relevant tests first.
2. Run `cargo fmt --all -- --check` for Rust edits.
3. Run `cargo clippy --workspace --all-targets --locked -- -D warnings` for
   non-trivial Rust changes.
4. Run `cargo test --workspace --locked` when core behavior, message flow, diff
   parsing, graph layout, or Git operations changed.
5. For UI changes, run `cargo run -p naite-app` when feasible and inspect the
   affected view manually.

Final reports should mention changed files, verification commands, and any
remaining risks or skipped checks.
