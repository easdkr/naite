# 나이테 (naite)

나이테는 커밋이 쌓여 만드는 히스토리 레이어를 나무의 나이테처럼 읽기
쉽게 보여주는 네이티브 데스크톱 Git 클라이언트입니다. Rust와 iced로
만든 로컬 우선 macOS 앱입니다.

## Status

Early, but past the first read-only slice. naite can open or discover a
local repository, reopen recent repositories, favorite repositories, initialize
an existing folder, clone into a selected parent directory, inspect refs and a
basic commit graph, filter commits, inspect first-parent diffs, and checkout
local branches with a dirty-worktree warning. It also shows a WIP row and
status panel with ignored/submodule grouping, previews per-file WIP diffs with
hunk navigation plus unified, focused-hunk, inline, and split modes,
stages/unstages files and text hunks, creates commits with amend,
co-author, skip-hooks, and commit-then-push options, creates, renames, and
deletes local branches, manages stashes including branch creation from a stash,
runs current-branch fetch, fetch-all, pull mode, and push operations, supports
merge/rebase, guided history-surgery actions, tag create/delete, worktree
list/create/open/remove/lock flows, repo tabs, a local workspace dashboard with
multi-repo fetch/pull/open/locate/remove actions, and a per-repo/worktree
terminal command panel. It also includes a GitHub CLI-backed pull request panel
for listing, filtering, searching, creating, opening, and checking out pull
requests, including new-worktree checkout and basic CI/review/issue-link
metadata, plus an issue panel for listing, filtering, searching, and opening
issues. Implemented actions are exposed through a compact command palette.

The current write surface is intentionally narrow: clone, init, local branch
checkout/create/rename/delete, file and text-hunk staging/unstaging/discard,
commit creation with common options, stash create/apply/pop/drop, and
create-branch-from-stash, plus current-branch remote sync, explicit pull modes,
merge/rebase, tag operations, worktree management, local workspace management,
scoped terminal commands, and GitHub pull request list/create/open/checkout
flows through the existing `gh` CLI. Provider auth beyond the local `gh` setup,
non-GitHub providers, PR merge actions, workspace-wide PR aggregation, and full
terminal emulation remain roadmap work.

## Naite-Specific Features

naite is not trying to be a clone of every Git GUI. Its special surfaces are
built around reading layered history, keeping local work visible, and making
risky Git operations inspectable before they run.

- **Layered history with a first-class WIP row:** the commit list, graph lanes,
  refs, branch sync chip, and current worktree status are shown together. The
  uncommitted WIP row opens the same diff detail surface as commits, so staged,
  unstaged, ignored, submodule, and conflicted work can be inspected without
  leaving the history view.
- **Diff inspection built for hunk work:** WIP and commit diffs support file
  selection, hunk navigation, syntax-highlighted lines, and unified, focused
  hunk, inline, and split layouts. File and text-hunk stage, unstage, and
  discard actions stay close to the hunk being reviewed.
- **Visual history surgery:** merge, rebase, conflict resolution, reword, drop,
  squash, fixup, edit, reorder, undo, and redo flows use explicit prompts that
  show the target commit or ref and the Git command shape before execution.
- **Interactive rebase planner:** selecting a rebase target opens an editable
  todo-list style planner with action chips, row reordering, per-commit diff
  detail, reword drafts, and presets such as Keep Mine, Squash Mine, and Squash
  All. The plan can be applied locally or as rebase-then-force-push.
- **Release Promotion:** a guided source-to-target branch promotion flow detects
  candidate profiles such as `staging -> main`, fetches remote refs, force-syncs
  both branches from the remote, can create a safety backup branch, opens the
  rebase planner, then offers explicit follow-up actions to update the target,
  push it, and sync the source branch back from the target with
  `--force-with-lease`.
- **PR, issue, and worktree handoff:** the GitHub CLI-backed panels list,
  filter, search, create, open, and check out pull requests, including checkout
  into a new worktree. PR rows carry CI, review, label, reviewer, draft, merge
  state, and linked-issue metadata, while issue rows stay filterable by open,
  assigned, mentioned, closed, or search mode when `gh` can provide it.
- **Local workspace cockpit:** recent and favorite repositories, repo tabs, and
  the workspace dashboard keep multiple local repositories visible. The
  dashboard summarizes dirty state, ahead/behind counts, worktree counts, last
  fetch age, and supports multi-repo fetch/pull/open/locate/remove flows.
- **Repo-scoped terminal panel:** terminal sessions are attached to the active
  repository or worktree instead of being global. The panel tracks sessions,
  cwd, shell status, last command/exit state, zsh integration events, history,
  path completion, and Git subcommand suggestions.
- **Local-first provider boundary:** core Git reading and writing lives in
  `naite-core`, while provider features go through the user's existing `gh`
  setup. The app does not introduce cloud sync, telemetry, token storage, or
  server-side source upload.

## Stack

- **UI:** [`iced`](https://crates.io/crates/iced) — retained-mode Rust GUI with first-class
  animations and a `Canvas` widget for future custom rendering (commit graph).
- **Git:** [`gix`](https://crates.io/crates/gix) — pure-Rust Git implementation.
- **File picker:** [`rfd`](https://crates.io/crates/rfd) — native folder/file dialogs.
- **Async:** `tokio` — used for blocking Git work off the UI thread.

## Layout

```
naite/
├── Cargo.toml                   ← workspace root
└── crates/
    ├── naite-core/              ← Git domain logic (depends on gix)
    │   └── src/                     Repository reads/writes, diff/status parsing
    └── naite-app/               ← iced UI (depends on naite-core)
        └── src/                     App state, persistence, view, update
```

The split keeps Git logic testable on its own and free of UI dependencies.
The UI never imports `gix` directly.

## Develop

```bash
cargo run -p naite-app          # debug
cargo run -p naite-app --release # release
scripts/macos-bundle.sh               # build target/debug/naite.app
scripts/macos-bundle.sh --release     # build target/release/naite.app
scripts/macos-install.sh              # build, install, and open unsigned release app
scripts/macos-install.sh --no-pause   # install without waiting before terminal exit
open target/debug/naite.app           # run with the project icon on macOS
```

## Repository Assets

- GitHub social preview: `.github/social-preview.png`
- Upload path: GitHub repository Settings → Social preview

## Roadmap

Implemented:
- [x] Open a local repository (folder picker)
- [x] Open a local repository from a command-line path
- [x] Render commit list and basic graph lanes (newest first, paged in 500-commit chunks)
- [x] Render grouped local/remote/tag refs
- [x] Inspect first-parent commit diffs
- [x] Checkout local branches with a dirty-worktree warning
- [x] Persist recent repositories and favorites locally
- [x] Clone and initialize repositories via system `git`
- [x] WIP node, grouped status panel, and per-file WIP diffs
- [x] Hunk navigation plus unified/focused/inline/split diff modes
- [x] Stage / unstage files and text hunks / commit with common options
- [x] Create, rename, and delete local branches
- [x] Stash create/list/inspect/apply/pop/drop/create branch
- [x] Fetch current/all remotes / pull --ff-only, --ff, --rebase / push current branch
- [x] Merge/rebase, conflict continue/abort, guided history surgery, tag create/delete
- [x] Worktree list/create/open/remove/lock and repo tabs
- [x] Local workspace dashboard with multi-repo fetch/pull/open/locate/remove
- [x] Per-repo/worktree terminal command panel
- [x] GitHub PR list/filter/search/create/open/checkout via `gh`
- [x] GitHub issue list/filter/search/open via `gh`
- [x] Command palette MVP for implemented actions

Next:
- [ ] Remote auth/error guidance
- [ ] Non-GitHub provider expansion and PR merge safeguards
- [ ] Full terminal emulation and streaming output
