mod common;
use common::{TestHome, stdout};
use std::fs;

#[test]
fn library_configuration_and_noninteractive_install_selection() {
    let home = TestHome::new("catalog-libraries");
    let extra = home.path.join("second disk/Games");
    stdout(&home.run(&["libraries", "add", "ssd", extra.to_str().unwrap()]));
    let libraries = stdout(&home.run(&["libraries", "list"]));
    assert!(libraries.contains("default"));
    assert!(libraries.contains(extra.to_str().unwrap()));
    assert!(
        !home
            .run(&["libraries", "add", "ssd", extra.to_str().unwrap()])
            .status
            .success()
    );
    let output = home.run(&["install", "MattMC"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--library"));
    let catalog = stdout(&home.run(&["catalog"]));
    for name in ["MattMC", "MicroMages", "Jackal"] {
        assert!(catalog.contains(name));
    }
    assert!(!home.run(&["install-mattmc"]).status.success());
}

#[test]
fn catalog_launch_honors_arguments_cwd_and_generic_maintenance_in_another_library() {
    let home = TestHome::new("catalog-launch");
    let root = home.path.join("second disk/Games/MattMC");
    fs::create_dir_all(root.join("run")).unwrap();
    fs::write(
        root.join("run-mattmc.sh"),
        "#!/bin/sh\nprintf '%s' \"$1\" > argument.txt\npwd > working-directory.txt\n",
    )
    .unwrap();
    fs::write(
        root.join("backup.sh"),
        "#!/bin/sh\npwd > backup-directory.txt\n",
    )
    .unwrap();
    let mut manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../resources/games/mattmc.json")).unwrap();
    manifest["launch"]["arguments"] = serde_json::json!(["argument with spaces; $(not a command)"]);
    manifest["launch"]["working_directory"] = serde_json::json!("run");
    let target = root.join(".basalt-game.json");
    fs::write(&target, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::create_dir_all(home.data_dir()).unwrap();
    fs::write(
        home.data_dir().join("games.tsv"),
        format!("MattMC\tcatalog\t{}\n", target.display()),
    )
    .unwrap();
    stdout(&home.run(&["launch", "MattMC"]));
    assert_eq!(
        fs::read_to_string(root.join("run/argument.txt")).unwrap(),
        "argument with spaces; $(not a command)"
    );
    assert_eq!(
        fs::read_to_string(root.join("run/working-directory.txt"))
            .unwrap()
            .trim(),
        root.join("run").to_str().unwrap()
    );
    stdout(&home.run(&["maintain", "MattMC", "backup"]));
    assert_eq!(
        fs::read_to_string(root.join("backup-directory.txt"))
            .unwrap()
            .trim(),
        root.to_str().unwrap()
    );
    // Even a competing old default-directory installation must not hijack the selected library.
    let legacy = home.path.join("Games/MattMC");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("run-mattmc.sh"), "exit 1").unwrap();
    stdout(&home.run(&["discover", "--mattmc"]));
    assert!(
        fs::read_to_string(home.data_dir().join("games.tsv"))
            .unwrap()
            .contains(target.to_str().unwrap())
    );
}
