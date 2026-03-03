# Basalt

Basalt is a minimal, clean game launcher intended to be reliable on Linux first, with cross-platform support for macOS and Windows.

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

## Rust and Cargo Basics

This project uses Rust and Cargo.

- **Rust** is the programming language used for building Basalt.
- **Cargo** is Rust’s build system and package manager.

Common Cargo commands:

- `cargo run` — builds the project (if needed) and runs the main binary.
- `cargo build` — compiles the project without running it.
- `cargo build --release` — builds an optimized release binary.
- `cargo check` — quickly checks code for compile errors without a full build.
- `cargo test` — runs automated tests.

Useful Rust tools:

- `rustup` — installs and manages Rust toolchains.
- `rustc` — the Rust compiler.
