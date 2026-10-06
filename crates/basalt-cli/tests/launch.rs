//! Launching must never install software or download anything.

mod common;

use std::fs;

use common::{stdout, TestHome};

#[test]
fn launching_an_emulator_game_without_its_core_fails_without_installing() {
    let home = TestHome::new("launch");
    let roms = home.path.join("Games/Emulators/roms/gba");
    fs::create_dir_all(&roms).unwrap();
    fs::write(roms.join("Test Game.gba"), [0u8; 512]).unwrap();
    stdout(&home.run(&["discover", "--emulators"]));

    let launch = home.run(&["launch", "Test Game"]);

    assert!(
        !launch.status.success(),
        "launch should fail without a core"
    );
    let stderr = String::from_utf8_lossy(&launch.stderr);
    // Either RetroArch or the GBA core is missing, depending on the machine; both say how to
    // install it rather than installing it.
    assert!(stderr.contains("is not installed"), "{}", stderr);
    assert!(stderr.contains("basalt install-"), "{}", stderr);

    let cores = home.path.join("Games/Emulators/runtime/retroarch/cores");
    let downloaded = fs::read_dir(&cores)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(downloaded, 0, "launch must not download cores");
}
