use std::path::Path;

use super::{EMULATOR_ARTWORK_MIN_HEIGHT, EMULATOR_ARTWORK_MIN_WIDTH};

/// Steam "library_600x900" portraits: big enough and roughly 2:3.
pub(super) fn is_valid_portrait_artwork(path: &Path) -> bool {
    let Some((width, height)) = image_dimensions(path) else {
        return false;
    };

    if width < 300 || height < 450 {
        return false;
    }

    let aspect_ratio = width as f32 / height as f32;
    (0.60..=0.74).contains(&aspect_ratio)
}

pub(super) fn is_valid_emulator_artwork(path: &Path) -> bool {
    image_dimensions(path).is_some_and(|(width, height)| {
        width >= EMULATOR_ARTWORK_MIN_WIDTH && height >= EMULATOR_ARTWORK_MIN_HEIGHT
    })
}

/// Reads only the image header, not the pixel data.
fn image_dimensions(path: &Path) -> Option<(u32, u32)> {
    image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}
