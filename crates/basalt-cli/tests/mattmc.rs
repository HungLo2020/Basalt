//! MattMC script commands, run against a stub MattMC install in a throwaway home (never the
//! real one). The stub sync script reads its direction from stdin like the real
//! SyncGameData.sh and records what it was told.

mod common;

use std::fs;
use std::path::PathBuf;

use common::{TestHome, stdout};

/// A fake MattMC install with recording stand-ins for its scripts. Returns the record file.
fn stub_mattmc(home: &TestHome) -> PathBuf {
    let dir = home.path.join("Games/MattMC");
    fs::create_dir_all(&dir).unwrap();
    let record = home.path.join("mattmc-record.txt");
    let record_arg = record.display();

    fs::write(dir.join("run-mattmc.sh"), "#!/bin/bash\nexit 0\n").unwrap();
    fs::write(
        dir.join("SyncGameData.sh"),
        format!(
            "#!/bin/bash\nset -euo pipefail\nread -r -p 'Enter sync direction: ' direction\necho \"sync $direction\" >> '{record_arg}'\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("update-mattmc.sh"),
        format!("#!/bin/bash\necho update >> '{record_arg}'\n"),
    )
    .unwrap();
    fs::write(
        dir.join("backup.sh"),
        format!("#!/bin/bash\necho backup >> '{record_arg}'\n"),
    )
    .unwrap();

    stdout(&home.run(&["discover", "--mattmc"]));
    record
}

fn recorded(record: &PathBuf) -> Vec<String> {
    fs::read_to_string(record)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn sync_commands_pass_the_direction_to_the_sync_script() {
    let home = TestHome::new("mattmc-sync");
    let record = stub_mattmc(&home);

    assert!(stdout(&home.run(&["sync-mattmc", "up"])).contains("sync-up"));
    assert!(stdout(&home.run(&["sync-mattmc", "down"])).contains("sync-down"));
    // The older spellings keep working.
    stdout(&home.run(&["sync-up", "mattmc"]));
    stdout(&home.run(&["sync-down", "mattmc"]));

    assert_eq!(
        recorded(&record),
        ["sync up", "sync down", "sync up", "sync down"]
    );
}

#[test]
fn interactive_sync_without_a_terminal_fails_instead_of_guessing() {
    let home = TestHome::new("mattmc-interactive");
    let record = stub_mattmc(&home);

    // No direction and no terminal: the script's prompt gets no answer.
    let output = home.run(&["sync-mattmc"]);

    assert!(!output.status.success());
    assert!(
        recorded(&record).is_empty(),
        "nothing should have been synced"
    );
}

#[test]
fn update_and_backup_run_their_scripts() {
    let home = TestHome::new("mattmc-scripts");
    let record = stub_mattmc(&home);

    stdout(&home.run(&["update-mattmc"]));
    stdout(&home.run(&["backup-mattmc"]));

    assert_eq!(recorded(&record), ["update", "backup"]);
}
