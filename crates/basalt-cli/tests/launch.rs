//! Launching must never implicitly install software.
mod common;
use common::TestHome;
#[test]
fn missing_game_does_not_trigger_installation() {
    let home = TestHome::new("launch-missing");
    let result = home.run(&["launch", "Nonexistent Game"]);
    assert!(!result.status.success());
    assert!(!home.path.join("Games/Nonexistent Game").exists());
}
