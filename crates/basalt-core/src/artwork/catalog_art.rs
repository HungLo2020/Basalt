//! Artwork for catalog games (STANDARDS.md, "Artwork"): files a game ships in `app/basalt/`,
//! and the cover its catalog entry links to, downloaded once and cached.

use std::path::{Path, PathBuf};

use xxhash_rust::xxh3::xxh3_64;

use super::{cache, fetch, validation};

const ARTWORK_DIR: &str = "app/basalt";
const COVER_FILES: [&str; 2] = ["cover.png", "cover.jpg"];
const ICON_FILES: [&str; 2] = ["icon.png", "icon.svg"];

/// The game's shipped cover, if it is a readable image.
pub(super) fn shipped_cover(install_root: Option<&Path>) -> Option<PathBuf> {
    let directory = install_root?.join(ARTWORK_DIR);
    COVER_FILES
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file() && validation::is_readable_image(path))
}

/// The game's shipped icon. SVG icons are left for the front end to render.
pub(super) fn shipped_icon(install_root: Option<&Path>) -> Option<PathBuf> {
    let directory = install_root?.join(ARTWORK_DIR);
    ICON_FILES.iter().map(|name| directory.join(name)).find(|path| {
        path.is_file()
            && (path.extension().is_some_and(|ext| ext == "svg")
                || validation::is_readable_image(path))
    })
}

/// Cache file for a cover URL; the name includes a hash of the URL, so a changed URL is fetched
/// again while an unchanged one never is.
fn cache_path(game_id: &str, url: &str) -> Option<PathBuf> {
    let extension = if url.to_ascii_lowercase().ends_with(".png") { "png" } else { "jpg" };
    Some(cache::catalog_artwork_cache_dir()?.join(format!(
        "{game_id}-{:016x}.{extension}",
        xxh3_64(url.as_bytes())
    )))
}

pub(super) fn cached_cover(game_id: &str, url: Option<&str>) -> Option<PathBuf> {
    let path = cache_path(game_id, url?)?;
    (path.is_file() && validation::is_readable_image(&path)).then_some(path)
}

/// Downloads the catalog's cover URL into the cache.
pub(super) fn download_cover(game_id: &str, url: &str) -> Option<PathBuf> {
    if let Some(cached) = cached_cover(game_id, Some(url)) {
        return Some(cached);
    }
    let path = cache_path(game_id, url)?;
    let partial = path.with_extension("part");
    if fetch::download_url_to_file_with_user_agent(url, &partial, "Basalt-Game-Artwork")
        && validation::is_readable_image(&partial)
        && std::fs::rename(&partial, &path).is_ok()
    {
        return Some(path);
    }
    let _ = std::fs::remove_file(&partial);
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_layout::tests::temp_root;

    fn png(path: &Path, width: u32, height: u32) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::new(width, height).save(path).unwrap();
    }

    #[test]
    fn shipped_cover_and_icon_are_found_in_app_basalt() {
        let root = temp_root("art");
        assert!(shipped_cover(Some(&root)).is_none());
        assert!(shipped_icon(Some(&root)).is_none());
        std::fs::create_dir_all(root.join("app/basalt")).unwrap();
        std::fs::write(root.join("app/basalt/icon.svg"), "<svg/>").unwrap();
        assert_eq!(shipped_icon(Some(&root)), Some(root.join("app/basalt/icon.svg")));
        png(&root.join("app/basalt/icon.png"), 64, 64);
        assert_eq!(shipped_icon(Some(&root)), Some(root.join("app/basalt/icon.png")));
        // A file that is not an image is ignored rather than shown broken.
        std::fs::write(root.join("app/basalt/cover.png"), "not an image").unwrap();
        assert!(shipped_cover(Some(&root)).is_none());
        png(&root.join("app/basalt/cover.jpg"), 90, 120);
        assert_eq!(shipped_cover(Some(&root)), Some(root.join("app/basalt/cover.jpg")));
        png(&root.join("app/basalt/cover.png"), 90, 120);
        assert_eq!(shipped_cover(Some(&root)), Some(root.join("app/basalt/cover.png")));
        assert!(shipped_cover(None).is_none());
        let _ = std::fs::remove_dir_all(root);
    }
}
