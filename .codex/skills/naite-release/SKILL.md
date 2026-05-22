---
name: naite-release
description: Use when Codex should perform a naite product release by bumping the Cargo workspace version, refreshing Cargo.lock, running release checks, committing with the Lore protocol, tagging vX.Y.Z, pushing the tag, and optionally creating a GitHub Release with macOS assets. Trigger for requests like release, patch/minor/major release, publish a new naite version, or create vX.Y.Z.
---

# naite Release

## Default Behavior

Perform a real product release, not a workflow-design task.

If the user asks for a release without specifying a bump type or version, default to `patch`. If they specify `minor`, `major`, or an explicit `X.Y.Z`, use that. If the worktree is dirty, stop and report the exact files; do not mix unrelated edits into a release commit.

## Command

Use the bundled release runner from the repository root:

```bash
python3 .codex/skills/naite-release/scripts/naite_release.py --bump patch
python3 .codex/skills/naite-release/scripts/naite_release.py --bump minor
python3 .codex/skills/naite-release/scripts/naite_release.py --version 0.2.0
```

By default the runner:

1. Requires a clean git worktree.
2. Reads `[workspace.package] version` from `Cargo.toml`.
3. Bumps the version.
4. Runs `cargo check --workspace` once to refresh `Cargo.lock`.
5. Runs release verification:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --locked -- -D warnings`
   - `cargo test --workspace --locked`
6. Commits `Cargo.toml` and `Cargo.lock` with a Lore-style release commit.
7. Creates an annotated `vX.Y.Z` tag.
8. Checks that the remote tag and GitHub Release do not already exist.
9. Pushes the release commit and tag.

To create the GitHub Release and local macOS zip assets from this machine as part of the same run, add:

```bash
--publish-github-release
```

Use dry-run first when checking the next version:

```bash
python3 .codex/skills/naite-release/scripts/naite_release.py --bump patch --dry-run
```

## Guardrails

- Never use `git tag --force`, `git push --force`, `gh release edit`, or `gh release upload --clobber`.
- Do not release from a dirty worktree.
- Do not continue if `vX.Y.Z` already exists locally, on the remote, or as a GitHub Release.
- Keep `CFBundleShortVersionString` equal to Cargo semver when publishing macOS assets.
- Keep `CFBundleVersion` monotonic. For local GitHub Release publishing, the runner uses the semver as the bundle build version unless a caller provides a different release process.
- If a verification command fails, fix the failure before recreating the release commit/tag. Delete only the local release tag created by the failed run if it has not been pushed.

## Reporting

Final response must include:

- Previous version and new version.
- Commit SHA and tag.
- Whether the commit/tag were pushed.
- Whether GitHub Release assets were created.
- Verification commands run and any skipped checks.
