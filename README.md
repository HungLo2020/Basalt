# Basalt

Basalt is a minimal, focused game launcher intended to be reliable on Linux first, with cross-platform support for macOS and Windows.

## Install

Run this on Linux to download only `Install.sh` from this repository and execute it:

`curl -fsSL https://raw.githubusercontent.com/HungLo2020/Basalt/main/Install.sh | bash`

## Project Goals

- Keep the launcher simple and fast.
- Prioritize Linux as the primary platform.
- Maintain compatibility with macOS and Windows.
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
- `crates/basalt-gui` — the `basalt-gui` desktop app.

Basalt stores its data in the platform's standard locations. On Linux these are `~/.local/share/basalt` (games, playlists, blacklist), `~/.config/basalt` (settings), and `~/.cache/basalt` (artwork). Files from the older `~/.basalt` directory are moved there automatically on first run.

## Rust and Cargo Basics

This project uses Rust and Cargo.

- **Rust** is the programming language used for building Basalt.
- **Cargo** is Rust’s build system and package manager.

Common Cargo commands:

- `cargo run --bin basalt-gui` — builds the project (if needed) and runs the GUI.
- `cargo run --bin basalt -- list` — runs a CLI command (here, `list`).
- `cargo build` — compiles the project without running it.
- `cargo build --release` — builds an optimized release binary.
- `cargo check` — quickly checks code for compile errors without a full build.
- `cargo test` — runs automated tests.

Useful Rust tools:

- `rustup` — installs and manages Rust toolchains.
- `rustc` — the Rust compiler.
