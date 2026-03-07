use crate::core;

use super::super::usage;

pub fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err(usage::usage_refresh_metadata());
    }

    core::clear_artwork_cache()?;
    println!("Artwork metadata and image caches were cleared.");
    Ok(())
}
