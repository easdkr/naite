# naite-core Agent Guide

This crate owns Git/domain behavior only. Keep `naite-app` free to render
typed core results without importing `gix` or running Git directly.

## Module Map

- `lib.rs`: module declarations and public re-exports only.
- `error.rs`: shared `Error` enum.
- `repo.rs`: `Repository` construction plus `pub(crate)` Git helpers.
- `command.rs`: git subprocess wrapper and command error formatting.
- `commits.rs`: commit summaries and parent lookup.
- `refs.rs`: ref summaries, upstream lookup, and branch sync status.
- `graph.rs`: commit graph layout and graph tests.
- `diff/`: commit diff types, patch helpers, and diff parsers.
- `worktree/`: status and working-tree diff parsing/operations.
- `ops/`: user-triggered Git operations (`fetch`, `pull`, `push`,
  `checkout`, `branch`, `stage`, `discard`, `commit`).
- `test_helpers.rs`: shared test fixtures, compiled only for tests.

## Where To Add Code

- New Git operation: add `ops/<name>.rs`, declare it in `ops/mod.rs`, and add an
  `impl Repository { pub fn <name>(...) }` method there.
- New operation error: add the variant to `error.rs` and keep display text
  user-facing but concise.
- New parser: put the typed result near its domain module and add co-located
  parser tests at the bottom of the same file.
- New shared Git helper: add it to `repo.rs` as `pub(crate)` only after checking
  existing `git` and `git_allowing_exit_codes` helpers.

## Rules

- Do not import iced or app-layer state here.
- Keep `Repository` fields crate-private so sibling modules can add focused
  `impl Repository` blocks without widening the public API.
- Validate status paths through `worktree::validate_status_path` before any
  path-taking operation mutates index or worktree state.
- Prefer set/batch Git calls over per-row subprocesses for scans.
- Keep tests close to pure parsing, graph, and operation behavior.
