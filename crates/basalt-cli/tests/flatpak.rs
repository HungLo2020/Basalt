mod common;

use common::{TestHome, stdout};
use std::{fs, os::unix::fs::PermissionsExt, process::Output};

struct FlatpakHome {
    home: TestHome,
}

impl FlatpakHome {
    fn new() -> Self {
        let home = TestHome::new("flatpak");
        let bin = home.path.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("flatpak");
        fs::write(&fake, r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/calls"
case "$1" in
  list)
    if [ -f "$HOME/list-fails" ]; then echo 'list unavailable' >&2; exit 1; fi
    if [ -f "$HOME/installed" ]; then
      printf 'net.supertuxkart.SuperTuxKart\t%s\t%s\n' "${TEST_BRANCH:-stable}" "${TEST_REMOTE:-flathub}"
    fi ;;
  install)
    if [ -f "$HOME/install-fails" ]; then echo 'remote unavailable' >&2; exit 1; fi
    : > "$HOME/installed" ;;
  update)
    if [ -f "$HOME/update-fails" ]; then echo 'update unavailable' >&2; exit 1; fi ;;
  run)
    [ -f "$HOME/installed" ] || exit 1 ;;
  *) echo 'unexpected command' >&2; exit 1 ;;
esac
"#).unwrap();
        fs::set_permissions(fake, fs::Permissions::from_mode(0o755)).unwrap();
        Self { home }
    }

    fn run(&self, args: &[&str]) -> Output {
        self.home
            .command(args)
            .env("PATH", self.home.path.join("bin"))
            .output()
            .unwrap()
    }

    fn marker(&self, name: &str) {
        fs::write(self.home.path.join(name), "").unwrap();
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.home.path.join("calls")).unwrap_or_default()
    }
}

#[test]
fn catalog_flatpak_installs_launches_updates_and_survives_registry_reload() {
    let fixture = FlatpakHome::new();
    assert!(
        stdout(&fixture.run(&["catalog"]))
            .contains("SuperTuxKart\tSuperTuxKart\tnet.supertuxkart.SuperTuxKart")
    );
    assert!(stdout(&fixture.run(&["install", "SuperTuxKart"])).contains("using Flatpak"));
    assert!(!fixture.home.path.join("Games").exists());
    let listing = stdout(&fixture.run(&["list"]));
    assert!(listing.contains("SuperTuxKart\tflatpak\t"));
    assert!(listing.contains("  Flatpak:\n    - SuperTuxKart\n"));
    stdout(&fixture.run(&["launch", "SuperTuxKart"]));
    let actions = stdout(&fixture.run(&["actions", "SuperTuxKart"]));
    assert_eq!(actions.lines().count(), 1);
    assert!(actions.starts_with("update\t"));
    stdout(&fixture.run(&["maintain", "SuperTuxKart", "update"]));
    assert!(
        !fixture
            .run(&["maintain", "SuperTuxKart", "sync"])
            .status
            .success()
    );
    let calls = fixture.calls();
    assert!(calls.contains("install --user --app --noninteractive --assumeyes --or-update flathub net.supertuxkart.SuperTuxKart//stable"));
    assert!(calls.contains("run --user --branch=stable net.supertuxkart.SuperTuxKart"));
    assert!(calls.contains(
        "update --user --app --noninteractive --assumeyes net.supertuxkart.SuperTuxKart//stable"
    ));
    stdout(&fixture.run(&["remove", "SuperTuxKart"]));
    assert!(stdout(&fixture.run(&["list"])).contains("  Flatpak:\n    (empty)\n"));
    assert!(
        fixture.home.path.join("installed").exists(),
        "removing the launcher entry must not uninstall the app"
    );
    stdout(&fixture.run(&["discover", "--catalog"]));
    stdout(&fixture.run(&["discover", "--catalog"]));
    assert!(stdout(&fixture.run(&["list"])).contains("  Flatpak:\n    - SuperTuxKart\n"));
    assert_eq!(
        stdout(&fixture.run(&["list"]))
            .matches("SuperTuxKart\tflatpak\t")
            .count(),
        1
    );
}

#[test]
fn discovery_requires_matching_user_app_branch_and_origin() {
    let fixture = FlatpakHome::new();
    fixture.marker("installed");
    for (key, value) in [("TEST_BRANCH", "beta"), ("TEST_REMOTE", "other")] {
        let output = fixture
            .home
            .command(&["discover", "--catalog"])
            .env("PATH", fixture.home.path.join("bin"))
            .env(key, value)
            .output()
            .unwrap();
        stdout(&output);
        assert!(!stdout(&fixture.run(&["list"])).contains("SuperTuxKart\tflatpak"));
    }
    stdout(&fixture.run(&["discover", "--catalog"]));
    assert!(stdout(&fixture.run(&["list"])).contains("SuperTuxKart\tflatpak"));
}

#[test]
fn failed_install_or_explicit_library_does_not_register_game() {
    let fixture = FlatpakHome::new();
    assert!(
        !fixture
            .run(&["install", "SuperTuxKart", "--library", "default"])
            .status
            .success()
    );
    assert!(fixture.calls().is_empty());
    fixture.marker("install-fails");
    let output = fixture.run(&["install", "SuperTuxKart"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("remote unavailable"));
    assert!(!stdout(&fixture.run(&["list"])).contains("SuperTuxKart\tflatpak"));
}

#[test]
fn missing_flatpak_is_actionable_and_does_not_break_catalog_discovery() {
    let home = TestHome::new("no-flatpak");
    let bin = home.path.join("empty-bin");
    fs::create_dir_all(&bin).unwrap();
    let run = |args: &[&str]| home.command(args).env("PATH", &bin).output().unwrap();
    let output = run(&["install", "SuperTuxKart"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Flatpak is not installed"));
    stdout(&run(&["discover", "--catalog"]));
}

#[test]
fn name_conflicts_are_checked_before_installing() {
    let fixture = FlatpakHome::new();
    let script = fixture.home.script("other");
    stdout(&fixture.run(&["add", "SuperTuxKart", &script]));
    assert!(!fixture.run(&["install", "SuperTuxKart"]).status.success());
    assert!(!fixture.calls().contains("install "));
}

#[test]
fn failed_update_or_missing_app_is_reported() {
    let fixture = FlatpakHome::new();
    stdout(&fixture.run(&["install", "SuperTuxKart"]));
    fixture.marker("update-fails");
    let output = fixture.run(&["maintain", "SuperTuxKart", "update"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("update unavailable"));
    fs::remove_file(fixture.home.path.join("installed")).unwrap();
    let output = fixture.run(&["maintain", "SuperTuxKart", "update"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no longer installed"));
}

#[test]
fn unavailable_flatpak_listing_does_not_block_native_catalog_discovery() {
    let fixture = FlatpakHome::new();
    fixture.marker("list-fails");
    let root = fixture.home.path.join("Games/MattMC");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("run-mattmc.sh"), "#!/bin/sh\nexit 0\n").unwrap();
    let output = fixture.run(&["discover", "--catalog"]);
    stdout(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("list unavailable"));
    assert!(stdout(&fixture.run(&["list"])).contains("MattMC\tbash\t"));
}

#[test]
fn flatpak_filter_rebuilds_existing_membership_without_changing_favorites() {
    let fixture = FlatpakHome::new();
    fixture.marker("installed");
    stdout(&fixture.run(&["add", "Portal 2", "620"]));
    fs::write(
        fixture.home.data_dir().join("playlists.tsv"),
        "Flatpak\tPortal 2\nFavorites\tPortal 2\n",
    )
    .unwrap();
    stdout(&fixture.run(&["discover", "--catalog"]));
    let listing = stdout(&fixture.run(&["list"]));
    assert!(listing.contains("  Favorites:\n    - Portal 2\n"));
    assert!(listing.contains("  Steam:\n    - Portal 2\n"));
    assert!(listing.contains("  Flatpak:\n    - SuperTuxKart\n"));
    assert!(!listing.contains("  Flatpak:\n    - Portal 2\n"));
}
