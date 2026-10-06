#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("basalt-gui-kirigami/cpp/app.h");

        fn run_application() -> i32;
    }
}
