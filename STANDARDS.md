# Basalt Game Standard

**Standard version: 1** · Status: draft. This is the target contract; Basalt is being updated to
implement it (see [Implementation status](#implementation-status)).

This document defines what a game must do to be installed, launched, updated, backed up and synced
by Basalt **without any game-specific handling**. A game that follows it needs only a short catalog
entry in this repository. Games with unusual needs can override any default in their catalog entry,
including replacing a built-in action with their own script.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119.

## Contents

1. [Install directory layout](#1-install-directory-layout)
2. [Data categories](#2-data-categories)
3. [Release packaging](#3-release-packaging)
4. [Launch environment](#4-launch-environment)
5. [Built-in actions](#5-built-in-actions)
6. [Catalog entry](#6-catalog-entry)
7. [Overriding defaults](#7-overriding-defaults)
8. [Custom scripts](#8-custom-scripts)
9. [Compliance checklist](#9-compliance-checklist)

---

## 1. Install directory layout

Basalt installs every game into its own directory inside a **game library**. The default library is
`~/Games`, but users can add any number of libraries (for example `/mnt/ssd/Games`), so a game's
install root can be anywhere:

```
<library>/<GameId>/            ← install root (any library, any path)
├── app/                       ← owned by the release; replaced on every update
│   ├── bin/<executable>
│   ├── defaults/              ← optional: default config copied on first run
│   └── ...                    ← assets, libraries, bundled runtime (JRE etc.)
└── data/                      ← owned by the user; never touched by install or update
    ├── save/                  ← progress and user content: synced, backed up
    ├── config/                ← machine-specific settings: backed up, not synced
    │   └── shared/            ← settings that should follow the user: synced, backed up
    └── cache/                 ← rebuildable: neither synced nor backed up
```

Rules:

- A game MUST NOT assume a particular library or absolute install path. Everything is relative to the
  install root or taken from the [environment variables](#4-launch-environment) Basalt provides.
- A game MUST NOT write to `app/` at runtime. Basalt may make it read-only.
- A game MUST NOT store data anywhere outside its install root (no `~/.mygame`, no `~/.config/mygame`),
  so that the install is self-contained and moving or deleting it is complete.
- Files Basalt itself manages are kept under `<install root>/.basalt/`. Games MUST NOT touch it.

## 2. Data categories

Where a file lives decides how Basalt treats it:

| Location | Contains | Install / update | Sync | Backup |
|---|---|---|---|---|
| `app/` | Everything shipped in the release | Replaced entirely | No | No |
| `data/save/` | Saves, worlds, profiles, unlocks, user-added content (mods, resource packs, levels) | Never modified | **Yes** | Yes |
| `data/config/shared/` | Settings that should follow the user: keybinds, gameplay options, accessibility | Never modified | **Yes** | Yes |
| `data/config/` (rest) | Machine-specific settings: resolution, display, audio device, graphics quality | Never modified | No | Yes |
| `data/cache/` | Shader caches, logs, crash dumps, downloaded or generated assets, temp files | May be deleted at any time | No | No |

Choosing a category:

- **Would losing it upset the player?** → `save/`.
- **Would it be wrong on a different computer?** → `config/`. Otherwise, if it's a preference → `config/shared/`.
- **Can the game rebuild it?** → `cache/`.
- **Did it come in the release?** → `app/`.

### Defaults and first run

Because releases never ship anything into `data/`, update never has to decide whether to overwrite a
file. A game that needs default settings MUST ship them in `app/` (for example `app/defaults/`) and,
at startup, copy any **missing** file into `data/config/`. Existing files MUST be left alone. The game
MUST start correctly when `data/` is empty or absent, creating the directories it needs.

### Save data rules

So that saves survive syncing between machines, crashes and updates:

- Save files MUST be written atomically (write to a temporary file in the same directory, then rename).
- `save/` and `config/shared/` MUST NOT contain absolute paths, machine identifiers, lock files, or
  anything else that is only valid on one computer.
- A game MUST read saves written by older versions of itself (migrating them if needed). Downgrades are
  not supported; Basalt takes a backup before every update instead.
- A game SHOULD tolerate its save directory being replaced while it is not running.

## 3. Release packaging

- Releases MUST be published as GitHub Release assets: a `.zip` archive per platform.
- Asset names MUST be predictable so the catalog can match them, e.g. `Jackal-1.4.0-linux-x64.zip`.
  The catalog entry gives the prefix, suffix and any required substrings (such as `linux-x64`).
- The archive root MUST contain only `app/`. It MUST NOT contain `data/` or `.basalt/`.
- The release MUST be self-contained: bundle any runtime it needs (JRE, Python, shared libraries) inside
  `app/`. It MUST NOT rely on packages beyond a standard desktop Linux install.
- Executables MUST have their execute bit set in the archive.
- A game MUST NOT include its own updater. Basalt owns installation and updates.

## 4. Launch environment

Basalt launches the configured executable with the working directory set to the install root (unless
the catalog entry overrides it) and these environment variables set:

| Variable | Value |
|---|---|
| `BASALT_GAME_ID` | The catalog id, e.g. `Jackal` |
| `BASALT_GAME_DIR` | Absolute install root |
| `BASALT_APP_DIR` | `<install root>/app` |
| `BASALT_SAVE_DIR` | `<install root>/data/save` |
| `BASALT_CONFIG_DIR` | `<install root>/data/config` |
| `BASALT_SHARED_CONFIG_DIR` | `<install root>/data/config/shared` |
| `BASALT_CACHE_DIR` | `<install root>/data/cache` |
| `BASALT_STANDARD_VERSION` | Version of this standard Basalt implements, e.g. `1` |

Rules:

- A game SHOULD read its data locations from these variables, and MUST fall back to the same paths
  relative to the install root when they are unset, so it still runs outside Basalt.
  (If a game remaps its layout in the catalog, Basalt sets the variables to the remapped paths.)
- A game MUST start without command-line arguments or user interaction in a terminal.
- A game MUST NOT require network access to start.
- A game SHOULD exit with status 0 on a normal quit and non-zero on a crash.
- A game SHOULD support controller input; Basalt is used from a couch with a gamepad.

## 5. Built-in actions

Basalt provides these actions for every game. The GUI shows them as buttons on the game's page and the
CLI runs them with `basalt maintain <game> <action>`. A catalog entry can disable, configure or replace
each one ([section 7](#7-overriding-defaults)), and can add its own actions ([section 8](#8-custom-scripts)).
Every action except `clear_cache` refuses to run while the game is running.

| Action | Default behavior |
|---|---|
| `install` / `update` | Download the latest matching release, unpack it to a staging directory, validate it, back up `data/`, replace `app/` and leave `data/` untouched. On failure the previous `app/` is restored. |
| `backup` | Copy the backed-up sets (`save/`, `config/`) into Basalt's backup store as a timestamped snapshot. Keeps the newest 10 snapshots per game. |
| `restore` | Replace the backed-up sets with a chosen snapshot, after first taking a backup of the current state. |
| `sync_up` | Make the remote copy of the synced sets match this machine. The remote copy being replaced is kept as a backup. |
| `sync_down` | Make this machine's synced sets match the remote copy. The local copy is backed up first. |
| `sync` | Sync in whichever direction changed since the last sync. If both sides changed, stop and ask the user (GUI) or fail with an explanation (CLI) instead of guessing. |
| `clear_cache` | Delete the contents of `data/cache/`. |

**Synced sets** are `save/` and `config/shared/`. **Backed-up sets** are `save/` and all of `config/`.
Each set is synced as a single unit, never merged file by file.

The sync remote is configured once by the user in Basalt's settings; games never see it. Each game's
data lives at `<remote>/<GameId>/` on the remote, so the remote layout is independent of which library
the game is installed in on each machine. Backups are stored in Basalt's data directory
(`~/.local/share/basalt/backups/<GameId>/` on Linux), outside the install root, so they survive
uninstalling or reinstalling the game.

## 6. Catalog entry

Each game has one JSON file in [`resources/games/`](resources/games/). A compliant game needs only:

```json
{
  "schema_version": 2,
  "standard_version": 1,
  "id": "Jackal",
  "name": "Jackal",
  "source": {
    "kind": "github_release",
    "repository": "HungLo2020/Jackal",
    "asset_prefix": "Jackal-",
    "asset_suffix": ".zip",
    "asset_contains": ["linux-x64"]
  },
  "launch": {
    "executable": "app/bin/jackal"
  }
}
```

| Field | Required | Meaning |
|---|---|---|
| `schema_version` | Yes | Version of the catalog file format. `2` for entries written against this standard. |
| `standard_version` | Yes | Version of this standard the game complies with. |
| `id` | Yes | Stable identifier and install directory name. Letters, digits, `-` and `_` only. Never changes. |
| `name` | Yes | Display name. |
| `source` | Yes | Where releases come from (see [section 3](#3-release-packaging)). |
| `launch.executable` | Yes | Path relative to the install root. |
| `launch.arguments` | No | Fixed arguments, passed directly with no shell interpretation. Default `[]`. |
| `launch.working_directory` | No | Relative to the install root. Default `.` (the install root). |
| `layout` | No | Remaps data categories ([section 7](#7-overriding-defaults)). |
| `actions` | No | Configures, disables, replaces or adds actions ([section 7](#7-overriding-defaults)). |

All paths in a catalog entry are relative to the install root. Paths MUST NOT be absolute and MUST NOT
contain `..`. Catalog entries never contain shell code; they can only point at scripts that ship in
the game's own release.

## 7. Overriding defaults

Everything in sections 2 and 5 is a default. A catalog entry overrides only what it needs; anything it
doesn't mention keeps the default.

### Remapping the layout

`layout` assigns paths to each category. Each value is a list of paths relative to the install root,
and replaces that category's default entirely:

```json
"layout": {
  "app":           ["app"],
  "save":          ["data/save"],
  "shared_config": ["data/config/shared"],
  "config":        ["data/config"],
  "cache":         ["data/cache"]
}
```

Any path not covered by `app` is treated as user data and is preserved on update, even if it is in no
category. When paths nest, the most specific one wins (that is how `config/shared/` is synced inside an
unsynced `config/`). This is how a game that predates the standard can be supported without moving its
files, for example:

```json
"layout": {
  "app":    ["app", "run-mygame.sh"],
  "save":   ["run/saves", "run/mods"],
  "config": ["run/options.txt"],
  "cache":  ["run/logs", "run/crash-reports"]
}
```

### Configuring, disabling or replacing actions

Each key under `actions` is an action name. Its value is one of:

- `false` — disable the action. It is hidden in the GUI and rejected by the CLI.
- `{ "options": { ... } }` — keep the built-in action with different options.
- `{ "script": { ... } }` — replace the built-in action with a script ([section 8](#8-custom-scripts)).

Any action object MAY also set `"label"` to change the button text shown in the GUI.

```json
"actions": {
  "backup":      { "options": { "keep": 20 } },
  "sync":        false,
  "update":      { "script": { "path": "app/scripts/update.sh" } },
  "clear_cache": { "label": "Clear shader cache" }
}
```

Built-in action options:

| Action | Option | Default | Meaning |
|---|---|---|---|
| `backup` | `keep` | `10` | Number of snapshots kept per game. |
| `backup` | `include` | `["save", "config"]` | Categories included in a snapshot. |
| `sync`, `sync_up`, `sync_down` | `include` | `["save", "shared_config"]` | Categories that are synced. |
| `update` | `backup_first` | `true` | Take a backup before replacing `app/`. |

## 8. Custom scripts

A script replaces a built-in action, or adds a new action under any other name (shown as an extra
button in the GUI and available as `basalt maintain <game> <name>` in the CLI). New actions MUST use a
script.

```json
"actions": {
  "sync_up":   { "script": { "path": "SyncGameData.sh", "stdin": "up\n" } },
  "sync_down": { "script": { "path": "SyncGameData.sh", "stdin": "down\n" } },
  "repair":    { "label": "Repair install", "script": { "path": "app/scripts/repair.sh", "arguments": ["--full"] } }
}
```

| Field | Required | Meaning |
|---|---|---|
| `path` | Yes | Script path relative to the install root. It must ship in the release. |
| `arguments` | No | Arguments passed directly, with no shell interpretation. |
| `stdin` | No | Text written to the script's standard input, for scripts that prompt. |

Script contract:

- Basalt runs the script with the working directory set to the install root and the same environment
  variables as a launch ([section 4](#4-launch-environment)), plus `BASALT_ACTION` (the action name) and
  `BASALT_BACKUP_DIR` (this game's backup store).
- Scripts MUST use those variables or relative paths and MUST NOT assume a library or absolute path.
- Scripts MUST NOT wait for input from a terminal beyond what `stdin` supplies.
- Scripts MUST exit with status 0 on success and non-zero on failure, writing a short, human-readable
  reason to standard error; Basalt shows it to the user.
- Scripts SHOULD print progress lines to standard output; Basalt shows the latest one as the job status.
- Scripts SHOULD be safe to run again after a failure.

Scripts are an escape hatch. Before writing one, check whether a `layout` or an `options` override does
the job: built-in actions get Basalt's backups, conflict handling and running-game checks for free.

## 9. Compliance checklist

Copy this into your game's repository and tick it off. A game that meets every item needs only the
minimal catalog entry in [section 6](#6-catalog-entry).

**Layout and data**
- [ ] The release archive contains only `app/` at its root.
- [ ] The game never writes to `app/` at runtime.
- [ ] Progress and user content are stored in `data/save/` (or `$BASALT_SAVE_DIR`).
- [ ] Machine-specific settings are in `data/config/`; settings that should follow the player are in `data/config/shared/`.
- [ ] Rebuildable files (caches, logs, crash dumps) are in `data/cache/`.
- [ ] Nothing is stored outside the install root.
- [ ] The game starts correctly with an empty or missing `data/`, copying defaults from `app/` only when missing.

**Robustness**
- [ ] Save files are written atomically (temp file + rename).
- [ ] Synced data contains no absolute paths, machine IDs or lock files.
- [ ] Saves from older versions load (and are migrated if needed).
- [ ] The game works from any install path, using `BASALT_*` variables with relative-path fallbacks.

**Packaging and launch**
- [ ] Releases are GitHub Release `.zip` assets with predictable, platform-tagged names.
- [ ] The release bundles every runtime it needs, with execute bits set.
- [ ] There is no built-in updater.
- [ ] The game starts without arguments, terminal input or network access.
- [ ] The game exits with 0 on a normal quit.

**Catalog**
- [ ] A catalog entry with `standard_version: 1` exists in `resources/games/`.
- [ ] Any `layout` or `actions` overrides are documented in the game's repository with the reason for them.

---

## Implementation status

Basalt does not implement all of this standard yet. Today it supports catalog `schema_version: 1`:
installs into any library, preserves a single `persistent_directory` (default `run/`) on update, and
runs per-game `maintenance` scripts. Catalog entries using `schema_version: 1` keep working and are
treated as: `persistent_directory` is preserved on update; no built-in sync or backup; each
`maintenance` command is a script action.

Still to be built: the `BASALT_*` launch environment, `layout` and `actions` in the catalog schema,
built-in `backup`, `restore`, `sync`, `sync_up`, `sync_down` and `clear_cache`, the sync remote setting,
and the backup store. Existing games (MattMC, MicroMages, Jackal) need to move to the `app/` + `data/`
layout or declare a `layout` override.
