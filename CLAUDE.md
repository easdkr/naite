# naite Claude Guide

This file is the Claude-facing companion to `AGENTS.md`. Treat `AGENTS.md` as
the canonical agent contract for this repository; this file mirrors the parts
Claude should keep top of mind while working in naite.

## Repository Shape

- Rust Cargo workspace, stable toolchain, edition `2021`.
- `crates/naite-core`: Git/domain layer. Owns `gix`, Git subprocess calls,
  diff parsing, graph layout, and plain data structures.
- `crates/naite-app`: iced desktop UI. Owns app state, `Message`, `update`,
  `view`, widget builders, theme tokens, styles, and SVG icons.
- The UI crate must not depend on `gix` directly. Add domain operations to
  `naite-core`, then consume structured results from `naite-app`.

## Dependency Context

- `iced` is declared as `0.13` and locked to `0.13.1`.
- Newer `docs.rs/latest` iced examples may refer to APIs beyond this pinned
  version. Verify against the pinned version before using latest docs.
- Do not add new dependencies unless the user explicitly asks. Prefer the
  existing stack: std, `gix`, `iced`, `rfd`, `thiserror`, and `tokio`.

## iced Implementation Rules

- Follow the current iced architecture: `Message`, `App::update`, `App::view`,
  `Task<Message>`, and `Subscription<Message>`.
- Keep `view` and widget builders pure. No Git calls, filesystem work, sleeps,
  subprocesses, or mutation from render code.
- Route side effects through `update` by returning `Task::perform`.
- Run blocking Git/filesystem/process work inside `tokio::task::spawn_blocking`.
- Keep styling in `crates/naite-app/src/styles.rs` and tokens/fonts/colors in
  `crates/naite-app/src/theme.rs`.
- Use existing builders in `crates/naite-app/src/widgets.rs` before adding
  new UI surfaces.
- Preserve stable dimensions for rows, graph canvas, panes, buttons, and cells.
  `ROW_HEIGHT` is coupled to row rendering, graph rendering, and scroll math.

## Rust and Git Safety

- Prefer typed data structures and `Result<T, Error>` in core code.
- Keep user-facing string conversion at the app/task boundary.
- Avoid `unwrap`, `expect`, and `panic` in production paths.
- Avoid per-row Git subprocess calls; use batch or set-based repository reads.
- Destructive/history-rewriting Git operations require explicit target,
  current state, and fallback/abort information before execution.
- Keep naite local-first. Do not add telemetry, cloud sync, token storage, or
  source upload without an explicit product decision.

## Local Commands

Conductor may start without Cargo on `PATH`; prefix commands with:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Run these as appropriate:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p naite-app
```

For docs-only edits, also run:

```bash
git diff --check
```

Report changed files, checks run, and skipped verification.
