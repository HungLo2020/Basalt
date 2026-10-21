//! Discover catalog manifests at build time so the installed binaries remain self-contained.

use std::{env, fs, path::PathBuf};

fn main() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources/games");
    // Watching the directory also catches newly added and removed manifests.
    println!("cargo::rerun-if-changed={}", directory.display());
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .expect("read bundled game catalog directory")
        .map(|entry| entry.expect("read bundled game catalog entry").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    let mut source = String::from("&[\n");
    for path in paths {
        println!("cargo::rerun-if-changed={}", path.display());
        source.push_str(&format!(
            "    ({:?}, include_str!({:?})),\n",
            path.file_name().expect("manifest filename"),
            path,
        ));
    }
    source.push_str("]\n");

    // Cargo provides this build-script-only environment variable.
    #[allow(clippy::disallowed_methods)]
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"));
    fs::write(output.join("bundled_catalog.rs"), source).expect("write bundled catalog index");
}
