mod cli;
mod core;
mod gui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let result = if args.is_empty() {
        gui::run()
    } else {
        cli::run(&args)
    };

    if let Err(error_message) = result {
        eprintln!("Error: {}", error_message);
        std::process::exit(1);
    }
}
