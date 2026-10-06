#![deny(clippy::disallowed_methods, clippy::disallowed_types)]

mod app;
mod artwork;
mod backend;
mod controller;
mod view_model;

fn main() {
    cxx_qt::init_crate!(basalt_gui_kirigami);
    cxx_qt::init_qml_module!("org.basalt.app");
    std::process::exit(app::ffi::run_application());
}
