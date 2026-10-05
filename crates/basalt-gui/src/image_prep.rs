//! Turns artwork files into display-ready RGBA buffers sized for library tiles.

use std::path::Path;

const MATTMC_SVG_TARGET_MAX_DIMENSION: u32 = 1024;
const PREPARED_ARTWORK_MAX_WIDTH: u32 = 360;
const PREPARED_ARTWORK_MAX_HEIGHT: u32 = 540;

pub(super) struct PreparedArtwork {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) rgba: Vec<u8>,
}

pub(super) fn prepare_artwork_payload_from_path(path: &Path) -> Option<PreparedArtwork> {
    let image_bytes = std::fs::read(path).ok()?;
    let foreground_rgba = image::load_from_memory(&image_bytes).ok()?.to_rgba8();
    prepare_artwork_payload_from_rgba(foreground_rgba)
}

pub(super) fn prepare_artwork_payload_from_svg_bytes(svg_bytes: &[u8]) -> Option<PreparedArtwork> {
    let usvg_options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg_bytes, &usvg_options).ok()?;
    let base_size = tree.size().to_int_size();
    let largest_dim = base_size.width().max(base_size.height()) as f32;
    if largest_dim <= 0.0 {
        return None;
    }

    let scale = (MATTMC_SVG_TARGET_MAX_DIMENSION as f32 / largest_dim).max(1.0);
    let target_width = ((base_size.width() as f32) * scale).round().max(1.0) as u32;
    let target_height = ((base_size.height() as f32) * scale).round().max(1.0) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let foreground_rgba =
        image::RgbaImage::from_raw(target_width, target_height, pixmap.data().to_vec())?;

    prepare_artwork_payload_from_rgba(foreground_rgba)
}

fn prepare_artwork_payload_from_rgba(foreground_rgba: image::RgbaImage) -> Option<PreparedArtwork> {
    let foreground_rgba = resize_for_prepared_artwork(
        foreground_rgba,
        PREPARED_ARTWORK_MAX_WIDTH,
        PREPARED_ARTWORK_MAX_HEIGHT,
    );

    let width = usize::try_from(foreground_rgba.width()).ok()?;
    let height = usize::try_from(foreground_rgba.height()).ok()?;

    Some(PreparedArtwork {
        width,
        height,
        rgba: foreground_rgba.into_raw(),
    })
}

fn resize_for_prepared_artwork(
    rgba: image::RgbaImage,
    max_width: u32,
    max_height: u32,
) -> image::RgbaImage {
    let width = rgba.width();
    let height = rgba.height();

    if width == 0 || height == 0 || width <= max_width && height <= max_height {
        return rgba;
    }

    let width_scale = max_width as f32 / width as f32;
    let height_scale = max_height as f32 / height as f32;
    let scale = width_scale.min(height_scale).min(1.0);

    let resized_width = ((width as f32) * scale).round().max(1.0) as u32;
    let resized_height = ((height as f32) * scale).round().max(1.0) as u32;

    image::imageops::resize(
        &rgba,
        resized_width,
        resized_height,
        image::imageops::FilterType::Triangle,
    )
}
