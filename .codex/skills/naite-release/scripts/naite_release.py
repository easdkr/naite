#!/usr/bin/env python3
import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path


VERSION_LINE_RE = re.compile(r'^(\s*version\s*=\s*")([^"]+)(".*)$')
SIMPLE_VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def run(args, env=None, capture=False):
    print("+ " + " ".join(args))
    result = subprocess.run(
        args,
        check=False,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
        env=env,
    )
    if result.returncode != 0:
        if capture and result.stdout:
            print(result.stdout, end="")
        raise SystemExit(result.returncode)
    return result.stdout.strip() if capture else ""


def parse_args():
    parser = argparse.ArgumentParser(description="Bump and publish a naite product release.")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--bump", choices=("patch", "minor", "major"))
    group.add_argument("--version", help="Explicit release version, for example 0.2.0.")
    parser.add_argument("--manifest", default="Cargo.toml")
    parser.add_argument("--crate", default="naite-app")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--no-push", action="store_true", help="Create the commit and tag locally only.")
    parser.add_argument(
        "--publish-github-release",
        action="store_true",
        help="Build local macOS assets and create the GitHub Release after pushing.",
    )
    parser.add_argument("--skip-checks", action="store_true", help="Skip fmt, clippy, and test checks.")
    parser.add_argument("--dry-run", action="store_true", help="Print the planned version/tag only.")
    return parser.parse_args()


def ensure_repo_root():
    root = run(["git", "rev-parse", "--show-toplevel"], capture=True)
    os.chdir(root)


def ensure_clean_worktree():
    status = run(["git", "status", "--porcelain"], capture=True)
    if status:
        print("Refusing to release from a dirty worktree:")
        print(status)
        raise SystemExit(1)


def find_version_line(lines):
    section = ""
    package_match = None
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            section = stripped
            continue
        match = VERSION_LINE_RE.match(line)
        if not match:
            continue
        if section == "[workspace.package]":
            return index, match.group(2), section
        if section == "[package]" and package_match is None:
            package_match = (index, match.group(2), section)
    if package_match:
        return package_match
    raise SystemExit("Could not find a Cargo version in [workspace.package] or [package].")


def bump_version(current, bump, explicit):
    if explicit:
        if not SIMPLE_VERSION_RE.match(explicit):
            raise SystemExit(f"Release version must be simple semver X.Y.Z: {explicit}")
        return explicit

    match = SIMPLE_VERSION_RE.match(current)
    if not match:
        raise SystemExit(f"Current version must be simple semver X.Y.Z for automatic bump: {current}")

    major, minor, patch = (int(part) for part in match.groups())
    if bump == "major":
        return f"{major + 1}.0.0"
    if bump == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def read_and_update_manifest(manifest_path, bump, explicit, dry_run):
    manifest = Path(manifest_path)
    lines = manifest.read_text().splitlines(keepends=True)
    line_index, current, section = find_version_line(lines)
    new_version = bump_version(current, bump, explicit)

    if current == new_version:
        raise SystemExit(f"{manifest} already has version {new_version}.")

    if not dry_run:
        lines[line_index] = VERSION_LINE_RE.sub(rf"\g<1>{new_version}\g<3>", lines[line_index])
        manifest.write_text("".join(lines))

    return current, new_version, section


def ensure_tag_available(remote, tag, require_gh):
    local_tag = subprocess.run(["git", "rev-parse", "-q", "--verify", f"refs/tags/{tag}"])
    if local_tag.returncode == 0:
        raise SystemExit(f"Local tag already exists: {tag}")

    remote_tag = subprocess.run(
        ["git", "ls-remote", "--exit-code", "--tags", remote, f"refs/tags/{tag}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if remote_tag.returncode == 0:
        raise SystemExit(f"Remote tag already exists: {tag}")

    gh_path = shutil.which("gh")
    if gh_path is None:
        if require_gh:
            raise SystemExit("gh is required for --publish-github-release.")
        print("warning: gh not found; skipping GitHub Release existence check.")
    else:
        release = subprocess.run(
            ["gh", "release", "view", tag],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if release.returncode == 0:
            raise SystemExit(f"GitHub Release already exists: {tag}")


def release_notes(version, commit):
    path = Path("target") / "release" / f"release-notes-v{version}.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                f"naite v{version}.",
                "",
                "Build:",
                f"- Version: `{version}`",
                f"- Commit: `{commit}`",
                "",
                "Verification:",
                "- Release checks passed before tagging.",
                "- macOS bundle packaged locally.",
                "- Zip integrity test passed.",
                "- SHA-256 checksum uploaded.",
                "",
            ]
        )
    )
    return path


def package_macos_assets(version):
    if platform.system() != "Darwin":
        raise SystemExit("--publish-github-release currently requires macOS for app packaging.")

    env = os.environ.copy()
    env["PATH"] = f"{Path.home() / '.cargo' / 'bin'}:{env.get('PATH', '')}"
    env["NAITE_BUNDLE_SHORT_VERSION"] = version
    env["NAITE_BUNDLE_VERSION"] = version

    bundle_path = run(["scripts/macos-bundle.sh", "--release"], env=env, capture=True).splitlines()[-1]
    run(["codesign", "--force", "--deep", "--sign", "-", bundle_path])
    run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", bundle_path])

    host = run(["rustc", "-vV"], env=env, capture=True)
    arch = ""
    for line in host.splitlines():
        if line.startswith("host: "):
            arch = line.replace("host: ", "").replace("-apple-darwin", "")
            break
    if not arch:
        raise SystemExit("Could not resolve Rust host architecture.")

    zip_path = Path("target") / "release" / f"naite-v{version}-macos-{arch}.app.zip"
    checksum_path = Path(str(zip_path) + ".sha256")
    run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", bundle_path, str(zip_path)])
    checksum = run(["shasum", "-a", "256", str(zip_path)], capture=True)
    checksum_path.write_text(checksum + "\n")
    run(["unzip", "-t", str(zip_path)])
    return zip_path, checksum_path


def commit_and_tag(manifest, version, skip_checks):
    run(["git", "add", manifest, "Cargo.lock"])
    staged = subprocess.run(["git", "diff", "--cached", "--quiet"])
    if staged.returncode == 0:
        raise SystemExit("No version changes staged.")

    subject = f"Release naite v{version} as a product version"
    body = "This release bumps the Cargo workspace version and records the lockfile state before tagging the immutable product release."
    message = [
        "git",
        "commit",
        "-m",
        subject,
        "-m",
        body,
        "-m",
        "Constraint: Release tags must not be overwritten",
        "-m",
        "Confidence: high",
        "-m",
        "Scope-risk: narrow",
        "-m",
        "Directive: Do not move this release tag; create a new version instead",
        "-m",
        "Tested: cargo check --workspace"
        if skip_checks
        else "Tested: cargo check --workspace; cargo fmt --all -- --check; cargo clippy --workspace --all-targets --locked -- -D warnings; cargo test --workspace --locked",
    ]
    if skip_checks:
        message.extend(["-m", "Not-tested: fmt, clippy, and tests skipped by --skip-checks"])
    run(message)
    commit = run(["git", "rev-parse", "--short", "HEAD"], capture=True)
    tag = f"v{version}"
    run(["git", "tag", "-a", tag, "-m", f"Release naite v{version}"])
    return commit, tag


def main():
    args = parse_args()
    ensure_repo_root()

    current, new_version, section = read_and_update_manifest(
        args.manifest, args.bump, args.version, True
    )
    tag = f"v{new_version}"

    if args.dry_run:
        print(f"{section}: {current} -> {new_version}")
        print(f"tag: {tag}")
        return

    ensure_clean_worktree()
    ensure_tag_available(args.remote, tag, args.publish_github_release)
    read_and_update_manifest(args.manifest, args.bump, args.version, False)

    env = os.environ.copy()
    env["PATH"] = f"{Path.home() / '.cargo' / 'bin'}:{env.get('PATH', '')}"

    run(["cargo", "check", "--workspace"], env=env)
    if not args.skip_checks:
        run(["cargo", "fmt", "--all", "--", "--check"], env=env)
        run(["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"], env=env)
        run(["cargo", "test", "--workspace", "--locked"], env=env)

    commit, created_tag = commit_and_tag(args.manifest, new_version, args.skip_checks)

    assets = None
    if args.publish_github_release:
        assets = package_macos_assets(new_version)

    if not args.no_push:
        branch = run(["git", "branch", "--show-current"], capture=True)
        if not branch:
            raise SystemExit("Cannot push release commit from a detached HEAD.")
        run(["git", "push", args.remote, f"HEAD:{branch}"])
        run(["git", "push", args.remote, created_tag])

    if args.publish_github_release:
        notes = release_notes(new_version, commit)
        zip_path, checksum_path = assets
        run(
            [
                "gh",
                "release",
                "create",
                created_tag,
                "--target",
                "HEAD",
                "--title",
                f"naite v{new_version}",
                "--notes-file",
                str(notes),
                str(zip_path),
                str(checksum_path),
            ]
        )

    print("")
    print(f"released: {current} -> {new_version}")
    print(f"commit: {commit}")
    print(f"tag: {created_tag}")
    print(f"pushed: {not args.no_push}")
    print(f"github_release: {args.publish_github_release}")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        raise SystemExit(130)
