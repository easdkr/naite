# naite-app Agent Guide

This crate owns iced state, messages, tasks, views, widgets, icons, styles, and
theme tokens. It may call `naite-core`, but must not import `gix` directly.

## Module Map

- `main.rs`: module declarations, app bootstrap, initial repository/catalog
  task wiring, and root re-exports.
- `app.rs`: `App`, pane IDs, command palette item metadata, and pure derived
  state helpers.
- `message.rs`: root `Message`, global keyboard messages, and `From<feature::Message>`
  adapters.
- `update.rs`: global update flow for selection, search, focus refresh, panes,
  and diff loading.
- `tasks.rs`: global async tasks that are not owned by a feature.
- `features/`: feature-owned message, update, and task modules.
- `widgets/`: pure render builders by panel; no IO or state mutation.
- `state.rs`: shared app state structs used across multiple modules.
- `styles.rs` and `theme.rs`: visual tokens and iced style functions.

## Feature Folder Convention

Every user-facing operation should live under `src/features/<name>/` when it
has its own messages, side effects, or update flow.

Use the current folders as examples:

- `fetch`, `pull`, `push`: minimal remote operations.
- `stage`, `discard`: working-tree mutations with typed targets.
- `checkout`: dirty-worktree confirmation plus checkout task.
- `commit`, `branch_create`: form-driven write operations.
- `command_palette`: command search, selection, and dispatch.
- `catalog`, `repo_open`: startup, recents, open/init/clone repository IO.

For a new feature:

1. Create `features/<name>/message.rs` with a small feature enum.
2. Add `features/<name>/task.rs` only if the feature performs IO.
3. Add `features/<name>/update.rs` with `impl App { update_<name>(...) }`.
4. Declare the module in `features/mod.rs`.
5. Add `Message::<Name>(<name>::Message)` and `impl From<<name>::Message> for Message`
   in `message.rs`.
6. Route the root `App::update` arm to the feature update function.
7. Wire widgets with `Message::from(<name>::Message::...)`.

## Boundaries

- Feature tasks may call `naite-core` and convert errors to `String` at the
  app/task boundary.
- Feature update functions may read `App` state directly. Cross-feature writes
  should go through the owning feature update/helper when one exists.
- Global `update.rs` should stay focused on shared selection/search/diff flows.
  Do not add new operation-specific match arms there.
- Widgets stay pure: no filesystem, Git, sleeps, mutation, or task creation.
- Keep styles in `styles.rs` and tokens in `theme.rs`; do not inline one-off
  style literals in feature or widget code.
