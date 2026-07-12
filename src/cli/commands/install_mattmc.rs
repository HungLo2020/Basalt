use crate::core;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("Usage: basalt install-mattmc".to_string());
    }

    let report = core::install_mattmc().map_err(String::from)?;
    println!(
        "Installed MattMC release '{}' into {}",
        report.release_tag,
        report.install_dir.display()
    );
    println!("{}", report.discovery_message());

    for warning in report.cleanup_warnings {
        eprintln!("Warning: {}", warning);
    }

    Ok(())
}
