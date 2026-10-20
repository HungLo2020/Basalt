# Basalt Game Standard

**Standard version: 1** · Implemented by Basalt (catalog `schema_version` 2).

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
10. [Trying an entry locally](#10-trying-an-entry-locally)
11. [Legacy catalog entries](#11-legacy-catalog-entries)

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
- Files Basalt itself manages are kept under `<install root>/.basalt/` (sync state, a marker while
  the game is running, and a lock held during updates, syncs, backups and restores). Games MUST NOT
  touch it. Basalt also writes `<install root>/.basalt-game.json`, the installed copy of the catalog
  entry.

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
- The archive root MUST contain only `app/` (or, with a `layout` override, only the `app` paths),
  **or** exactly one directory that itself contains only `app/`, if the catalog entry names that
  directory in `source.archive_root`. The wrapper suits games whose zip should stay self-contained
  when unzipped by hand. Either way, the archive MUST NOT contain `data/` or `.basalt/`, at the root
  or inside the wrapper.
- Basalt checks this before touching an installation and refuses a release that does not match:
  a missing or differently named wrapper, extra top-level entries, a wrapper holding more than
  `app/`, or a wrapper the catalog entry does not declare. A release can therefore never overwrite
  user data.

  ```
  Jackal-1.4.0-linux-x64.zip          MattMC-Client-2.0-linux-x64.zip  ("archive_root": "MattMC")
  └── app/                            └── MattMC/
      └── bin/jackal                      └── app/
                                              └── bin/mattmc
  ```
- The release MUST be self-contained: bundle any runtime it needs (JRE, Python, shared libraries) inside
  `app/`. It MUST NOT rely on packages beyond a standard desktop Linux install.
- Executables MUST have their execute bit set in the archive.
- A game MUST NOT include its own updater. Basalt owns installation and updates.

### Artwork

Basalt shows every game as a portrait tile, and the same picture on the game's details page. A game
provides that picture by shipping a **cover**, and optionally an **icon**, in its release:

```
app/
└── basalt/
    ├── cover.png      (or cover.jpg)   portrait box art, 3:4
    └── icon.png       (or icon.svg)    square emblem
```

| File | Required | What it is | Format |
|---|---|---|---|
| `app/basalt/cover.png` or `.jpg` | SHOULD | Box art that fills the tile: key art and the game's title, readable at tile size. | Exactly 3:4 portrait; **900×1200** recommended, at least 600×800. PNG or JPEG, opaque, under 2 MB. |
| `app/basalt/icon.png` or `.svg` | MAY | A simple emblem that still reads at 32×32: no small text. | Square; **512×512** recommended, at least 256×256. PNG with transparency, or SVG. |

- A game SHOULD ship a cover. A cover in another shape still shows, scaled to fit with empty
  bands, so 3:4 matters; Steam-style 2:3 art shows thin bands at the top and bottom.
- The icon is a fallback: Basalt shows it centered in the tile when there is no cover, and may use it
  where a small square image fits (window, shortcuts, compact lists). A game with neither shows a
  generic placeholder.
- Artwork lives in `app/`, so it is replaced on update like any other release file and is never
  synced or backed up. If both `.png` and the alternative exist, Basalt uses the `.png`.
- Before a game is installed, Basalt cannot see its `app/`. To show the cover on the Install page,
  the catalog entry MAY name a copy with `artwork.cover_url` ([section 6](#6-catalog-entry)),
  normally the same file in the game's repository. It SHOULD match the shipped cover.
- An image the user puts in Basalt's artwork folder (`~/.local/share/basalt/artwork/<Game name>.png`
  or `.jpg`) always wins over the game's own artwork.
- In full, Basalt shows the first of: the user's image, the shipped cover, the cover from
  `artwork.cover_url` (downloaded once and cached), the shipped icon, an icon bundled with Basalt
  (MattMC only), and finally the placeholder. Files that are not readable images are skipped.

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
  (If a game remaps its layout in the catalog, each variable points at the first path listed for
  that category.)
- A game MUST start without command-line arguments or user interaction in a terminal.
- A game MUST NOT require network access to start.
- A game SHOULD exit with status 0 on a normal quit and non-zero on a crash.
- A game SHOULD support controller input; Basalt is used from a couch with a gamepad.

## 5. Built-in actions

Basalt provides these actions for every game. The GUI shows them as buttons on the game's page and the
CLI runs them with `basalt maintain <game> <action>`. A catalog entry can disable, configure or replace
each one ([section 7](#7-overriding-defaults)), and can add its own actions ([section 8](#8-custom-scripts)).
Every action except `clear_cache` refuses to run while the game is running (Basalt records a marker
while it has the game open), and only one action runs per game at a time. Basalt also refuses to
launch a game while one of its actions is running.

| Action | Default behavior |
|---|---|
| `update` | Download the latest matching release, unpack it to a staging directory, validate it, back up user data, replace the `app` paths and leave everything else untouched. On failure the previous installation is restored. Reinstalling from the Install page or `basalt install` does the same. |
| `backup` | Copy the backed-up sets (`save/`, `config/`) into Basalt's backup store as a timestamped snapshot. Keeps up to the user's maximum per game (default: 3). |
| `restore` | Replace the snapshot's sets with its contents, after first taking a backup of the current state. The GUI asks which snapshot; the CLI takes `--snapshot <id>` (list them with `basalt backups <game>`). |
| `sync_up` | Make the remote copy of the synced sets match this machine. The remote copy being replaced is kept as a backup. |
| `sync_down` | Make this machine's synced sets match the remote copy. The local copy is backed up first. |
| `sync` | Sync in whichever direction changed since this machine last synced. If both sides changed, stop and ask the user (GUI) or fail with an explanation (CLI) instead of guessing. On a machine that has never synced, an empty side takes the other side's data (so a fresh install picks up your saves); if both sides have different data, it asks. |
| `clear_cache` | Delete the contents of `data/cache/`. |

**Synced sets** are `save/` and `config/shared/`. **Backed-up sets** are `save/` and all of `config/`
(which includes `config/shared/`). Which way to sync, and whether there is a conflict, is decided for
the synced sets as a whole; changes are never merged file by file. Basalt compares contents (file
hashes), not timestamps, so copies on different machines and file systems compare reliably, and it
then copies only the files that differ, several at a time.

To stay fast, Basalt remembers each local file's hash with its size and modification time and only
re-reads files whose size or time changed, and it keeps a list of the synced files and their hashes
in the sync folder (`.basalt-sync.json`). An unchanged sync therefore reads one small file from the
sync folder. The list is marked incomplete while files are being copied; if it is missing or
incomplete (for example after an interrupted sync), Basalt reads the folder itself and rewrites it.
`basalt maintain <game> <sync action> --verify` re-reads every file on both sides.

The **sync folder** is chosen by the user (Settings → Game Sync, or `basalt sync-remote <folder>`) and
defaults to `/mnt/storage/Storage/Sync`; games never see it. It can be any folder: a local disk, or a
mounted network share or cloud folder. Basalt refuses to sync when the folder is missing (for
example, an unmounted share) rather than creating it; the per-game folders inside it are created as
needed. Each game's data lives at `<sync folder>/<GameId>/`, with the same relative paths as in
the install root, so it does not matter which library the game is installed in on each machine.
Files in that folder outside the synced sets are left alone.

Backups are stored in Basalt's data directory (`~/.local/share/basalt/backups/<GameId>/` on Linux),
outside the install root, so they survive uninstalling or reinstalling the game. Each distinct file
content is stored once and shared by every snapshot that contains it, so a backup only costs what
changed since an earlier one, and pruning removes content no remaining snapshot uses. Basalt takes one
automatically before an update, a `sync_down`, a `restore`, and (of the sync folder's copy) before a
`sync_up` that replaces different data. Automatic backups use the `backup` action's options even when
that action is disabled or replaced by a script.

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
| `source` | Yes | Where releases come from (see [section 3](#3-release-packaging)): `kind` (`github_release`), `repository` (`owner/name`), `asset_prefix`, `asset_suffix` (`.zip`), and optionally `asset_contains` (substrings the asset name must contain). |
| `source.archive_root` | No | The single folder the release zip wraps its files in. Default: none (`app/` is at the archive root). A single directory name: letters, digits, `-`, `_` and `.`; no `/`, no `..`, not absolute. When set, Basalt installs the contents of that folder as the install root. |
| `launch.executable` | Yes | Path relative to the install root. |
| `launch.arguments` | No | Fixed arguments, passed directly with no shell interpretation. Default `[]`. |
| `launch.working_directory` | No | Relative to the install root. Default `.` (the install root). |
| `artwork.cover_url` | No | HTTPS URL of the game's cover (see [Artwork](#artwork)), for showing it before the game is installed. Basalt downloads and caches it. |
| `layout` | No | Remaps data categories ([section 7](#7-overriding-defaults)). |
| `actions` | No | Configures, disables, replaces or adds actions ([section 7](#7-overriding-defaults)). |

A game whose zip wraps its files in a folder names it in `archive_root`. MattMC's entry, for example:

```json
{
  "schema_version": 2,
  "standard_version": 1,
  "id": "MattMC",
  "name": "MattMC",
  "source": {
    "kind": "github_release",
    "repository": "HungLo2020/MattMC",
    "asset_prefix": "MattMC-Client-",
    "asset_suffix": ".zip",
    "asset_contains": ["linux-x64"],
    "archive_root": "MattMC"
  },
  "launch": { "executable": "app/bin/mattmc" },
  "layout": {
    "cache": ["data/cache", "data/save/**/DistantHorizons.sqlite*"]
  }
}
```

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

Paths outside `app` may use wildcards: `*` matches any characters within one file or folder name,
and `**` (a whole path component) matches any number of folders. A wildcard path counts as more
specific than a shorter literal one by its number of literal components, so it can carve files out
of a category. MattMC uses this to treat Distant Horizons' level-of-detail databases, which the mod
rebuilds and which change every time you play, as cache inside its saves:

```json
"layout": {
  "cache": ["data/cache", "data/save/**/DistantHorizons.sqlite*"]
}
```

Wildcards are not allowed in `app` paths or in a path's first component. Directories only matched
by a pattern are not created on install, and the `BASALT_*_DIR` variables point at the first
literal path of each category.

### Configuring, disabling or replacing actions

Each key under `actions` is an action name. Its value is one of:

- `false` — disable the action. It is hidden in the GUI and rejected by the CLI.
- `{ "options": { ... } }` — keep the built-in action with different options.
- `{ "script": { ... } }` — replace the built-in action with a script ([section 8](#8-custom-scripts)).

Any action object MAY also set `"label"` to change the button text shown in the GUI.

```json
"actions": {
  "backup":      { "options": { "keep": 2 } },
  "sync":        false,
  "update":      { "script": { "path": "app/scripts/update.sh" } },
  "clear_cache": { "label": "Clear shader cache" }
}
```

Listing `config` in an `include` also includes `shared_config`. Options a built-in action does not
take, and unknown fields anywhere in `actions` or `layout`, are rejected so that typos fail loudly.

Built-in action options:

| Action | Option | Default | Meaning |
|---|---|---|---|
| `backup` | `keep` | User's limit (initially `3`) | Number of snapshots kept per game, capped by the user's retention setting. |
| `backup` | `include` | `["save", "config"]` | Categories included in a snapshot (`save`, `shared_config`, `config`). |
| `sync`, `sync_up`, `sync_down` | `include` | `["save", "shared_config"]` | Categories that are synced (`save`, `shared_config`, `config`). |
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
| `path` | Yes | Script path relative to the install root. It must ship in the release. `.sh` files run with bash; anything else is executed directly and needs its execute bit. |
| `arguments` | No | Arguments passed directly, with no shell interpretation. |
| `stdin` | No | Text written to the script's standard input, for scripts that prompt. Without it, standard input is empty. |

Script contract:

- Basalt runs the script with the working directory set to the install root and the same environment
  variables as a launch ([section 4](#4-launch-environment)), plus `BASALT_ACTION` (the action name) and
  `BASALT_BACKUP_DIR` (this game's backup store).
- Scripts MUST use those variables or relative paths and MUST NOT assume a library or absolute path.
- Scripts MUST NOT wait for input from a terminal beyond what `stdin` supplies.
- Scripts MUST exit with status 0 on success and non-zero on failure, writing a short, human-readable
  reason to standard error; Basalt shows it to the user.
- Scripts SHOULD print progress lines to standard output; Basalt shows the latest one as the job status.
- Scripts SHOULD be safe to run again after a failure. Cancelling the action stops the script.

Scripts are an escape hatch. Before writing one, check whether a `layout` or an `options` override does
the job: built-in actions get Basalt's backups, conflict handling and running-game checks for free.

## 9. Compliance checklist

Copy this into your game's repository and tick it off. A game that meets every item needs only the
minimal catalog entry in [section 6](#6-catalog-entry).

**Layout and data**
- [ ] The release archive contains only `app/` at its root, or only one wrapper folder containing only
      `app/`, named in the catalog entry's `source.archive_root`. Neither contains `data/` or `.basalt/`.
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

**Artwork**
- [ ] The release ships `app/basalt/cover.png` (or `.jpg`): 3:4 portrait, 900×1200 recommended, under 2 MB.
- [ ] Optionally, `app/basalt/icon.png` (512×512, transparent) or `app/basalt/icon.svg`.
- [ ] Optionally, the catalog entry's `artwork.cover_url` points at the same cover.

**Catalog**
- [ ] A catalog entry with `schema_version: 2` and `standard_version: 1` exists in `resources/games/`,
      and was tried locally first ([section 10](#10-trying-an-entry-locally)).
- [ ] Any `layout` or `actions` overrides are documented in the game's repository with the reason for them.

---

## 10. Trying an entry locally

Basalt also reads catalog entries from `~/.config/basalt/games/*.json`. A local entry with the same
`id` as a bundled one replaces it, and an invalid file is skipped with a warning. Use this to try a
game's entry, including `layout` and `actions` overrides, before adding it to this repository:

```sh
cp mygame.json ~/.config/basalt/games/
basalt catalog                       # the game is listed
basalt install MyGame --library default
basalt actions MyGame                # the actions the GUI will show
basalt maintain MyGame backup
basalt backups MyGame
basalt sync-remote /mnt/nas/Games
basalt maintain MyGame sync
basalt maintain MyGame sync --verify   # re-read everything on both sides
```

## 11. Legacy catalog entries

Entries written before this standard use `schema_version: 1` and keep working unchanged: reinstalling
preserves their `persistent_directory` (default `run/`) and replaces everything else, they get no
built-in actions, and each `maintenance` script is offered as an action, run attached to the terminal
as before. Their archives may still wrap everything in one top-level folder, which Basalt detects by
looking for the launch executable; `source.archive_root` is not used with them. To adopt the standard, a game moves to the layout in [section 1](#1-install-directory-layout)
(or describes its own with `layout`) and its entry changes to `schema_version: 2` with
`standard_version: 1`, replacing `maintenance` with `actions` and dropping `persistent_directory`.
