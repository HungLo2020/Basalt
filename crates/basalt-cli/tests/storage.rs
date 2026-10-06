//! End-to-end tests of Basalt's on-disk storage through the real `basalt` binary.
//!
//! Every test runs the CLI with its own throwaway HOME (and the XDG variables removed), so
//! nothing touches the developer's real data, and separate processes are as independent as
//! the CLI and GUI are in real use.

mod common;

use std::fs;
use std::path::Path;
use std::process::Child;

use common::{TestHome, stdout};
use serde_json::Value;

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn concurrent_adds_from_many_processes_are_all_kept() {
    const GAMES: usize = 40;
    let home = TestHome::new("race");
    // Create the data dir first so all processes race on the registry, not on setup.
    stdout(&home.run(&["list"]));

    let scripts: Vec<String> = (0..GAMES)
        .map(|i| home.script(&format!("game{}", i)))
        .collect();
    let children: Vec<Child> = scripts
        .iter()
        .enumerate()
        .map(|(i, script)| home.spawn(&["add", &format!("Game {}", i), script]))
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }

    let listing = stdout(&home.run(&["list"]));
    let added = (0..GAMES)
        .filter(|i| listing.contains(&format!("  Game {}\tbash\t", i)))
        .count();
    assert_eq!(added, GAMES, "lost updates:\n{}", listing);

    // Atomic writes leave no temp files behind.
    let leftovers: Vec<String> = files_in(&home.data_dir())
        .into_iter()
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temp files left: {:?}", leftovers);
}

#[test]
fn legacy_basalt_dir_is_migrated_to_xdg_locations() {
    let home = TestHome::new("migrate");
    let script = home.script("celeste");
    let legacy = home.path.join(".basalt");
    fs::create_dir_all(legacy.join("cache/steam_artwork")).unwrap();
    let games = format!("Celeste\tbash\t{}\nPortal 2\tsteam\t620\n", script);
    // Includes the automatic Steam playlist, as real data does, so listing doesn't rewrite it.
    let playlists = "Favorites\tCeleste\nSteam\tPortal 2\n";
    fs::write(legacy.join("games.tsv"), &games).unwrap();
    fs::write(legacy.join("playlists.tsv"), playlists).unwrap();
    fs::write(legacy.join("blacklist.txt"), "# none\n").unwrap();
    fs::write(
        legacy.join("settings.json"),
        r#"{"emulation":{"remote_roms_root_dir":"/legacy/roms"}}"#,
    )
    .unwrap();
    fs::write(legacy.join("cache/steam_artwork/620.jpg"), "jpg").unwrap();

    let listing = stdout(&home.run(&["list"]));
    assert!(listing.contains("  Celeste\tbash\t"), "{}", listing);
    assert!(listing.contains("  Portal 2\tsteam\t620"), "{}", listing);
    assert!(listing.contains("    - Celeste"), "{}", listing);

    assert_eq!(
        fs::read_to_string(home.data_dir().join("games.tsv")).unwrap(),
        games
    );
    assert_eq!(
        fs::read_to_string(home.data_dir().join("playlists.tsv")).unwrap(),
        playlists
    );
    assert!(home.data_dir().join("blacklist.txt").is_file());
    assert!(home.config_dir().join("settings.json").is_file());
    assert!(
        home.path
            .join(".cache/basalt/steam_artwork/620.jpg")
            .is_file()
    );
    assert!(!legacy.exists(), "emptied legacy dir should be removed");

    let settings = stdout(&home.run(&["settings", "get"]));
    assert!(
        settings.contains("Remote ROM root: /legacy/roms"),
        "{}",
        settings
    );
}

#[test]
fn settings_round_trip_preserves_unknown_keys() {
    let home = TestHome::new("settings");
    fs::create_dir_all(home.config_dir()).unwrap();
    fs::write(
        home.config_dir().join("settings.json"),
        r#"{
            "emulation": { "remote_roms_root_dir": "/old/roms", "future_option": 3 },
            "launcher": { "fullscreen": true },
            "some_new_section": { "enabled": true }
        }"#,
    )
    .unwrap();

    stdout(&home.run(&["settings", "set", "--saves-root", "  /new/saves  "]));

    let saved: Value =
        serde_json::from_str(&fs::read_to_string(home.config_dir().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(saved["emulation"]["remote_roms_root_dir"], "/old/roms");
    assert_eq!(saved["emulation"]["remote_saves_root_dir"], "/new/saves");
    assert_eq!(saved["emulation"]["future_option"], 3);
    assert_eq!(saved["launcher"]["fullscreen"], true);
    assert_eq!(saved["some_new_section"]["enabled"], true);

    let shown = stdout(&home.run(&["settings", "get"]));
    assert!(shown.contains("Remote ROM root: /old/roms"), "{}", shown);
    assert!(shown.contains("Remote Saves root: /new/saves"), "{}", shown);
}

#[test]
fn duplicate_adds_fail_with_a_clear_error() {
    let home = TestHome::new("duplicate");
    let script = home.script("celeste");
    stdout(&home.run(&["add", "Celeste", &script]));

    let by_name = home.run(&["add", "Celeste", &home.script("other")]);
    assert!(!by_name.status.success());
    assert!(
        String::from_utf8_lossy(&by_name.stderr)
            .contains("A game with name 'Celeste' already exists")
    );

    let by_target = home.run(&["add", "Another Name", &script]);
    assert!(!by_target.status.success());
    assert!(String::from_utf8_lossy(&by_target.stderr).contains("already exists"));
}
