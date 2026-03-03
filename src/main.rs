mod cli;
mod core;

fn main() {
    if let Err(error_message) = cli::run() {
        eprintln!("Error: {}", error_message);
        std::process::exit(1);
    }
}
