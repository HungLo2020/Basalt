use std::fs;

use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlModule};
use qt_build_utils::QResourceFile;

/// Prefix of the pure-QML module `org.basalt.ui` (see qml/qmldir).
const UI_MODULE_PREFIX: &str = "/qt/qml/org/basalt/ui";

/// (file, path inside the org.basalt.app module) for images compiled into the binary.
const ASSETS: &[(&str, &str)] = &[
    (
        "../../resources/assets/icons/apps/MattMC.svg",
        "assets/MattMC.svg",
    ),
    (
        "../../resources/assets/icons/basalt.svg",
        "assets/basalt.svg",
    ),
];

fn main() {
    let assets = ASSETS
        .iter()
        .fold(QResource::new(), |resource, (file, alias)| {
            println!("cargo::rerun-if-changed={}", file);
            resource.file(QResourceFile::new(file).alias(*alias))
        });

    // The QML pages are embedded as plain resources and compiled by Qt at runtime. They are not
    // given to qmlcachegen: its ahead-of-time C++ output uses Qt's private ABI, which would tie
    // the binary (and the .deb) to one exact Qt version.
    println!("cargo::rerun-if-changed=qml");
    let mut ui_files: Vec<String> = fs::read_dir("qml")
        .expect("qml directory")
        .map(|entry| {
            entry
                .expect("qml entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name == "qmldir" || name.ends_with(".qml"))
        .collect();
    ui_files.sort();
    let ui = ui_files.iter().fold(
        QResource::new().prefix(UI_MODULE_PREFIX),
        |resource, name| resource.file(QResourceFile::new(format!("qml/{}", name)).alias(name)),
    );

    // org.basalt.app holds only the Rust `Backend` singleton.
    CxxQtBuilder::new_qml_module(QmlModule::new("org.basalt.app"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Widgets")
        .files(["src/app.rs", "src/backend.rs"])
        .cpp_files([
            "cpp/app.cpp",
            "cpp/windowgrabber.h",
            "cpp/windowgrabber.cpp",
        ])
        .qrc_resources(QResources::new().resource(assets).resource(ui))
        .build();
}
