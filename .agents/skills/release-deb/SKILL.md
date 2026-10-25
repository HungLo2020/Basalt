---
name: release-deb
description: Release Basalt by bumping the workspace version, verifying locally, committing and pushing, watching every GitHub Actions workflow for the pushed commit, and publishing the Debian package to MattPackages. Use only when the user explicitly asks to commit, push, bump the version, verify workflows, and publish a .deb release.
---

# Release Basalt (.deb)

Use this workflow only when the user explicitly asks for a release. For an
ordinary commit, stop at the commit step. Never force-push, rewrite published
history, move or delete tags, publish the same version twice, or modify another
repository.

The order matters: the version is committed and validated on GitHub **before**
the package is published. That way the `.deb` distributed to users was built
from a commit whose workflows are green.

## 1. Preflight

From the repository root:

```sh
git fetch origin
git status --short
git diff --check
git branch --show-current
git log -5 --oneline
git rev-list --left-right --count HEAD...origin/main
```

Review every changed and untracked path. Never stage generated or local data:

- `target/`, `builds/`, `Build/`
- `DevUtils/.downloaded/`, `__pycache__/`, `*.pyc`
- `gui-smoke/`, screenshots, logs, local JSON presets
- credentials, tokens, or anything outside the repository

If an unexpected change belongs to the user, stop and ask instead of committing
or deleting it. Do not release from a dirty tree you do not understand.

Confirm the release tooling before changing anything:

```sh
gh auth status
python3 DevUtils/PublishMattOSPackage.py doctor
```

`doctor` reporting `token_configured: False` is informational, not a blocker:
the MattPackages server currently accepts uploads from the trusted tailnet
without a token. Do not stop a release solely because of that line. If the
upload later returns HTTP 401 or 403, configure a token in one of these places
and retry:

- `MATTOS_REPOSITORY_TOKEN` in the environment
- `~/.config/mattos-repository/token`
- `/etc/mattos-repository/client-token`

Never invent a token or claim a package was published without verifying the
repository index.

## 2. Choose and apply the version

Read the current `[workspace.package] version` in `Cargo.toml`. If the user
named a version, use it. Otherwise follow semantic versioning:

- **patch** (`0.8.0` → `0.8.1`): fixes, documentation, tooling, small catalog
  corrections
- **minor** (`0.8.0` → `0.9.0`): new user-visible behavior, a new catalog game,
  or a new release workflow
- **major**: breaking changes to catalog format, CLI, or saved data

State the chosen version and why. Change only the workspace version; do not
upgrade dependencies in the same release.

```sh
cargo update --workspace
git diff -- Cargo.toml Cargo.lock
```

`Cargo.lock` should change only the workspace crates (`basalt`,
`basalt-core`, `basalt-cli`, `basalt-gui-kirigami`, and any other local
members). If unrelated dependency versions change, restore the lockfile and
update only the workspace members.

## 3. Verify locally

Every CI-enforced command must pass before committing:

```sh
cargo check --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D clippy::disallowed_methods -D clippy::disallowed_types
cargo test --locked --workspace
bash DevUtils/LintQml.sh
bash DevUtils/SmokeTestGui.sh
python3 -m unittest discover -s tests
```

`cargo fmt --all -- --check` is advisory for now: the repository has
pre-existing formatting drift and CI does not enforce it. Do not fold a
repo-wide reformat into a release commit; report the drift instead.

The GUI checks need Qt 6 development tools (`qmake6`, `qmllint`) and run
offscreen; see the README. If a required check cannot run in this environment,
say so explicitly and do not describe the release as fully verified. Do not
commit with a failing required check.

## 4. Commit

Stage explicit paths, not the whole tree by reflex:

```sh
git add Cargo.toml Cargo.lock README.md AGENTS.md .claude .agents .github DevUtils crates resources tests
git status --short
```

Use a Conventional Commit message that describes the release:

```sh
git commit -m "release: publish Basalt 0.9.0"
```

If the change is primarily a feature, prefer a feature message and mention the
version in the body, for example:

```sh
git commit -m "feat(catalog): add Classic Classics entry" \
  -m "Release 0.9.0."
```

Never amend a commit that is already pushed. Make a follow-up commit instead.

## 5. Push

```sh
git push origin HEAD
```

If the remote moved, inspect it and create a merge or rebase that preserves the
user's work. Do not use `git push --force` or `--force-with-lease` without
explicit permission.

## 6. Wait for every GitHub Actions workflow

Use the pushed SHA, not just the branch name:

```sh
sha="$(git rev-parse HEAD)"
git rev-parse --abbrev-ref --symbolic-full-name @{u}
gh run list --commit "$sha" --limit 20 \
  --json databaseId,name,status,conclusion,url
```

Wait until every run for this SHA has completed:

```sh
gh run watch <run-id> --exit-status
```

Requirements:

- Every workflow triggered by the push must conclude `success` (including
  `CI`; the manual DEB workflow does not run on a push).
- Do not publish while any run is queued or in progress.
- If a run fails, inspect only the failing logs:

  ```sh
  gh run view <run-id> --log-failed
  ```

- Fix the cause, rerun the local checks, make a **new** commit, push, and watch
  the new SHA's runs. Do not amend or force-push the failed commit.
- If `gh` is unavailable or unauthenticated, report the push result and the
  failing/missing evidence. Do not claim the workflows passed.

Do not continue to publishing until CI is green.

## 7. Publish the Debian package

Confirm the working tree is clean and HEAD is the pushed, validated commit:

```sh
git status --short
git rev-parse HEAD
```

Then publish:

```sh
python3 DevUtils/PublishMattOSPackage.py doctor
python3 DevUtils/PublishMattOSPackage.py publish
```

The script refuses a version that is not newer than every published version,
builds the `.deb` from the current tree, and uploads it through the
authoritative repository manager. It is safe to retry the same version after a
network or server failure, but not after a successful publish.

If publishing fails:

- **Authentication/configuration**: stop and report the missing token or
  configuration; do not build a second version to work around it.
- **Version already published**: verify the repository index. If the version
  is there, this release is already published; do not overwrite it. If it is
  not there, investigate before retrying.
- **Build failure**: fix the build, rerun local checks, commit the fix, push,
  wait for the new workflows, then publish the still-unpublished version.
- **Upload failure after a successful build**: retry the upload/publish; the
  same version may still be unpublished.

## 8. Verify the published package

Check the built metadata and the public repository index:

```sh
cat builds/latest-build.env
dpkg-deb --info builds/basalt_<version>_amd64.deb
curl -fsSL 'https://mattpackages.mattsherfey.com/dists/stable/main/binary-amd64/Packages?cachebust='"$(date +%s)" |
  awk -v v='<version>' '$0 == "Package: basalt" {p=1} p && $0 == "Version: " v {print; found=1} END {exit !found}'
```

The index must list the new version. Optionally confirm the `.deb` filename and
that its `Version` field matches. Do not install the package as part of the
release unless the user asks.

## 9. Final report

Report:

- old and new version, and the bump reason
- commit SHA and message
- branch and push result
- every workflow name, run ID, URL, and final conclusion
- publish command result, artifact path, and repository-index verification
- any check that could not run, and why

Leave the working tree clean. If the user asked to keep changes uncommitted,
say that explicitly instead of committing them.
