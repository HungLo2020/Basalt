# Basalt

Basalt is a minimal, focused game launcher for Linux.

## Install

On a system with the MattPackages apt repository configured:

`sudo apt install basalt`

Basalt is then updated with the rest of the system (`sudo apt upgrade`); it has no built-in updater.

Without the repository, `Install.sh` installs the latest `.deb` from GitHub releases (or builds one locally), but that install will not update itself:

`curl -fsSL https://raw.githubusercontent.com/HungLo2020/Basalt/main/Install.sh | bash`

## Project Goals

- Keep the launcher simple and fast.
- Prioritize Linux as the primary platform.
- Minimize dependencies and keep setup lightweight.
- Support launching Steam games.
- Support launching GOG games.
- Support launching games through custom scripts or standalone executables.
- Integrate with MattMC (a Minecraft fork) to download, install, and launch it.
- Provide a robust CLI for scripting and automation workflows.

## MattMC Integration Goal

MattMC is fully independent and can run without a launcher, but Basalt is designed to provide a unified flow to manage and launch it alongside other games.

## Design Principles

- Minimal UI and minimal configuration complexity.
- Predictable behavior over feature bloat.
- Clear, maintainable codebase with low overhead.

## Project Layout

Basalt is a Cargo workspace with three crates:

- `crates/basalt-core` — library with all launcher logic (library, discovery, launching, emulation, sync). No UI dependencies.
- `crates/basalt-cli` — the `basalt` command-line tool. Running `basalt` with no command opens the GUI.
- `crates/basalt-gui-kirigami` — the `basalt-gui` desktop app, built with Qt Quick and KDE Kirigami. This is the default GUI.

Basalt stores its data in the platform's standard locations. On Linux these are `~/.local/share/basalt` (games, playlists, blacklist), `~/.config/basalt` (settings), and `~/.cache/basalt` (artwork). Files from the older `~/.basalt` directory are moved there automatically on first run.

## Kirigami GUI

The GUI (`basalt-gui`, crate `basalt-gui-kirigami`) uses [CXX-Qt](https://github.com/KDAB/cxx-qt) for the Rust/Qt bridge and KDE Kirigami for the interface. It needs Qt 6.5 or newer. It is a default workspace member, so plain `cargo run` starts it and `cargo build`/`cargo test` need Qt installed.

Linux (Debian/Ubuntu package names):

```
sudo apt install qt6-base-dev qt6-declarative-dev qml6-module-org-kde-kirigami qml6-module-org-kde-desktop
cargo run
```

Because of that, QML mistakes are caught by `qmllint` instead of the compiler: run `bash DevUtils/LintQml.sh` (CI runs it too). It also checks the QML against the Rust `Backend`'s properties.

The QML is compiled by Qt at runtime rather than ahead of time: Qt's ahead-of-time QML compiler uses Qt's private ABI, which would tie the `.deb` to one exact Qt version. The package only needs minimum Qt versions, so distribution Qt updates don't break it.


The app uses KDE's `org.kde.desktop` controls style by default; set `QT_QUICK_CONTROLS_STYLE` (for example `Fusion`) to use a different one.

For checking the UI without touching the desktop or the network, the app has a screenshot tour: with `BASALT_SCREENSHOT_DIR` set it walks through every screen, saves a PNG of each, and quits. Run it against a throwaway home, offscreen and without network access:

```
unshare -rn env HOME=/tmp/basalt-test QT_QPA_PLATFORM=offscreen \
  BASALT_SCREENSHOT_DIR=/tmp/basalt-shots target/debug/basalt-gui
```

The offscreen renderer does not draw every KDE control (progress bars, for one); use `QT_QPA_PLATFORM=xcb` to check those in a real window.

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
