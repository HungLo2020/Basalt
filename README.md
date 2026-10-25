# Basalt

Basalt is a minimal, focused game launcher for Linux.

This project follows the
[CODING-STANDARDS.md](https://github.com/HungLo2020/LinuxScripts/blob/master/CODING-STANDARDS.md)
from HungLo2020/LinuxScripts.

## Install

On a system with the MattPackages apt repository configured:

`sudo apt install basalt`

Basalt is then updated with the rest of the system (`sudo apt upgrade`); it has no built-in updater.

Without the repository, build and install the `.deb` from a checkout (that install will not update itself):

`git clone https://github.com/HungLo2020/Basalt.git && cd Basalt && ./Install.sh`

## Project Goals

- Keep the launcher simple and fast.
- Prioritize Linux as the primary platform.
- Minimize dependencies and keep setup lightweight.
- Support launching Steam games.
- Support launching GOG games.
- Support launching games through custom scripts or standalone executables.
- Integrate with MattMC (a Minecraft fork) to download, install, and launch it.
- Provide a robust CLI for scripting and automation workflows.

## Catalog games and installation libraries

Game definitions in `resources/games/*.json` describe where a game's releases come from, how to
launch it, and what Basalt can do with it. Games that follow [STANDARDS.md](STANDARDS.md)
(`schema_version: 2`) need only a few lines: Basalt knows from the standard folder layout what to
replace on update, what to back up and what to sync, and provides update, backup, restore, sync,
sync up, sync down and clear cache for them. Their entries can relabel, disable or reconfigure any
of these, replace one with a script that ships in the release, or add their own script actions.
Every `resources/games/*.json` file is discovered and embedded automatically when Basalt is
built. Adding or removing an entry requires no Rust list or game-count changes; `basalt catalog`
shows the available games, and the Install page and artwork requests use that same catalog.
Local entries in `~/.config/basalt/games/*.json` are discovered at runtime and can add games or
override bundled entries without rebuilding Basalt.

A release can wrap `app/` in a folder named by `source.archive_root`, or ship `app/` at the zip
root. Entries can link covers for the Install page with `artwork.cover_url`. Entries in the
original `schema_version: 1` format keep working unchanged (see STANDARDS.md, section 11).

```sh
basalt catalog
basalt libraries list
basalt libraries add ssd /mnt/ssd/Games
basalt install MattMC --library ssd
basalt launch MattMC
basalt actions MattMC                # what `maintain` can run for this game
basalt maintain MattMC sync_up
basalt backups MyGame                # snapshots, newest first
basalt maintain MyGame restore --snapshot <id>
basalt maintain MattMC sync --verify # re-read every file instead of trusting cached hashes
basalt sync-remote                   # where standard games sync to (default /mnt/storage/Storage/Sync)
basalt sync-remote /media/usb/Sync   # use another folder; --clear goes back to the default
```

`basalt install <game>` prompts for a library in a terminal. Scripts must supply `--library <id>`;
`default` means `~/Games`. In the GUI, select a game on the Install page, choose an installation
folder, and press **Install / reinstall**. That page also lets you register additional libraries.
Both interfaces use the same catalog installer. A game's page in the Library shows its actions as
buttons; the sync folder (default `/mnt/storage/Storage/Sync`, one subfolder per game) is set under
Settings → Game Sync. Sync and backup are incremental: Basalt caches file hashes, keeps a file list
in the sync folder, copies only files that changed, and stores each file content once across backups.
MattMC's Distant Horizons databases are treated as cache, so they are not synced or backed up.
Catalog games show the artwork they ship in `app/basalt/` (cover, else icon), or the cover their
catalog entry links to before they are installed; see STANDARDS.md, "Artwork". Your own image in
`~/.local/share/basalt/artwork/` named after the game always wins.

Downloads and extraction happen in a staging directory inside the chosen library. The installer
validates the launch target, preserves executable permissions, and refuses unsafe archive paths.
For standard games, a release may only contain the game's `app` paths (optionally wrapped in one
folder named by the entry's `source.archive_root`), and on reinstall everything
else in the installation (saves, settings, anything uncategorized) carries over unchanged after a
backup. For `schema_version: 1` games, the release can seed `run/` on a fresh install, and on
reinstall the **entire existing `run/` wins**: worlds, settings, server data, shader packs, and the
existing bundled JDK are retained unchanged. In both cases application files are replaced, not
merged, so obsolete libraries do not linger. Reinstallation needs enough free space for staging plus
a copy of the preserved data. Close the game and any dedicated server before reinstalling.

Settings → Game Backups controls the maximum snapshots per game (default: **3**). Manual and
automatic safety backups share this limit; older snapshots are pruned when the next backup is
created. The value is stored as `backup.keep` in `~/.config/basalt/settings.json` and also applies
to CLI actions. A catalog entry can request fewer snapshots, but cannot exceed this limit.

An existing installation is retained until activation and registry persistence succeed, with
rollback on reported failures. A crash or power loss during activation is not automatically
recovered: any retained `.basalt-install-*/previous` directory is left for recovery. Basalt
currently registers one installation per game name; installing it in a different library requires
removing the old Basalt entry first (which does not delete its files).

For `schema_version: 1` games, maintenance remains game-owned: their entries' `maintenance` scripts
run attached to the terminal as before, and the GUI shows them except a prompting `sync`. Steam
support is unchanged.

MattMC installations from before it adopted the standard (the `run-mattmc.sh` layout) are still
discovered and launch as before, but Basalt's actions refuse to run on them, since their folder
layout is not the standard one. Reinstall MattMC from the Install page (or
`basalt install MattMC`) once a standard release is published: the reinstall takes over the old
registration and keeps every existing file, including `run/`, alongside the new `app/` and `data/`.
Moving worlds and settings from `run/` into `data/` is up to MattMC.

To try a catalog entry before it ships with Basalt, put it in `~/.config/basalt/games/`; a local
entry with the same id replaces the bundled one.

## Flatpak catalog games

SuperTuxKart, SuperTux, Chocolate Doom and Space Cadet Pinball are available in the regular catalog
as Flatpak games. Both the Install page and Library group these games under **Flatpak**. Install Flatpak and configure Flathub for your user:

```sh
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
basalt install SuperTuxKart
basalt launch SuperTuxKart
basalt maintain SuperTuxKart update
```

Basalt uses the user installation (`--user`); system-wide Flatpaks are not adopted. Flatpak owns
installation locations and shared runtimes, so omit `--library` for these games. Basalt does not add
remotes or install Flatpak itself. A missing Flatpak executable or remote is reported as an error.
`basalt discover --catalog` also registers matching user-installed catalog Flatpaks, checking the
application ID, branch and origin remote. Removing a Basalt entry leaves the Flatpak installed;
Discover can register it again.

Flatpak games currently offer **Update**, not the Basalt standard's backup, restore, sync or cache
actions. Their saves and settings use the application's Flatpak data paths rather than Basalt's
standard folder layout. Artwork supports the same catalog cover URLs and user overrides as other
games. Installation and updates report an indeterminate progress step and can be cancelled; if a
Flatpak transaction completed before cancellation or Basalt registration failed, use Discover to
register the installed game. Package changes are managed by Flatpak rather than Basalt's ZIP rollback.

## Design Principles

- Minimal UI and minimal configuration complexity.
- Predictable behavior over feature bloat.
- Clear, maintainable codebase with low overhead.

## Project Layout

Basalt is a Cargo workspace with three crates:

- `crates/basalt-core` — library with all launcher logic (library, discovery, launching, installation, maintenance). No UI dependencies.
- `crates/basalt-cli` — the `basalt` command-line tool. Running `basalt` with no command opens the GUI.
- `crates/basalt-gui-kirigami` — the `basalt-gui` desktop app, built with Qt Quick and KDE Kirigami. This is the default GUI.

Basalt stores its data in the platform's standard locations. On Linux these are `~/.local/share/basalt` (games, playlists, blacklist), `~/.config/basalt` (settings), and `~/.cache/basalt` (artwork). Files from the older `~/.basalt` directory are moved there automatically on first run.

## Kirigami GUI

The GUI (`basalt-gui`, crate `basalt-gui-kirigami`) uses [CXX-Qt](https://github.com/KDAB/cxx-qt) for the Rust/Qt bridge and KDE Kirigami for the interface. It needs Qt 6.5 or newer. It is a default workspace member, so plain `cargo run` starts it and `cargo build`/`cargo test` need Qt installed.

On Debian/Ubuntu, `bash DevUtils/Setup.sh` installs the Rust toolchain and every package needed to build, test and package Basalt. The package list lives in `DevUtils/apt-dependencies.txt` and CI installs from the same file (`DevUtils/InstallDependencies.sh` installs just the packages):

```
bash DevUtils/Setup.sh
cargo run
```

Because of that, QML mistakes are caught by `qmllint` instead of the compiler: run `bash DevUtils/LintQml.sh` (CI runs it too). It also checks the QML against the Rust `Backend`'s properties.

The QML is compiled by Qt at runtime rather than ahead of time: Qt's ahead-of-time QML compiler uses Qt's private ABI, which would tie the `.deb` to one exact Qt version. The package only needs minimum Qt versions, so distribution Qt updates don't break it.


The app uses KDE's `org.kde.desktop` controls style by default; set `QT_QUICK_CONTROLS_STYLE` (for example `Fusion`) to use a different one.

For checking the UI without touching the desktop or the network, the app has a screenshot tour: with `BASALT_SCREENSHOT_DIR` set it walks through every screen, saves a PNG of each, and quits. `bash DevUtils/SmokeTestGui.sh [screenshot-dir]` runs it offscreen against a throwaway home with test fixtures and fails on any QML error; CI runs it on every push and keeps the screenshots as an artifact.

The offscreen renderer does not draw every KDE control (progress bars, for one); use `QT_QPA_PLATFORM=xcb` to check those in a real window.

## Tests

- `cargo test --workspace` — unit tests, plus end-to-end tests that run the `basalt` CLI against throwaway homes (concurrent writes, `~/.basalt` migration, settings) and download tests against a local HTTP server.
- `bash DevUtils/LintQml.sh` — `qmllint` over the GUI's QML.
- `bash DevUtils/SmokeTestGui.sh` — the GUI smoke test.
- `python3 -m unittest discover -s tests` — the publish script.

## Publishing

`python3 DevUtils/PublishMattOSPackage.py publish` builds the `.deb` and uploads it to MattPackages. It refuses to run unless the `[workspace.package]` version in `Cargo.toml` is newer than every published version, because apt ignores a re-uploaded version.

To bump the version, commit and push, wait for every GitHub Actions workflow, and publish in one verified sequence, invoke the `release-deb` skill explicitly. Publishing only happens after CI is green.

GitHub releases are not used for distribution. The manual "Build Linux amd64 DEB" workflow builds the `.deb` in a clean Ubuntu container when you want one built off your machine. Dependabot proposes dependency and GitHub Actions updates weekly.

## Rust and Cargo Basics

This project uses Rust and Cargo.

- **Rust** is the programming language used for building Basalt.
- **Cargo** is Rust’s build system and package manager.

Common Cargo commands:

- `cargo run` — builds the project (if needed) and runs the GUI.
- `cargo run -p basalt-cli -- list` — runs a CLI command (here, `list`).
- `cargo test --workspace` — runs every crate's tests (plain `cargo test` covers the core and the GUI).
- `cargo build` — compiles the project without running it.
- `cargo build --release` — builds an optimized release binary.
- `cargo check` — quickly checks code for compile errors without a full build.
- `cargo test` — runs automated tests.

Useful Rust tools:

- `rustup` — installs and manages Rust toolchains.
- `rustc` — the Rust compiler.

## GitHub private releases (optional)

Basalt's existing `github_release` catalog entries work unchanged for both
public and private repositories. Public releases do **not** require a GitHub login.

For private releases, Basalt resolves credentials in this order:
1. `GH_TOKEN`, then `GITHUB_TOKEN` (use a read-only credential with access to the repository).
2. Native GitHub login, stored in the Linux Secret Service (KWallet/GNOME Keyring).
3. An existing `gh auth login` credential, if GitHub CLI happens to be installed.
4. Anonymous public release access.

GitHub CLI is **not** a required dependency. For native login, the Basalt
maintainer must register a GitHub **OAuth App** at
[GitHub Developer Settings](https://github.com/settings/developers), enable
**Device Flow**, The public Client ID `Ov23likSQpJPk474GXMn` is already built into Basalt as the default. An optional `BASALT_GITHUB_CLIENT_ID` environment variable can override it at build or run time. The OAuth App must
have permission to access private repositories via the `repo` scope. This
Client ID is public and is not a secret.

Install the `libsecret-tools` package (providing `secret-tool`) and configure
a Secret Service provider such as KWallet. Then select **Sign in with GitHub**
in Basalt's Settings, visit the GitHub URL displayed, and enter its one-time
verification code. Alternatively run `basalt github login`. Use
`basalt github status` or `basalt github logout` to inspect or remove
saved native credentials.

OAuth credentials are never stored in game manifests or settings JSON.
Basalt automatically refreshes expiring OAuth access tokens when needed,
persisting the replacement access and refresh tokens in the system keyring.
If a refresh token expires after an extended period without use, sign in again.
Refreshing only requires the public Client ID for Device Flow credentials.
Already installed games remain playable if authentication is unavailable.

For CI/headless installations, use a GitHub read-only access token in an
environment variable; a desktop keyring is unnecessary.

## AGENTS

`AGENTS.md` is a symbolic link to this README; the README is the single source
of truth for agent instructions.

- Read this document, [STANDARDS.md](STANDARDS.md), and the linked coding
  standards before changing the project.
- Keep game-specific behavior out of the launcher. Installation, layout,
  actions, and artwork belong in `resources/games/*.json` and are governed by
  `STANDARDS.md`.
- Keep platform-specific calls behind `basalt-core`'s platform modules. The
  `clippy.toml` boundary lints are enforced in CI and must stay green.
- Never modify another project or repository as part of work here.
- Preserve user data: installation and update code must leave `data/` and
  game-owned paths untouched unless the catalog explicitly categorizes them.
- Run the checks in [Tests](#tests) before finishing. At minimum:
  `cargo clippy --locked --workspace --all-targets -- -D clippy::disallowed_methods -D clippy::disallowed_types`,
  `cargo test --locked --workspace`,
  `bash DevUtils/LintQml.sh`,
  `bash DevUtils/SmokeTestGui.sh`, and
  `python3 -m unittest discover -s tests`. `cargo fmt --all -- --check` is
  advisory until the existing formatting drift is cleaned up; do not mix a
  repo-wide reformat into an unrelated change.
- To commit, push, bump the version, verify every GitHub Actions workflow, and
  publish the `.deb` to MattPackages, invoke the `release-deb` skill explicitly.
  Do not run that workflow unless the user asks for a release.
- Update this README and the relevant skill when a workflow or release step
  changes.
