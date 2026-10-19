//! STANDARDS.md games end to end: launch environment, built-in backup/restore/sync/clear_cache,
//! catalog overrides and script actions. A local catalog entry (~/.config/basalt/games) and a
//! hand-made installation stand in for a downloaded release, so no network is needed.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{TestHome, stdout};

/// Registers a standard game "Sample" installed in `<home>/<library>/Sample`, merging `extra`
/// into its catalog entry. Its executable records its environment and arguments.
fn install_sample(home: &TestHome, library: &str, extra: serde_json::Value) -> PathBuf {
    let mut entry = serde_json::json!({
        "schema_version": 2,
        "standard_version": 1,
        "id": "Sample",
        "name": "Sample",
        "source": {
            "kind": "github_release",
            "repository": "example/Sample",
            "asset_prefix": "Sample-",
            "asset_suffix": ".zip"
        },
        "launch": { "executable": "app/bin/sample.sh" }
    });
    for (key, value) in extra.as_object().unwrap() {
        entry[key] = value.clone();
    }
    let catalog = home.config_dir().join("games");
    fs::create_dir_all(&catalog).unwrap();
    fs::write(catalog.join("sample.json"), entry.to_string()).unwrap();

    let root = home.path.join(library).join("Sample");
    write(
        &root.join("app/bin/sample.sh"),
        "#!/bin/bash\nenv | grep '^BASALT_' | sort > \"$BASALT_CACHE_DIR/env.txt\"\npwd >> \"$BASALT_CACHE_DIR/env.txt\"\nsleep \"${SAMPLE_SLEEP:-0}\"\n",
    );
    for folder in ["data/save", "data/config/shared", "data/cache"] {
        fs::create_dir_all(root.join(folder)).unwrap();
    }
    let manifest = root.join(".basalt-game.json");
    fs::write(&manifest, entry.to_string()).unwrap();
    fs::create_dir_all(home.data_dir()).unwrap();
    fs::write(
        home.data_dir().join("games.tsv"),
        format!("Sample\tcatalog\t{}\n", manifest.display()),
    )
    .unwrap();
    root
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn failure(output: &std::process::Output) -> String {
    assert!(!output.status.success(), "expected failure");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn actions_list_builtins_for_standard_games_and_scripts_for_legacy_ones() {
    let home = TestHome::new("standard-actions");
    install_sample(&home, "Games", serde_json::json!({}));
    let actions = stdout(&home.run(&["actions", "Sample"]));
    for name in ["update", "backup", "restore", "sync", "sync_up", "sync_down", "clear_cache"] {
        assert!(actions.contains(&format!("{name}\t")), "{name} missing: {actions}");
    }
    home.use_legacy_mattmc();
    let legacy = stdout(&home.run(&["actions", "MattMC"]));
    assert!(legacy.contains("sync_up\tSync up\tscript SyncGameData.sh"));
    assert!(!legacy.contains("built-in"));
}

#[test]
fn launch_sets_standard_environment_in_any_library() {
    let home = TestHome::new("standard-launch");
    let root = install_sample(&home, "second disk/Games", serde_json::json!({}));
    stdout(&home.run(&["launch", "Sample"]));
    let env = read(&root.join("data/cache/env.txt"));
    let root = root.display();
    for line in [
        "BASALT_GAME_ID=Sample".to_string(),
        format!("BASALT_GAME_DIR={root}"),
        format!("BASALT_APP_DIR={root}/app"),
        format!("BASALT_SAVE_DIR={root}/data/save"),
        format!("BASALT_CONFIG_DIR={root}/data/config"),
        format!("BASALT_SHARED_CONFIG_DIR={root}/data/config/shared"),
        format!("BASALT_CACHE_DIR={root}/data/cache"),
        "BASALT_STANDARD_VERSION=1".to_string(),
    ] {
        assert!(env.lines().any(|l| l == line), "missing {line} in:\n{env}");
    }
    // Launches run from the install root and leave no running marker behind.
    assert!(env.lines().any(|l| l == root.to_string()));
    assert!(!Path::new(&format!("{root}/.basalt/running.pid")).exists());
}

#[test]
fn layout_overrides_remap_environment_variables() {
    let home = TestHome::new("standard-layout-env");
    let root = install_sample(
        &home,
        "Games",
        serde_json::json!({"layout": {"save": ["run/saves", "run/mods"], "cache": ["data/cache"]}}),
    );
    stdout(&home.run(&["launch", "Sample"]));
    let env = read(&root.join("data/cache/env.txt"));
    assert!(env.contains(&format!("BASALT_SAVE_DIR={}/run/saves", root.display())));
}

#[test]
fn backup_and_restore_round_trip_with_safety_backup() {
    let home = TestHome::new("standard-backup");
    let root = install_sample(&home, "Games", serde_json::json!({}));
    write(&root.join("data/save/slot1"), "chapter 1");
    write(&root.join("data/config/video.ini"), "1080p");
    write(&root.join("data/config/shared/keys.ini"), "wasd");
    write(&root.join("data/cache/shaders.bin"), "cache");
    assert!(stdout(&home.run(&["maintain", "Sample", "backup"])).contains("3 files"));

    write(&root.join("data/save/slot1"), "chapter 9");
    write(&root.join("data/save/world/extra"), "created later");
    write(&root.join("data/config/shared/keys.ini"), "arrows");

    let backups = stdout(&home.run(&["backups", "Sample"]));
    let id = backups.lines().next().unwrap().split('\t').next().unwrap().to_string();
    assert!(id.ends_with("-manual"), "{backups}");
    assert!(failure(&home.run(&["maintain", "Sample", "restore"])).contains("--snapshot"));
    stdout(&home.run(&["maintain", "Sample", "restore", "--snapshot", &id]));

    assert_eq!(read(&root.join("data/save/slot1")), "chapter 1");
    assert!(!root.join("data/save/world").exists());
    assert_eq!(read(&root.join("data/config/shared/keys.ini")), "wasd");
    assert_eq!(read(&root.join("data/cache/shaders.bin")), "cache");
    let backups = stdout(&home.run(&["backups", "Sample"]));
    assert!(backups.lines().next().unwrap().contains("before-restore"));
}

#[test]
fn backup_keep_option_prunes_old_snapshots() {
    let home = TestHome::new("standard-backup-keep");
    let root = install_sample(
        &home,
        "Games",
        serde_json::json!({"actions": {"backup": {"options": {"keep": 2, "include": ["save"]}}}}),
    );
    write(&root.join("data/save/slot1"), "x");
    write(&root.join("data/config/video.ini"), "excluded");
    for _ in 0..3 {
        assert!(stdout(&home.run(&["maintain", "Sample", "backup"])).contains("(1 files)"));
    }
    assert_eq!(stdout(&home.run(&["backups", "Sample"])).lines().count(), 2);
}

#[test]
fn sync_moves_save_and_shared_config_between_machines_and_detects_conflicts() {
    let a = TestHome::new("standard-sync-a");
    let b = TestHome::new("standard-sync-b");
    let remote = a.path.join("nas");
    fs::create_dir_all(&remote).unwrap();
    let a_root = install_sample(&a, "Games", serde_json::json!({}));
    let b_root = install_sample(&b, "elsewhere/Library", serde_json::json!({}));
    for home in [&a, &b] {
        stdout(&home.run(&["sync-remote", remote.to_str().unwrap()]));
    }

    write(&a_root.join("data/save/slot1"), "from A");
    write(&a_root.join("data/config/shared/keys.ini"), "A keys");
    write(&a_root.join("data/config/video.ini"), "A display");
    assert!(stdout(&a.run(&["maintain", "Sample", "sync"])).contains("up"));
    assert_eq!(read(&remote.join("Sample/data/save/slot1")), "from A");
    assert!(!remote.join("Sample/data/config/video.ini").exists());

    // B has never synced and has different data: refuse to guess.
    write(&b_root.join("data/save/slot1"), "from B");
    write(&b_root.join("data/config/video.ini"), "B display");
    let conflict = failure(&b.run(&["maintain", "Sample", "sync"]));
    assert!(conflict.contains("sync_up") && conflict.contains("sync_down"), "{conflict}");
    stdout(&b.run(&["maintain", "Sample", "sync_down"]));
    assert_eq!(read(&b_root.join("data/save/slot1")), "from A");
    assert_eq!(read(&b_root.join("data/config/shared/keys.ini")), "A keys");
    assert_eq!(read(&b_root.join("data/config/video.ini")), "B display");
    assert!(stdout(&b.run(&["backups", "Sample"])).contains("before-sync-down"));

    // Only B changes: B's sync goes up, then A's sync comes down.
    write(&b_root.join("data/save/slot1"), "B progress");
    assert!(stdout(&b.run(&["maintain", "Sample", "sync"])).contains("up"));
    assert!(stdout(&a.run(&["maintain", "Sample", "sync"])).contains("down"));
    assert_eq!(read(&a_root.join("data/save/slot1")), "B progress");
    assert!(stdout(&a.run(&["maintain", "Sample", "sync"])).contains("already in sync"));

    // A fresh install elsewhere simply takes the synced data on its first sync.
    let c = TestHome::new("standard-sync-c");
    let c_root = install_sample(&c, "Games", serde_json::json!({}));
    stdout(&c.run(&["sync-remote", remote.to_str().unwrap()]));
    assert!(stdout(&c.run(&["maintain", "Sample", "sync"])).contains("down"));
    assert_eq!(read(&c_root.join("data/save/slot1")), "B progress");
}

#[test]
fn sync_folder_defaults_and_must_be_available() {
    // Never sync without configuring a folder here: the default is a real share.
    let home = TestHome::new("standard-sync-folder");
    install_sample(&home, "Games", serde_json::json!({}));
    assert_eq!(
        stdout(&home.run(&["sync-remote"])).trim(),
        "/mnt/storage/Storage/Sync (default)"
    );
    assert!(!home.run(&["sync-remote", "relative/path"]).status.success());
    let missing = home.path.join("unmounted");
    stdout(&home.run(&["sync-remote", missing.to_str().unwrap()]));
    assert_eq!(
        stdout(&home.run(&["sync-remote"])).trim(),
        missing.to_str().unwrap()
    );
    assert!(failure(&home.run(&["maintain", "Sample", "sync_up"])).contains("mounted"));
    assert!(!missing.exists(), "must not create an unmounted share's mount point");
    assert!(stdout(&home.run(&["sync-remote", "--clear"])).contains("/mnt/storage/Storage/Sync"));
    assert!(stdout(&home.run(&["sync-remote"])).contains("(default)"));
}

#[test]
fn clear_cache_only_touches_cache() {
    let home = TestHome::new("standard-cache");
    let root = install_sample(&home, "Games", serde_json::json!({}));
    write(&root.join("data/cache/shaders/a.bin"), "x");
    write(&root.join("data/save/slot1"), "keep");
    assert!(stdout(&home.run(&["maintain", "Sample", "clear_cache"])).contains("1 cached"));
    assert!(!root.join("data/cache/shaders").exists());
    assert!(root.join("data/cache").is_dir());
    assert!(root.join("data/save/slot1").exists());
}

#[test]
fn actions_wait_for_the_game_to_close_except_clear_cache() {
    let home = TestHome::new("standard-running");
    let root = install_sample(&home, "Games", serde_json::json!({}));
    let mut game = home.command(&["launch", "Sample"]).env("SAMPLE_SLEEP", "3").spawn().unwrap();
    let marker = root.join(".basalt/running.pid");
    for _ in 0..50 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(marker.exists());
    assert!(failure(&home.run(&["maintain", "Sample", "backup"])).contains("Close Sample"));
    assert!(failure(&home.run(&["launch", "Sample"])).contains("already running"));
    stdout(&home.run(&["maintain", "Sample", "clear_cache"]));
    game.wait().unwrap();
    stdout(&home.run(&["maintain", "Sample", "backup"]));
}

#[test]
fn scripts_replace_builtins_and_add_actions() {
    let home = TestHome::new("standard-scripts");
    let root = install_sample(
        &home,
        "my games/lib",
        serde_json::json!({"actions": {
            "sync": false,
            "sync_up": {"script": {"path": "app/scripts/sync.sh", "arguments": ["--up"], "stdin": "yes\n"}},
            "repair": {"label": "Repair install", "script": {"path": "app/scripts/repair.sh"}}
        }}),
    );
    write(
        &root.join("app/scripts/sync.sh"),
        "#!/bin/bash\nread -r answer\necho \"$1 $answer $BASALT_ACTION $(pwd)\" > \"$BASALT_SAVE_DIR/record\"\necho 'Uploading...'\n",
    );
    write(
        &root.join("app/scripts/repair.sh"),
        "#!/bin/bash\necho 'checking files'\necho 'Repair failed: disk is read-only' >&2\nexit 3\n",
    );
    stdout(&home.run(&["maintain", "Sample", "sync_up"]));
    assert_eq!(
        read(&root.join("data/save/record")).trim(),
        format!("--up yes sync_up {}", root.display())
    );
    let error = failure(&home.run(&["maintain", "Sample", "repair"]));
    assert!(error.contains("Error: Repair failed: disk is read-only"), "{error}");
    assert!(failure(&home.run(&["maintain", "Sample", "sync"])).contains("does not support"));
    let actions = stdout(&home.run(&["actions", "Sample"]));
    assert!(actions.contains("repair\tRepair install\tscript app/scripts/repair.sh"));
}

#[test]
fn standard_mattmc_refuses_actions_on_an_old_run_mattmc_registration() {
    let home = TestHome::new("standard-old-mattmc");
    let old = home.path.join("Games/MattMC");
    // Legacy bash launches run in the caller's directory, so record via an absolute path.
    let launched = home.path.join("launched.txt");
    write(
        &old.join("run-mattmc.sh"),
        &format!("#!/bin/bash\necho launched > '{}'\n", launched.display()),
    );
    write(&old.join("run/saves/world/level.dat"), "world");
    // Discovery still finds the old installation, and it still launches.
    stdout(&home.run(&["discover", "--catalog"]));
    assert!(stdout(&home.run(&["list"])).contains("MattMC\tbash"));
    stdout(&home.run(&["launch", "MattMC"]));
    assert_eq!(read(&launched).trim(), "launched");
    let actions = stdout(&home.run(&["actions", "MattMC"]));
    assert!(actions.contains("sync_down\tSync down\tbuilt-in"), "{actions}");
    // Built-in actions must not guess the installation root of the old layout.
    for action in ["backup", "sync_down", "clear_cache"] {
        let error = failure(&home.run(&["maintain", "MattMC", action]));
        assert!(error.contains("Reinstall it"), "{action}: {error}");
    }
    assert_eq!(read(&old.join("run/saves/world/level.dat")), "world");
}

#[test]
fn invalid_local_catalog_entries_are_ignored_with_a_warning() {
    let home = TestHome::new("standard-invalid");
    let catalog = home.config_dir().join("games");
    write(&catalog.join("broken.json"), "{\"schema_version\": 2}");
    let output = home.run(&["catalog"]);
    assert!(stdout(&output).contains("MattMC"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Ignoring local catalog entry"));
}
